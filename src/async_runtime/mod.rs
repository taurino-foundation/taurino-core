pub mod emitter;
pub mod source;

use std::{
    sync::{Arc, Mutex, mpsc::sync_channel},
    thread::{JoinHandle, spawn},
};

use anyhow::{Context, Result};
use serde_json::Value;
use wry::WebViewBuilder;

use crate::{
    LaunchInfo,
    async_runtime::{
        emitter::{Emitter, EmitterMessage, WireMessage},
        source::ProtocolSystem,
    },
    runner::RuntimeLoopProxy,
    schema::webview::WebviewUrl,
};

pub struct IPCRuntime {
    emitter: Emitter,
    protocol: Arc<ProtocolSystem>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl IPCRuntime {
    pub fn new(config: &LaunchInfo, proxy: RuntimeLoopProxy) -> Result<Arc<Self>> {
        let (emitter, mut rx) = Emitter::new();
        let config = config.clone();

        let frontend_dist = config.frontend_dist();
        let protocol_options = config.protocol_options();
        let config = config.clone();

        /*
         * Startup channel:
         *
         * Der GUI-Thread wartet nur darauf, dass das ProtocolSystem
         * innerhalb des Tokio-Threads fertig initialisiert wurde.
         *
         * Es wird KEIN tokio_handle.block_on() im GUI-Thread verwendet.
         */
        let (protocol_tx, protocol_rx) = sync_channel::<Result<Arc<ProtocolSystem>>>(1);

        let join =
            spawn(move || {
                let _config = config;
                let _proxy = proxy;

                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt,

                    Err(error) => {
                        let _ = protocol_tx.send(Err(anyhow::anyhow!(
                            "failed to build Tokio runtime: {error}"
                        )));

                        return;
                    }
                };

                /*
                 * ProtocolSystem speichert den Handle selbst.
                 *
                 * Wichtig:
                 * Der Handle gehört genau zu der Runtime, die darunter
                 * mit block_on betrieben wird.
                 */
                let runtime_handle = rt.handle().clone();

                rt.block_on(async move {
                    let protocol =
                        match ProtocolSystem::new(frontend_dist, runtime_handle, protocol_options)
                            .await
                        {
                            Ok(protocol) => Arc::new(protocol),

                            Err(error) => {
                                let _ = protocol_tx.send(Err(error));
                                return;
                            }
                        };

                    /*
                     * Ab jetzt darf der GUI-Thread Fenster bauen und
                     * IPCRuntime::apply() benutzen.
                     */
                    if protocol_tx.send(Ok(Arc::clone(&protocol))).is_err() {
                        // Der Empfänger existiert nicht mehr.
                        // Dann muss auch die Runtime nicht weiterlaufen.
                        return;
                    }

                    /*
                     * Tokio bleibt danach am Leben und verarbeitet
                     * deine Runtime-Nachrichten.
                     */
                    while let Some(msg) = rx.recv().await {
                        match msg {
                            EmitterMessage::Http(items, _responder, _request_id) => {
                                /*
                                 * Falls Http wirklich JSON enthält.
                                 *
                                 * Wenn du hier MessagePack sendest,
                                 * musst du entsprechend rmp_serde benutzen.
                                 */
                                match serde_json::from_slice::<Value>(&items) {
                                    Ok(value) => {
                                        println!("http payload: {value}");
                                    }

                                    Err(error) => {
                                        eprintln!("failed to decode HTTP payload: {error:#}");
                                    }
                                }
                            }

                            EmitterMessage::MsgPack(items) => {
                                match rmp_serde::from_slice::<WireMessage<Value>>(&items) {
                                    Ok(msg) => {
                                        println!("kind = {}, payload = {}", msg.kind, msg.payload);
                                    }

                                    Err(error) => {
                                        eprintln!("failed to decode msgpack payload: {error:#}");
                                    }
                                }
                            }

                            EmitterMessage::Shutdown => {
                                break;
                            }
                        }
                    }

                    /*
                     * `protocol` wird hier erst zerstört, nachdem
                     * die Message-Loop beendet wurde.
                     *
                     * Der GUI-Thread besitzt zusätzlich seinen Arc.
                     */
                    drop(protocol);
                });
            });

        /*
         * Das ist nur std::sync::mpsc::Receiver::recv().
         *
         * Kein Tokio block_on.
         *
         * new() kehrt erst zurück, wenn ProtocolSystem wirklich
         * einsatzbereit ist oder seine Initialisierung fehlgeschlagen ist.
         */
        let protocol = protocol_rx
            .recv()
            .context("Tokio thread terminated before ProtocolSystem initialization finished")??;

        Ok(Arc::new(Self {
            emitter,
            protocol,
            join: Mutex::new(Some(join)),
        }))
    }

    pub fn emitter(&self) -> Result<Emitter> {
        Ok(self.emitter.clone())
    }

    /// Konfiguriert einen WRY WebViewBuilder.
    ///
    /// Diese Funktion läuft synchron im GUI-/Tao-Thread.
    pub fn apply<'a>(
        &self,
        builder: WebViewBuilder<'a>,
        webview_url: &WebviewUrl,
    ) -> Result<WebViewBuilder<'a>> {
        self.protocol.apply(builder, webview_url)
    }

    /// Falls andere Manager direkten Zugriff auf das ProtocolSystem benötigen.
    pub fn protocol(&self) -> &Arc<ProtocolSystem> {
        &self.protocol
    }

    /// Konsumiert IPCRuntime und gibt den Thread-Handle zurück.
    ///
    /// Das ist nur sinnvoll, wenn IPCRuntime danach nicht mehr
    /// für `apply()` gebraucht wird.
    pub fn take_join_handle(&self) -> Result<Option<JoinHandle<()>>> {
        let mut join = self
            .join
            .lock()
            .map_err(|_| anyhow::anyhow!("IPCRuntime join handle mutex poisoned"))?;

        Ok(join.take())
    }
}
