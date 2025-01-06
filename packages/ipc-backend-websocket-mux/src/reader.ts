import { Packet, PacketSchema } from "@aws/amazon-q-developer-cli-proto/mux";
import { Socket } from "./socket.js";
import { fromBinary } from "@bufbuild/protobuf";
import Emittery from "emittery";

export class PacketReader {
  private buffer: string;
  private emitter: Emittery;

  constructor(socket: Socket) {
    this.buffer = "";
    this.emitter = new Emittery();

    socket.onMessage((data) => {
      if (typeof data === "string") {
        this.buffer += data;
        this.parse();
      }
    });
  }

  onPacket(listener: (packet: Packet) => void | Promise<void>) {
    this.emitter.on("packet", listener);
  }

  parse() {
    // Keep trying to parse while we have data
    while (this.buffer.length > 0) {
      const result = parseBase64Line(this.buffer);

      switch (result.type) {
        case "success": {
          // Enqueue the parsed value and remove consumed characters
          this.emitter.emit("packet", result.value);
          this.buffer = this.buffer.slice(result.charsConsumed);
          break;
        }
        case "needs_more": {
          // Not enough data yet, wait for more
          if (result.minimumCharsNeeded > this.buffer.length) {
            return;
          }
          // If we have enough chars but parse still failed, there might be an issue
          console.error(
            "Parser reported needs more characters but buffer contains requested amount",
          );
          return;
        }
        case "error": {
          // Forward parse errors to stream consumer
          console.error(result.error);
          return;
        }
      }
    }
  }
}

// Transform stream to handle parsing of incoming bytes
// Define possible parse results
type ParseSuccess<T> = {
  type: "success";
  value: T;
  charsConsumed: number;
};

type ParseNeedsMore = {
  type: "needs_more";
  minimumCharsNeeded: number;
};

type ParseError = {
  type: "error";
  error: Error;
};

type ParseResult<T> = ParseSuccess<T> | ParseNeedsMore | ParseError;

function parseBase64Line(input: string): ParseResult<Packet> {
  // Look for either CRLF or LF
  const crlfIndex = input.indexOf("\r\n");
  const lfIndex = input.indexOf("\n");

  // Determine which line ending comes first (if any)
  let lineEndIndex: number;
  let lineEndLength: number;

  if (crlfIndex !== -1 && (lfIndex === -1 || crlfIndex < lfIndex)) {
    // CRLF comes first
    lineEndIndex = crlfIndex;
    lineEndLength = 2;
  } else if (lfIndex !== -1) {
    // LF comes first
    lineEndIndex = lfIndex;
    lineEndLength = 1;
  } else {
    // No line ending found yet
    return {
      type: "needs_more",
      minimumCharsNeeded: input.length + 1,
    };
  }

  // Extract the base64 string (excluding line ending)
  const base64Str = input.slice(0, lineEndIndex);

  // Validate base64 string
  if (!isValidBase64(base64Str)) {
    return {
      type: "error",
      error: new Error("Invalid base64 string"),
    };
  }

  try {
    // Decode base64 to Uint8Array
    const binaryStr = atob(base64Str);
    const bytes = new Uint8Array(binaryStr.length);
    for (let i = 0; i < binaryStr.length; i++) {
      bytes[i] = binaryStr.charCodeAt(i);
    }

    const packet = fromBinary(PacketSchema, bytes);

    return {
      type: "success",
      value: packet,
      charsConsumed: lineEndIndex + lineEndLength,
    };
  } catch (error) {
    return {
      type: "error",
      error: new Error(`Failed to decode line: ${error}`),
    };
  }
}

// Helper function to validate base64 string
function isValidBase64(str: string): boolean {
  // Check if string matches base64 pattern
  // Allow padding at the end (=), must be multiple of 4 chars
  const base64Regex = /^[A-Za-z0-9+/]*={0,2}$/;
  return base64Regex.test(str) && str.length % 4 === 0;
}
