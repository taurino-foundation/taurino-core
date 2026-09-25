use std::{
    future::Future,
    sync::{mpsc as std_mpsc, Arc},
    thread::{self, Builder as ThreadBuilder, JoinHandle},
};

use ::tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{
        mpsc::{self, UnboundedReceiver},
        oneshot,
    },
};
use anyhow::{anyhow, Context, Result};
use wry::WebViewBuilder;

use crate::utils::{FrontendDist, WebviewUrl};

pub mod emitter;
pub mod source;

pub use self::emitter::{Emitter, EmitterMessage};
use self::source::{ProtocolOptions, ProtocolSystem};

pub struct TokioRuntime {
    emitter: Emitter,
    protocol: Arc<ProtocolSystem>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    join: Option<JoinHandle<()>>,
}

impl TokioRuntime {
    /// Startet Tokio und initialisiert das ProtocolSystem im eigenen Thread.
    ///
    /// Wartet synchron auf die Protokoll-Initialisierung. Der Handler erhält
    /// danach einmalig den Receiver und das kooperative Shutdown-Signal.
    /// Sein Server-/Clientstart ist nicht Teil dieser Startbestätigung.
    pub fn new<F, Fut>(
        frontend_dist: FrontendDist,
        protocol_options: ProtocolOptions,
        handler: F,
    ) -> Result<Self>
    where
        F: FnOnce(UnboundedReceiver<EmitterMessage>, oneshot::Receiver<()>) -> Fut
            + Send
            + 'static,
        Fut: Future<Output = ()> + 'static,
    {
        let (tx, rx) = mpsc::unbounded_channel::<EmitterMessage>();
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Ein Kanal für beide Startphasen: Runtime und ProtocolSystem.
        let (protocol_tx, protocol_rx) =
            std_mpsc::sync_channel::<Result<Arc<ProtocolSystem>>>(1);

        let join = ThreadBuilder::new()
            .name("tokio-runtime".to_owned())
            .spawn(move || {
                // Das vorhandene ProtocolSystem verlangt diesen Runtime-Typ.
                let runtime = match RuntimeBuilder::new_multi_thread()
                    .enable_all()
                    .build()
                    .context("Tokio-Runtime konnte nicht erstellt werden")
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        let _ = protocol_tx.send(Err(error));
                        return;
                    }
                };

                let runtime_handle = runtime.handle().clone();

                runtime.block_on(async move {
                    let protocol = match ProtocolSystem::new(
                        frontend_dist,
                        runtime_handle,
                        protocol_options,
                    )
                    .await
                    .context("ProtocolSystem konnte nicht initialisiert werden")
                    {
                        Ok(protocol) => Arc::new(protocol),
                        Err(error) => {
                            let _ = protocol_tx.send(Err(error));
                            return;
                        }
                    };

                    // Erst jetzt darf new() erfolgreich zurückkehren.
                    if protocol_tx.send(Ok(Arc::clone(&protocol))).is_err() {
                        return;
                    }
                    drop(protocol_tx);

                    // Keine Empfangsschleife hier: Der Handler übernimmt sie.
                    handler(rx, shutdown_rx).await;

                    // Unsere Protokoll-Referenz bleibt bis zum Handlerende erhalten.
                    drop(protocol);
                });

                // Danach wird die Tokio-Runtime auf diesem Thread abgebaut.
            })
            .context("Tokio-Thread konnte nicht gestartet werden")?;

        // Blockierendes std-recv(), aber kein Tokio-block_on() im GUI-Thread.
        let startup_result = protocol_rx
            .recv()
            .context("Tokio-Thread vor Abschluss der Protokoll-Initialisierung beendet")
            .and_then(|result| result);

        let protocol = match startup_result {
            Ok(protocol) => protocol,
            Err(error) => {
                // Bei fehlgeschlagenem Start den Thread nicht unbemerkt ablösen.
                if join.join().is_err() {
                    return Err(error.context("Panic im Tokio-Thread während des Starts"));
                }
                return Err(error);
            }
        };

        Ok(Self {
            emitter: Emitter::new(tx),
            protocol,
            shutdown_tx: Some(shutdown_tx),
            join: Some(join),
        })
    }

    pub fn emitter(&self) -> Emitter {
        self.emitter.clone()
    }

    /// Konfiguriert einen WebViewBuilder synchron im GUI-Thread.
    pub fn apply<'a>(
        &self,
        builder: WebViewBuilder<'a>,
        webview_url: &WebviewUrl,
    ) -> Result<WebViewBuilder<'a>> {
        self.protocol.apply(builder, webview_url)
    }

    pub fn protocol(&self) -> &Arc<ProtocolSystem> {
        &self.protocol
    }

    /// Signalisiert das Beenden, ohne den Handler abzubrechen oder abzuwarten.
    pub fn request_shutdown(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }

    /// Signalisiert das Beenden und wartet auf Handler und Runtime-Abbau.
    ///
    /// Blockiert. Nicht aus dem Handler oder einer Task dieser Runtime aufrufen.
    /// Der Handler muss das Shutdown-Signal verarbeiten und zurückkehren.
    pub fn shutdown(mut self) -> Result<()> {
        self.request_shutdown();

        if let Some(join) = self.join.take() {
            if join.thread().id() == thread::current().id() {
                return Err(anyhow!(
                    "Der Tokio-Thread kann nicht auf sich selbst warten"
                ));
            }

            join.join()
                .map_err(|_| anyhow!("Der Tokio-Thread wurde durch eine Panic beendet"))?;
        }

        Ok(())
    }
}

impl Drop for TokioRuntime {
    fn drop(&mut self) {
        // Nur signalisieren. Das Fallenlassen des JoinHandles wartet nicht.
        self.request_shutdown();
    }
}
