import type {
  InsertTextRequest,
  InterceptRequest,
  IpcBackend,
  RunProcessRequest,
  RunProcessResponse,
  InterceptedKeyHook,
  PostExecHook,
  PreExecHook,
  PromptHook,
} from "@aws/amazon-q-developer-cli-ipc-backend-core";
import { Clientbound, Hostbound } from "@aws/amazon-q-developer-cli-proto/mux";
import { PacketStream } from "./packetStream.js";
import { CsWebsocket, Socket } from "./socket.js";
import { clientboundToPacket, packetToHostbound } from "./mux.js";
import { EditBufferHook } from "@aws/amazon-q-developer-cli-proto/local";
import { create } from "@bufbuild/protobuf";
import { RunProcessResponseSchema } from "@aws/amazon-q-developer-cli-proto/fig";
import Emittery from "emittery";

export { CsWebsocket };

const EditBufferHookSymbol = Symbol("EditBufferHook");
const PromptHookSymbol = Symbol("PromptHook");
const PreExecHookSymbol = Symbol("PreExecHook");
const PostExecHookSymbol = Symbol("PostExecHook");
const InterceptedKeyHookSymbol = Symbol("InterceptedKeyHook");

export class WebsocketMuxBackend implements IpcBackend {
  emitter: Emittery = new Emittery();
  packetStream: PacketStream;

  constructor(websocket: CsWebsocket) {
    const socket = Socket.cs(websocket);
    console.log("1. socket created");
    this.packetStream = new PacketStream(socket);
    console.log("2. packet stream created");
    this.packetStream.onPacket(async (packet) => {
      console.log("3. packet received");
      const hostbound = await packetToHostbound(packet);
      console.log("4. decoded hostbound", { hostbound });
      this.handleHostbound(hostbound);
      console.log("5. hostbound handled");
    });
  }

  private handleHostbound(message: Hostbound) {
    const submessage = message.submessage;
    console.log(submessage);
    switch (submessage?.case) {
      case "editBuffer":
        this.emitter.emit(EditBufferHookSymbol, submessage.value);
        break;
      case "interceptedKey":
        this.emitter.emit(InterceptedKeyHookSymbol, submessage.value);
        break;
      case "postExec":
        this.emitter.emit(PostExecHookSymbol, submessage.value);
        break;
      case "preExec":
        this.emitter.emit(PreExecHookSymbol, submessage.value);
        break;
      case "prompt":
        this.emitter.emit(PromptHookSymbol, submessage.value);
        break;
      case "runProcessResponse":
        break;
    }
  }

  // Helper requests

  private async sendRequest(
    sessionId: string,
    clientbound: Clientbound["submessage"],
  ) {
    const packet = await clientboundToPacket({
      sessionId,
      submessage: clientbound,
    });
    this.packetStream.write(packet);
  }

  insertText(sessionId: string, request: InsertTextRequest): void {
    console.log("insertText");
    this.sendRequest(sessionId, {
      case: "insertText",
      value: request,
    });
  }

  intercept(sessionId: string, request: InterceptRequest): void {
    console.log("intercept");
    this.sendRequest(sessionId, {
      case: "intercept",
      value: request,
    });
  }

  runProcess(
    sessionId: string,
    request: RunProcessRequest,
  ): RunProcessResponse {
    this.sendRequest(sessionId, {
      case: "runProcess",
      value: request,
    });

    return create(RunProcessResponseSchema, {
      stdout: "",
      stderr: "",
      exitCode: 0,
    });
  }

  onEditBufferChange(callback: (notification: EditBufferHook) => void): void {
    this.emitter.on(EditBufferHookSymbol, () => {
      console.log(callback);
    });
  }

  onPrompt(callback: (notification: PromptHook) => void): void {
    this.emitter.on(PromptHookSymbol, callback);
  }

  onPreExec(callback: (notification: PreExecHook) => void): void {
    this.emitter.on(PreExecHookSymbol, callback);
  }

  onPostExec(callback: (notification: PostExecHook) => void): void {
    this.emitter.on(PostExecHookSymbol, callback);
  }

  onInterceptedKey(callback: (notification: InterceptedKeyHook) => void): void {
    this.emitter.on(InterceptedKeyHookSymbol, callback);
  }
}
