# `@ohos-rs/websocket`

## Install

use`ohpm` to install package.

```shell
ohpm install @ohos-rs/websocket
```

## API

```ts
export declare class WebSocket {
  constructor(url: string)
  connect(): Promise<void>
  send(data: string | ArrayBuffer): Promise<void>
  close(): Promise<void>
  ping(): Promise<void>
  onError(callback: () => void): void
  onMessage(callback: (arg: string | ArrayBuffer) => void): void
  onOpen(callback: () => void): void
  onClose(callback: () => void): void
}
```

## Usage

```ts
const ws = new WebSocket('ws://127.0.0.1:8080')

ws.onOpen(() => {
  console.log('open')
  ws.send('hello')
})

ws.onMessage((data) => {
  console.log(data)
})

ws.connect()
```

## Server Example

We provide a simple server example to help you get started which use `uWebSockets.js`.

> **Why not socket.io?**
> socket.io is a good choice, but it is not suitable for mobile devices. See details [here](https://socket.io/docs/v4/#what-socketio-is-not)

```js
const uWS = require("uWebSockets.js");
const port = 9001;

let wsInstance;

const app = uWS
  .App({})
  .ws("/*", {
    /* Options */
    compression: uWS.SHARED_COMPRESSOR,
    maxPayloadLength: 16 * 1024 * 1024,
    idleTimeout: 10,
    /* Handlers */
    open: (ws) => {
      wsInstance = ws;
      console.log("A WebSocket connected!");
    },
    message: (ws, message, isBinary) => {
      /* Ok is false if backpressure was built up, wait for drain */
      let ok = ws.send(message, isBinary);
    },
    drain: (ws) => {
      console.log("WebSocket backpressure: " + ws.getBufferedAmount());
    },
    close: (ws, code, message) => {
      console.log("WebSocket closed");
    },
  })
  .get("/ping", (res, req) => {
    if (wsInstance) {
      let a = wsInstance.ping("ping");
      console.log(a)
    }
    res.end("pong");
  })
  .any("/*", (res, req) => {
    res.end("Nothing to see here!");
  })
  .listen(port, (token) => {
    if (token) {
      console.log(token);
      console.log("Listening to port " + port);
    } else {
      console.log("Failed to listen to port " + port);
    }
  });

```
