const uWS = require("uWebSockets.js");
const port = 9001;

let wsInstance;

const app = uWS
  .SSLApp({
    key_file_name: "../misc/server.key",
    cert_file_name: "../misc/server.crt",
    passphrase: "1234",
  })
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
