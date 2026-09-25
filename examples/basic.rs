use std::sync::Arc;

use taurino_core::native::tao::window::Window;
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
    // Event loop with custom user event
    // -------------------------------------------------------------------------
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // -------------------------------------------------------------------------
    // Install the muda event handler BEFORE showing any menu.
    //
    // Runs on muda's own thread → only forward `Send` data into the proxy.
    // -------------------------------------------------------------------------
    install_menu_event_handler({
        let proxy = proxy.clone();
        move |item_id: String| {
            let _ = proxy.send_event(UserEvent::MenuClicked(item_id));
        }
    });

    // -------------------------------------------------------------------------
    // Window + WebView
    //
    // The 5th argument is a `SetupMenu`:
    //   Box<dyn Fn(&Window) -> Result<WindowMenu> + Send + 'static>
    // It is invoked once per window to build that window's menu.
    // -------------------------------------------------------------------------
    let web_context: WebContextStore = Default::default();

    let window_builder = WindowBuilder::new()
        .theme(Some(Theme::Dark))
        .add_webview_builder(
            WebViewBuilder::new().with_url(WebviewUrl::External(Url::parse("https://tauri.app").unwrap())),
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

    let window = window_builder.build(
        &event_loop,
        1.into(),
        web_context.clone(),
        None::<fn(wry::WebViewBuilder<'_>, WebviewUrl) -> crate::Result<wry::WebViewBuilder<'_>>>,
        Some(Box::new(|_window: &Window| -> Result<WindowMenu> {
            // --- File -------------------------------------------------------
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

            // --- Edit (all predefined) --------------------------------------
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

            // --- View -------------------------------------------------------
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

            // --- Root -------------------------------------------------------
            let menu = Menu::with_id_and_items("main", &[&file_menu, &edit_menu, &view_menu])?;

            // Window-scoped (not app-wide) menu.
            Ok(WindowMenu {
                is_app_wide: false,
                menu,
            })
        })),
    )?;

    let window = Arc::new(window);
    let webview = window.webview("root")?;

    webview.set_zoom(1.0)?;
    println!("current url: {}", webview.url()?);

    for cookie in webview.cookies()? {
        println!("cookie: {} = {}", cookie.name(), cookie.value());
    }

    // -------------------------------------------------------------------------
    // Install the menu built above onto the window.
    //
    // `set_window_menu` runs the platform-specific `init_for_hwnd` /
    // `init_for_gtk_window` internally, so we don't need to do it ourselves.
    // On macOS the menu is app-wide; on Win/Linux it becomes the window menubar.
    // -------------------------------------------------------------------------
    if let Ok(menu) = build_menu() {
        let _previous = window.set_window_menu(menu)?;
    }

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
                        Ok(true) => *control_flow = ControlFlow::Exit,
                        Ok(false) => {}
                        Err(error) => eprintln!("close error: {error}"),
                    },

                    WindowEvent::Resized(size) => {
                        println!("resized: {size:?}");
                    }

                    // Right-click → context menu.
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
            // Menu clicks forwarded from muda
            // -----------------------------------------------------------------
            Event::UserEvent(UserEvent::MenuClicked(item_id)) => {
                println!("menu clicked: {item_id}");

                match item_id.as_str() {
                    "file.open" => println!("→ OPEN"),
                    "file.save" => println!("→ SAVE"),

                    "view.dark_mode" => {
                        if let Some(menu) = window_for_loop.menu() {
                            let id = "view.dark_mode".into();

                            match menu.find(&id) {
                                Ok(Some(MenuItemKind::Check(check))) => match check.is_checked() {
                                    Ok(current) => {
                                        let next = !current;

                                        if let Err(error) = check.set_checked(next) {
                                            eprintln!("set_checked error: {error}");
                                        } else {
                                            println!("→ dark mode = {next}");
                                        }
                                    }

                                    Err(error) => {
                                        eprintln!("is_checked error: {error}");
                                    }
                                },

                                Ok(Some(_)) => {
                                    eprintln!("view.dark_mode is not a CheckMenuItem");
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

                    other => println!("→ unhandled item: {other}"),
                }
            }

            Event::LoopDestroyed => {
                println!("event loop destroyed");
            }

            _ => {}
        }
    });
}

// -----------------------------------------------------------------------------
// Helper: build the same menu we pass to the window at startup, so we can call
// `set_window_menu` after the window is constructed.
//
// In a real app you'd just capture the `WindowMenu` returned from the factory.
// -----------------------------------------------------------------------------
fn build_menu() -> Result<WindowMenu> {
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

    Ok(WindowMenu {
        is_app_wide: false,
        menu,
    })
}
