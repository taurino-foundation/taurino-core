use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use wry::RequestAsyncResponder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WireMessage<'a, P: Serialize> {
    #[serde(rename = "type")]
    pub kind: &'a str,
    pub payload: P,
}

pub enum EmitterMessage {
    Http(Vec<u8>, RequestAsyncResponder, u32),
    MsgPack(Vec<u8>),

    /// Requests termination of the background Tokio runtime.
    Shutdown,
}

#[derive(Clone)]
pub struct Emitter {
    tx: UnboundedSender<EmitterMessage>,
    next_id: Arc<AtomicU32>,
}

impl Emitter {
    pub fn new() -> (Self, UnboundedReceiver<EmitterMessage>) {
        let (tx, rx) = unbounded_channel();

        (
            Self {
                tx,
                next_id: Default::default(),
            },
            rx,
        )
    }

    fn next_req_id(&self) -> u32 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    fn emit(&self, msg: EmitterMessage) -> Result<()> {
        self.tx
            .send(msg)
            .map_err(|e| anyhow::anyhow!("Emitter closed: {e}"))
    }

    fn send_msg_pack<E, P>(&self, action: E, payload: P) -> Result<()>
    where
        E: Into<&'static str>,
        P: Serialize,
    {
        let msg = WireMessage {
            kind: action.into(),
            payload,
        };

        let bytes = rmp_serde::to_vec(&msg)?;

        self.emit(EmitterMessage::MsgPack(bytes))
    }

    pub fn send_event<P: Serialize>(&self, payload: P) -> Result<()> {
        self.send_msg_pack("event", payload)
    }

    pub fn send_ipc<P: Serialize>(&self, payload: P) -> Result<()> {
        self.send_msg_pack("ipc", payload)
    }

    pub fn send_api_result<P: Serialize>(&self, payload: P) -> Result<()> {
        self.send_msg_pack("api_result", payload)
    }

    pub fn send_http<P: Serialize>(&self, req: P, responder: RequestAsyncResponder) -> Result<()> {
        let bytes = rmp_serde::to_vec(&req)?;
        let id = self.next_req_id();

        self.emit(EmitterMessage::Http(bytes, responder, id))
    }

    /// Requests a graceful shutdown of the background Tokio runtime.
    pub fn shutdown(&self) -> Result<()> {
        self.emit(EmitterMessage::Shutdown)
    }

    pub fn is_connected(&self) -> bool {
        !self.tx.is_closed()
    }
}
