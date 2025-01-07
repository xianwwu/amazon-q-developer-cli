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
        console.log("message", message.data);
        this.emitter.emit("message2", message);
      }
    };
  }

  public on(event: "message2", listener: (data: { data: string }) => void) {
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
  const [ws, setWs] = useState<WebsocktShim | undefined>();

  console.log("hi");

  useEffect(() => {
    const inner = new WebSocket("ws://localhost:8080");
    inner.onopen = () => console.log("ws opened");
    inner.onclose = () => console.log("ws closed");

    setWs(new WebsocktShim(inner));

    return () => {
      inner.close();
    };
  }, []);

  return (
    <div className="">
      {ws && (
        <Autocomplete
          ipcBackend={{
            type: "",
            websocket: ws,
          }}
          // enableMocks={true}
        />
      )}
    </div>
  );
}
