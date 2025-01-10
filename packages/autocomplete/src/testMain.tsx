import { useEffect, useState } from "react";
import Autocomplete from "./Autocomplete";
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
  const [websocket, setWebsocket] = useState<WebsocktShim | undefined>();

  console.log("hi");

  useEffect(() => {
    const inner = new WebSocket("ws://localhost:8080");
    inner.onopen = () => console.log("ws opened");
    inner.onclose = () => console.log("ws closed");

    setWebsocket(new WebsocktShim(inner));

    return () => {
      inner.close();
    };
  }, []);

  return (
    <div className="">
      {websocket && (
        <Autocomplete
          ipcClient={{
            type: "",
            websocket,
          }}
        />
      )}
    </div>
  );
}
