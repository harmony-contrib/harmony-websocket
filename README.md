# `@ohos-rs/websocket`

## Install

use`ohpm` to install package.

```shell
ohpm install @ohos-rs/websocket
```

## API

```ts
export interface WebSocketConfig {
  /** Custom cert file path */
  certPath?: string;
}

export declare class WebSocket {
  constructor(url: string, config?: WebSocketConfig | undefined | null);
  connect(): Promise<void>;
  send(data: string | ArrayBuffer): Promise<void>;
  close(): Promise<void>;
  ping(pingMessage?: ArrayBuffer | undefined | null): Promise<void>;
  onError(callback: () => void): void;
  onMessage(callback: (arg: string | ArrayBuffer) => void): void;
  onOpen(callback: () => void): void;
  onClose(callback: () => void): void;
  onPing(callback: (arg: ArrayBuffer) => ArrayBuffer | null): void;
  onPong(callback: (arg: ArrayBuffer) => void): void;
}
```

## Usage

### basic

```ts
const ws = new WebSocket("ws://127.0.0.1:8080");

ws.onOpen(() => {
  console.log("open");
  ws.send("hello");
});

ws.onMessage((data) => {
  console.log(data);
});

ws.connect();
```

### wss

We support wss protocol which is powered by `native-tls`. We support public CA certificate and self-signed certificate. If you want to use self-signed certificate, please provide self-signed cert file path.

An example here:

```ts
let context = getContext(this) as common.UIAbilityContext;
let filesDir = context.filesDir;

// Before use it, please make sure that file path exist.
const ws = new WebSocket("wss://127.0.0.1:9000", {
  certPath: filesDir + "./test.pem",
});
```

**Note: self-signed certificate must has SAN with IP**

You can generate self-signed certificate with the following bash:

```bash
openssl req -x509 -nodes -days 365 -newkey rsa:2048 \
  -keyout server.key -out server.crt \
  -subj "/CN=localhost" \
  -addext "subjectAltName = IP:$IP"
```

And use the following bash to verify ip address:

```bash
openssl x509 -in server.crt -text -noout | grep -A1 "Subject Alternative Name"
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
      console.log(a);
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


## How to build

We provide two methods to build:

1. Build with native-tls vendor mode.

2. Build with openssl prebuild mode.

**Note: For windows, please make sure that `OHOS_NDK_HOME` do not has space charactor**