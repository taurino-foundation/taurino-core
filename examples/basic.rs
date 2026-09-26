use std::sync::Arc;

use taurino_core::native::{
    tao::{
        event::{ElementState, Event, MouseButton, WindowEvent},
        event_loop::{ControlFlow, EventLoopBuilder},
    },
    wry,
};
use taurino_core::prelude::*;
use url::Url;

// -----------------------------------------------------------------------------
// User event: bridges muda's internal thread back into the tao event loop.
// -----------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum UserEvent {
    MenuClicked(String), // item id
}

fn main() -> Result<()> {
    // -------------------------------------------------------------------------
    // Event loop
    // -------------------------------------------------------------------------

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // -------------------------------------------------------------------------
    // muda menu events -> Tao user events
    //
    // Der muda-Handler kann auf einem anderen Thread laufen.
    // Deshalb wird nur Send-fähige Information in den Tao EventLoop übertragen.
    // -------------------------------------------------------------------------

    install_menu_event_handler({
        let proxy = proxy.clone();

        move |item_id: String| {
            let _ = proxy.send_event(UserEvent::MenuClicked(item_id));
        }
    });

    // -------------------------------------------------------------------------
    // WebContext
    // -------------------------------------------------------------------------

    let web_context: WebContextStore = Default::default();

    // -------------------------------------------------------------------------
    // WindowBuilder
    //
    // Das Menü wird jetzt direkt im Builder gespeichert.
    // `with_menu` speichert einen SetupMenu:
    //
    // Box<
    //     dyn Fn(&Window) -> Result<WindowMenu>
    //         + Send
    //         + 'static
    // >
    //
    // create_window() führt den Callback aus, nachdem das native Tao-Fenster
    // erfolgreich erstellt wurde.
    // -------------------------------------------------------------------------

    let window_builder = WindowBuilder::new()
        .theme(Some(Theme::Dark))
        .add_webview_builder(
            WebViewBuilder::new().with_url(Some(WebviewUrl::External(Url::parse("https://tauri.app").unwrap()))),
        )
        .title("Tao + Wry + Menu")
        .center()
        .inner_size(800.0, 600.0)
        .on_window_event(|metadata, event| {
            println!(
                "window={} webview={} event={event:?}",
                metadata.window_label, metadata.webview_label,
            );
        })
        .with_close_requested(|sender| {
            let _ = sender.send(false);
        });

    let theme = window_builder.get_theme();

    // -------------------------------------------------------------------------
    // Build
    //
    // SetupMenu ist kein build()-Argument mehr.
    // Er befindet sich bereits im WindowBuilder.
    // -------------------------------------------------------------------------

    let window = window_builder.build(
        &event_loop,
        1.into(),
        web_context.clone(),
        None::<fn(wry::WebViewBuilder<'_>, WebviewUrl) -> crate::Result<wry::WebViewBuilder<'_>>>,
        Some(move |raw_window: RawWindow| -> Result<WindowMenu> {
            let file_menu = Submenu::with_id_and_items(
                "file",
                "File",
                true,
                &[
                    &MenuItem::with_id("file.open", "Open", true, Some("Ctrl+O"))?,
                    &MenuItem::with_id("file.save", "Save", true, Some("Ctrl+S"))?,
                    &PredefinedMenuItem::separator()?,
                    &PredefinedMenuItem::close_window(None)?,
                    &PredefinedMenuItem::quit(None)?,
                ],
            )?;

            let edit_menu = Submenu::with_id_and_items(
                "edit",
                "Edit",
                true,
                &[
                    &PredefinedMenuItem::undo(None)?,
                    &PredefinedMenuItem::redo(None)?,
                    &PredefinedMenuItem::separator()?,
                    &PredefinedMenuItem::cut(None)?,
                    &PredefinedMenuItem::copy(None)?,
                    &PredefinedMenuItem::paste(None)?,
                    &PredefinedMenuItem::select_all(None)?,
                ],
            )?;

            let dark_mode = CheckMenuItem::with_id("view.dark_mode", "Dark Mode", true, true, None::<&str>)?;

            let view_menu = Submenu::with_id_and_items(
                "view",
                "View",
                true,
                &[
                    &dark_mode,
                    &PredefinedMenuItem::separator()?,
                    &PredefinedMenuItem::fullscreen(None)?,
                ],
            )?;

            let menu = Menu::with_id_and_items("main", &[&file_menu, &edit_menu, &view_menu])?;

            #[cfg(target_os = "windows")]
            #[cfg(target_os = "windows")]
            {
                let theme = theme.map(map_to_menu_theme).unwrap_or(muda::MenuTheme::Auto);
                let _ = unsafe { menu.inner().init_for_hwnd_with_theme(raw_window.hwnd as _, theme) };
            }

            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                let _ = menu
                    .0
                    .inner
                    .init_for_gtk_window(raw_window.gtk_window, raw_window.default_vbox);
            }

            Ok(WindowMenu {
                is_app_wide: false,
                menu,
            })
        }),
    )?;

    // -------------------------------------------------------------------------
    // Shared ManagedWindow
    // -------------------------------------------------------------------------

    let window = Arc::new(window);

    // -------------------------------------------------------------------------
    // Root WebView
    // -------------------------------------------------------------------------

    let webview = window.webview("root")?;

    webview.set_zoom(1.0)?;

    println!("current url: {}", webview.url()?);

    for cookie in webview.cookies()? {
        println!("cookie: {} = {}", cookie.name(), cookie.value());
    }

    // -------------------------------------------------------------------------
    // KEIN build_menu() / set_window_menu() mehr hier.
    //
    // Das WindowMenu wurde bereits durch:
    //
    // WindowBuilder::with_menu(...)
    //          ↓
    // create_window(...)
    //          ↓
    // setup_menu(&window)
    //          ↓
    // ManagedWindow.menu
    //
    // erzeugt.
    // -------------------------------------------------------------------------

    // -------------------------------------------------------------------------
    // Event loop
    // -------------------------------------------------------------------------

    let window_for_loop = window.clone();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            // -----------------------------------------------------------------
            // Window events
            // -----------------------------------------------------------------
            Event::WindowEvent { event, window_id, .. } if window_id == window_for_loop.id() => {
                window_for_loop.emit_window_event(&event);

                match event {
                    WindowEvent::CloseRequested => match window_for_loop.close() {
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

                    // ---------------------------------------------------------
                    // Rechtsklick -> gespeichertes WindowMenu als Popup öffnen
                    // ---------------------------------------------------------
                    WindowEvent::MouseInput {
                        state: ElementState::Pressed,
                        button: MouseButton::Right,
                        ..
                    } => {
                        if let Err(error) = window_for_loop.popup_window_menu() {
                            eprintln!("popup error: {error}");
                        }
                    }

                    _ => {}
                }
            }

            // -----------------------------------------------------------------
            // Menu events
            // -----------------------------------------------------------------
            Event::UserEvent(UserEvent::MenuClicked(item_id)) => {
                println!("menu clicked: {item_id}");

                match item_id.as_str() {
                    "file.open" => {
                        println!("→ OPEN");
                    }

                    "file.save" => {
                        println!("→ SAVE");
                    }

                    "view.dark_mode" => {
                        if let Some(menu) = window_for_loop.menu() {
                            let id = "view.dark_mode".into();

                            match menu.find(&id) {
                                Ok(Some(MenuItemKind::Check(check))) => match check.is_checked() {
                                    Ok(current) => {
                                        let next = !current;

                                        if let Err(error) = check.set_checked(next) {
                                            eprintln!(
                                                "set_checked error: \
                                                     {error}"
                                            );
                                        } else {
                                            println!("→ dark mode = {next}");
                                        }
                                    }

                                    Err(error) => {
                                        eprintln!("is_checked error: {error}");
                                    }
                                },

                                Ok(Some(_)) => {
                                    eprintln!(
                                        "view.dark_mode is not a \
                                         CheckMenuItem"
                                    );
                                }

                                Ok(None) => {
                                    eprintln!("view.dark_mode not found");
                                }

                                Err(error) => {
                                    eprintln!("menu.find error: {error}");
                                }
                            }
                        }
                    }

                    other => {
                        println!("→ unhandled item: {other}");
                    }
                }
            }

            // -----------------------------------------------------------------
            // Shutdown
            // -----------------------------------------------------------------
            Event::LoopDestroyed => {
                println!("event loop destroyed");
            }

            _ => {}
        }
    });
}
