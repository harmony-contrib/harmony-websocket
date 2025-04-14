use std::{collections::HashMap, fs::File, io::Read, sync::Arc};

use futures_util::{SinkExt, StreamExt};
use napi_derive_ohos::napi;
use napi_ohos::{
    bindgen_prelude::*,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Result,
};
use ohos_hilog_binding::hilog_error;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{
    connect_async_tls_with_config,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    Connector,
};

#[napi(object)]
pub struct WebSocketConfig {
    /// Custom cert file path
    pub cert_path: Option<String>,

    /// Custom headers
    pub headers: Option<HashMap<String, String>>,

    /// Enable websocket extensions.
    /// If enabled, the client will add `Sec-WebSocket-Extensions` header with `permessage-deflate; client_max_window_bits`
    pub enable_extension: Option<bool>,
}

#[napi]
pub struct WebSocket {
    url: String,
    config: Option<WebSocketConfig>,
    on_error: Option<Arc<ThreadsafeFunction<(), (), (), false>>>,
    on_message:
        Option<Arc<ThreadsafeFunction<Either<String, Buffer>, (), Either<String, Buffer>, false>>>,
    on_open: Option<Arc<ThreadsafeFunction<(), (), (), false>>>,
    on_close: Option<Arc<ThreadsafeFunction<(), (), (), false>>>,
    on_ping: Option<Arc<ThreadsafeFunction<Buffer, Option<Buffer>, Buffer, false>>>,
    on_pong: Option<Arc<ThreadsafeFunction<Buffer, (), Buffer, false>>>,
    writer: RwLock<Option<mpsc::Sender<Message>>>,
}

#[napi]
impl WebSocket {
    #[napi(constructor)]
    pub fn new(url: String, config: Option<WebSocketConfig>) -> Self {
        WebSocket {
            url,
            on_error: None,
            on_message: None,
            on_open: None,
            on_close: None,
            on_ping: None,
            on_pong: None,
            config: config,
            writer: RwLock::new(None),
        }
    }

    #[napi]
    pub async fn connect(&self) -> Result<()> {
        let mut connector: Option<Connector> = None;

        if let Some(config) = &self.config {
            if let Some(cert_path) = &config.cert_path {
                let mut cert_data = Vec::new();
                File::open(cert_path)
                    .map_err(|e| {
                        Error::new(
                            Status::GenericFailure,
                            format!("Try to open cert file path failed: {}", e.to_string()),
                        )
                    })?
                    .read_to_end(&mut cert_data)
                    .map_err(|e| {
                        Error::new(
                            Status::GenericFailure,
                            format!("Try to read cert file failed: {}", e.to_string()),
                        )
                    })?;
                let cert = native_tls::Certificate::from_pem(&cert_data).map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Try to parse cert file failed: {}", e.to_string()),
                    )
                })?;

                let mut builder = native_tls::TlsConnector::builder();
                builder.add_root_certificate(cert);

                let tls_connector = builder.build().map_err(|e| {
                    Error::new(
                        Status::GenericFailure,
                        format!("Try to build tls connector failed: {}", e.to_string()),
                    )
                })?;

                connector = Some(Connector::NativeTls(tls_connector));
            }
        }
        let mut request = (&self.url).into_client_request().map_err(|e| {
            Error::new(
                Status::GenericFailure,
                format!("Try to build request failed: {}", e.to_string()),
            )
        })?;

        let header = request.headers_mut();

        let custom_header = self.config.as_ref().and_then(|d| d.headers.clone());
        let enable_extension = self
            .config
            .as_ref()
            .and_then(|d| d.enable_extension.clone())
            .unwrap_or(false);

        if let Some(mut h) = custom_header {
            if enable_extension {
                h.insert(
                    "Sec-WebSocket-Extensions".to_string(),
                    "permessage-deflate; client_max_window_bits".to_string(),
                );
            }
            for (key, value) in h {
                // First parse the header name from the string
                let header_name =
                    match tokio_tungstenite::tungstenite::http::header::HeaderName::from_bytes(
                        key.as_bytes(),
                    ) {
                        Ok(name) => name,
                        Err(e) => {
                            return Err(Error::new(
                                Status::GenericFailure,
                                format!("Invalid header name '{}': {}", key, e),
                            ));
                        }
                    };

                // Then parse the header value
                match value.parse() {
                    Ok(header_value) => {
                        header.insert(header_name, header_value);
                    }
                    Err(e) => {
                        return Err(Error::new(
                            Status::GenericFailure,
                            format!("Invalid header value for key '{}': {}", key, e),
                        ));
                    }
                }
            }
        }

        let ws_stream = match connect_async_tls_with_config(request, None, false, connector).await {
            Ok((ws_stream, _)) => {
                if let Some(on_open) = &self.on_open {
                    on_open.call((), ThreadsafeFunctionCallMode::NonBlocking);
                }
                ws_stream
            }
            Err(e) => {
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("ws-rs connection failed: {}", e),
                ));
            }
        };

        let (mut write, read) = ws_stream.split();

        let (tx, mut rx) = mpsc::channel::<Message>(32);

        self.writer.write().await.replace(tx);

        let write_from_js = async move {
            while let Some(message) = rx.recv().await {
                write.send(message).await.unwrap();
            }
        };

        let read_from_ws = read.for_each(|message_result| {
            async move {
                match message_result {
                    Ok(message) => match message {
                        Message::Text(text) => {
                            if let Some(on_message) = &self.on_message {
                                on_message.call(
                                    Either::A(text.to_string()),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        Message::Binary(data) => {
                            if let Some(on_message) = &self.on_message {
                                let buf = data.iter().as_slice();
                                on_message.call(
                                    Either::B(Buffer::from(buf)),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        Message::Close(frame) => {
                            if let Some(frame) = frame {
                                if let Some(on_close) = &self.on_close {
                                    on_close.call((), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            } else {
                                if let Some(on_close) = &self.on_close {
                                    on_close.call((), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            }
                        }
                        Message::Ping(ping_message) => {
                            if let Some(on_ping) = &self.on_ping {
                                let buf = ping_message.iter().as_slice();
                                let pong_message = match on_ping.call_async(Buffer::from(buf)).await
                                {
                                    Ok(return_value) => {
                                        if let Some(pong_message) = return_value {
                                            let pong_buf = Vec::<u8>::from(pong_message);
                                            Message::Pong(pong_buf.into())
                                        } else {
                                            Message::Pong("pong".into())
                                        }
                                    }
                                    Err(e) => {
                                        hilog_error!(format!("ws-rs: onPing error: {}", e));
                                        Message::Pong("pong".into())
                                    }
                                };
                                let writer = self.writer.read().await;
                                if let Some(writer) = writer.as_ref() {
                                    writer.send(pong_message).await.unwrap();
                                }
                            }
                        }
                        Message::Pong(pong_message) => {
                            if let Some(on_pong) = &self.on_pong {
                                let buf = pong_message.iter().as_slice();
                                on_pong.call(
                                    Buffer::from(buf),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        _ => {} // 忽略其他类型的消息
                    },
                    Err(e) => {
                        if let Some(on_error) = &self.on_error {
                            on_error.call((), ThreadsafeFunctionCallMode::NonBlocking);
                        }
                    }
                }
            }
        });

        napi_ohos::tokio::select! {
          _ = read_from_ws => {},
          _ = write_from_js => {},
        }

        Ok(())
    }

    #[napi]
    pub async fn send(&self, data: Either<String, Buffer>) -> Result<()> {
        let writer = self.writer.read().await;
        if let Some(writer) = writer.as_ref() {
            let message = match data {
                Either::A(text) => Message::Text(text.into()),
                Either::B(buf) => {
                    let bytes = Vec::<u8>::from(buf);
                    Message::Binary(bytes.into())
                }
            };
            writer
                .send(message)
                .await
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        }
        Ok(())
    }

    #[napi]
    pub async fn close(&self) -> Result<()> {
        let writer = self.writer.read().await;
        if let Some(writer) = writer.as_ref() {
            writer
                .send(Message::Close(None))
                .await
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        }
        Ok(())
    }

    #[napi]
    pub async fn ping(&self, ping_message: Option<Buffer>) -> Result<()> {
        let ping_message = match ping_message {
            Some(buf) => {
                let bytes = Vec::<u8>::from(buf);
                bytes
            }
            None => "ping".into(),
        };

        if ping_message.len() > 128 {
            return Err(Error::new(
                Status::GenericFailure,
                "ping message length exceeds 128 bytes".to_string(),
            ));
        }
        let ping_message = Message::Ping(ping_message.into());

        let writer = self.writer.read().await;
        if let Some(writer) = writer.as_ref() {
            writer
                .send(ping_message)
                .await
                .map_err(|e| Error::new(Status::GenericFailure, e.to_string()))?;
        }
        Ok(())
    }

    #[napi]
    pub unsafe fn on_error(&mut self, callback: Function<(), ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_error = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_message(
        &mut self,
        callback: Function<Either<String, Buffer>, ()>,
    ) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_message = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_open(&mut self, callback: Function<(), ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_open = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_close(&mut self, callback: Function<(), ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_close = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_ping(&mut self, callback: Function<Buffer, Option<Buffer>>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_ping = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_pong(&mut self, callback: Function<Buffer, ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<false>()
            .build()?;
        self.on_pong = Some(Arc::new(callback));
        Ok(())
    }
}
