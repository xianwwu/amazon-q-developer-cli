import React, { useRef } from "react";
import Autocomplete, { AUTOCOMPLETE_CONNECTION_TYPES } from "./Autocomplete";
import Emittery from "emittery";

class WebsocktShim {
  private emitter: Emittery = new Emittery();

  constructor(private websocket: WebSocket) {
    websocket.onmessage = async (event) => {
      if (event.data instanceof Blob) {
        const message = {
          ...event,
          data: await event.data.text(),
        };
        this.emitter.emit("autocompleteMessage", message);
      }
    };
  }

  public on(
    event: "autocompleteMessage",
    listener: (data: { data: string }) => void,
  ) {
    this.emitter.on(event, listener);
    return this;
  }

  public send(data: string): void {
    this.websocket.send(data);
  }

  public close() {
    this.websocket.close();
  }
}

export function Test() {
  const websocket = useRef<WebsocktShim>();
  if (!websocket.current) {
    const inner = new WebSocket("ws://localhost:8080");
    inner.onopen = () => console.log("ws opened");
    inner.onclose = () => console.log("ws closed");
    websocket.current = new WebsocktShim(inner);
  }

  return (
    <div className="">
      {websocket && (
        <Autocomplete
          ipcClient={{
            type: AUTOCOMPLETE_CONNECTION_TYPES.CS_WEBSOCKET,
            websocket: websocket.current,
          }}
          onDisconnect={() => {
            console.error("DISCONNECT!");
          }}
        />
      )}
    </div>
  );
}
