use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use napi_derive_ohos::napi;
use napi_ohos::{
    bindgen_prelude::*,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Result,
};
use ohos_hilog_binding::hilog_error;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[napi]
pub struct WebSocket {
    url: String,
    on_error: Option<Arc<ThreadsafeFunction<(), ()>>>,
    on_message: Option<Arc<ThreadsafeFunction<Either<String, Buffer>, ()>>>,
    on_open: Option<Arc<ThreadsafeFunction<(), ()>>>,
    on_close: Option<Arc<ThreadsafeFunction<(), ()>>>,
    on_ping: Option<Arc<ThreadsafeFunction<Buffer, Option<Buffer>>>>,
    on_pong: Option<Arc<ThreadsafeFunction<Buffer, ()>>>,
    writer: RwLock<Option<mpsc::Sender<Message>>>,
}

#[napi]
impl WebSocket {
    #[napi(constructor)]
    pub fn new(url: String) -> Self {
        WebSocket {
            url,
            on_error: None,
            on_message: None,
            on_open: None,
            on_close: None,
            on_ping: None,
            on_pong: None,
            writer: RwLock::new(None),
        }
    }

    #[napi]
    pub async fn connect(&self) -> Result<()> {
        let ws_stream = match connect_async(&self.url).await {
            Ok((ws_stream, _)) => ws_stream,
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
                                    Ok(Either::A(text.to_string())),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        Message::Binary(data) => {
                            if let Some(on_message) = &self.on_message {
                                let buf = data.iter().as_slice();
                                on_message.call(
                                    Ok(Either::B(Buffer::from(buf))),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        Message::Close(frame) => {
                            if let Some(frame) = frame {
                                if let Some(on_close) = &self.on_close {
                                    on_close.call(Ok(()), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            } else {
                                if let Some(on_close) = &self.on_close {
                                    on_close.call(Ok(()), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            }
                        }
                        Message::Ping(ping_message) => {
                            if let Some(on_ping) = &self.on_ping {
                                let buf = ping_message.iter().as_slice();
                                let pong_message =
                                    match on_ping.call_async(Ok(Buffer::from(buf))).await {
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
                                    Ok(Buffer::from(buf)),
                                    ThreadsafeFunctionCallMode::NonBlocking,
                                );
                            }
                        }
                        _ => {} // 忽略其他类型的消息
                    },
                    Err(e) => {
                        if let Some(on_error) = &self.on_error {
                            on_error.call(Ok(()), ThreadsafeFunctionCallMode::NonBlocking);
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
            .callee_handled::<true>()
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
            .callee_handled::<true>()
            .build()?;
        self.on_message = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_open(&mut self, callback: Function<(), ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<true>()
            .build()?;
        self.on_open = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_close(&mut self, callback: Function<(), ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<true>()
            .build()?;
        self.on_close = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_ping(&mut self, callback: Function<Buffer, Option<Buffer>>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<true>()
            .build()?;
        self.on_ping = Some(Arc::new(callback));
        Ok(())
    }

    #[napi]
    pub unsafe fn on_pong(&mut self, callback: Function<Buffer, ()>) -> Result<()> {
        let callback = callback
            .build_threadsafe_function()
            .callee_handled::<true>()
            .build()?;
        self.on_pong = Some(Arc::new(callback));
        Ok(())
    }
}
