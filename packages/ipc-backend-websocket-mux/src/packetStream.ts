import { Packet } from "@aws/amazon-q-developer-cli-proto/mux";
import { PacketReader } from "./reader.js";
import { Socket } from "./socket.js";
import { PacketWriter } from "./writer.js";

export class PacketStream {
  private readable: PacketReader;
  private writable: PacketWriter;

  constructor(socket: Socket) {
    this.readable = new PacketReader(socket);
    this.writable = new PacketWriter(socket);
  }

  onPacket(listener: (packet: Packet) => void | Promise<void>) {
    this.readable.onPacket(listener);
  }

  write(packet: Packet) {
    this.writable.write(packet);
  }
}
