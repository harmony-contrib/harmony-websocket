pub enum WebSocketError {
    TlsError,
    HeaderError,
    ConnectError,
    SendError,
    ReceiveError,
    CloseError,
}

impl AsRef<str> for WebSocketError {
    fn as_ref(&self) -> &str {
        match self {
            WebSocketError::TlsError => "TlsError",
            WebSocketError::HeaderError => "HeaderError",
            WebSocketError::ConnectError => "ConnectError",
            WebSocketError::SendError => "SendError",
            WebSocketError::ReceiveError => "ReceiveError",
            WebSocketError::CloseError => "CloseError",
        }
    }
}
