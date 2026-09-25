use ::tokio::sync::mpsc::{UnboundedSender, error::SendError};
use serde_json::Value;
use wry::{RequestAsyncResponder, http::Request};

pub enum EmitterMessage {
    Http(String, Request<Vec<u8>>, RequestAsyncResponder),
    Ipc(Request<Vec<u8>>),
    Bytes(Vec<u8>),
    Json(Value),
}

#[derive(Clone)]
pub struct Emitter {
    tx: UnboundedSender<EmitterMessage>,
}

impl Emitter {
    pub(super) fn new(tx: UnboundedSender<EmitterMessage>) -> Self {
        Self { tx }
    }

    pub fn emit(&self, message: EmitterMessage) -> Result<(), SendError<EmitterMessage>> {
        self.tx.send(message)
    }
}
