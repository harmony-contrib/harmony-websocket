use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use napi_derive_ohos::napi;
use napi_ohos::{
    bindgen_prelude::*,
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode},
    Result,
};
use ohos_hilog_binding::hilog_info;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[napi]
pub struct WebSocket {
    url: String,
    on_error: Option<Arc<ThreadsafeFunction<(), ()>>>,
    on_message: Option<Arc<ThreadsafeFunction<Either<String, Buffer>, ()>>>,
    on_open: Option<Arc<ThreadsafeFunction<(), ()>>>,
    on_close: Option<Arc<ThreadsafeFunction<(), ()>>>,
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
            writer: RwLock::new(None),
        }
    }

    #[napi]
    pub async fn connect(&self) -> Result<()> {
        let url = self.url.clone();

        let ws_stream = match connect_async(&url).await {
            Ok((ws_stream, _)) => {
                hilog_info!(format!("ws-rs connect success"));
                ws_stream
            }
            Err(e) => {
                let err_msg = format!("连接错误: {}", e);
                hilog_info!(format!("ws-rs connect error: {}", err_msg));
                return Err(Error::new(
                    Status::GenericFailure,
                    format!("连接错误: {}", e),
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

        // 处理从WebSocket接收的消息
        let read_from_ws = read.for_each(|message_result| {
            async move {
                match message_result {
                    Ok(message) => match message {
                        Message::Text(text) => {
                            hilog_info!(format!("ws-rs text data: {}", text));
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
                                hilog_info!(format!(
                                    "ws-rs: {} {}",
                                    frame.code,
                                    frame.reason.to_string()
                                ));
                                if let Some(on_close) = &self.on_close {
                                    on_close.call(Ok(()), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            } else {
                                hilog_info!("ws-rs: 正常关闭连接");
                                if let Some(on_close) = &self.on_close {
                                    on_close.call(Ok(()), ThreadsafeFunctionCallMode::NonBlocking);
                                }
                            }
                        }
                        _ => {} // 忽略其他类型的消息
                    },
                    Err(e) => {
                        let err_msg = format!("ws-rs:接收消息错误: {}", e);
                        hilog_info!(err_msg);

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
    pub async fn ping(&self) -> Result<()> {
        let writer = self.writer.read().await;
        if let Some(writer) = writer.as_ref() {
            writer
                .send(Message::Ping("ping".into()))
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
}
