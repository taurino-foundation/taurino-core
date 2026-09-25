use std::sync::Arc;

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};
use taurino_core::prelude::*;
use url::Url;

fn main() -> crate::Result<()> {
    let event_loop = EventLoop::new();

    // -------------------------------------------------------------------------
    // Window
    // -------------------------------------------------------------------------
    let web_context: WebContextStore = Default::default();
    let window_builder = WindowBuilder::new()
        .add_webview_builder(
            WebViewBuilder::new().with_url(WebviewUrl::External(Url::parse("https://example.com").unwrap())),
        )
        .title("Tao + Wry")
        .center()
        .inner_size(800.0, 600.0)
        .on_window_event(|metadata, event| {
            println!(
                "window={} webview={} event={event:?}",
                metadata.window_label, metadata.webview_label,
            );
        })
        .with_close_requested(|sender| {
            // true  = Close verhindern
            // false = Close erlauben
            let _ = sender.send(false);
        });

    let window = window_builder.build(
        &event_loop,
        1.into(),
        web_context.clone(),
        None::<fn(wry::WebViewBuilder<'_>, WebviewUrl) -> crate::Result<wry::WebViewBuilder<'_>>>,
    )?;

    let window = Arc::new(window);
    let webview = window.webview("root")?;

    // -------------------------------------------------------------------------
    // WebContext
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // WebView
    // -------------------------------------------------------------------------

    // Beispiele für deine neuen direkten Methoden.
    webview.set_zoom(1.0)?;

    println!("current url: {}", webview.url()?);

    let cookies = webview.cookies()?;

    for cookie in cookies {
        println!("cookie: {} = {}", cookie.name(), cookie.value(),);
    }

    // -------------------------------------------------------------------------
    // Tao event loop
    // -------------------------------------------------------------------------
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent { event, window_id, .. } if window_id == window.id() => {
                window.emit_window_event(&event);

                match event {
                    WindowEvent::CloseRequested => match window.close() {
                        Ok(true) => {
                            *control_flow = ControlFlow::Exit;
                        }

                        Ok(false) => {}

                        Err(error) => {
                            eprintln!("close error: {error}");
                        }
                    },

                    WindowEvent::Resized(size) => {
                        println!("resized: {size:?}");
                    }

                    _ => {}
                }
            }

            Event::LoopDestroyed => {
                println!("event loop destroyed");
            }

            _ => {}
        }
    });
}
