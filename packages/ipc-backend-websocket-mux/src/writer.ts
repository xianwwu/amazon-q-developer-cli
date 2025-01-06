import { Packet, PacketSchema } from "@aws/amazon-q-developer-cli-proto/mux";
import { Socket } from "./socket.js";
import { toBinary } from "@bufbuild/protobuf";

export class PacketWriter {
  private socket: Socket;

  constructor(socket: Socket) {
    this.socket = socket;
  }

  write(packet: Packet) {
    const bytes = toBinary(PacketSchema, packet);
    const base64 = btoa(String.fromCharCode(...bytes));
    this.socket.send(base64 + "\n");
  }
}
