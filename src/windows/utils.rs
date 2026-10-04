#[cfg(windows)]
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use crate::schema::FocusState;
#[cfg(windows)]
use webview2_com::{FocusChangedEventHandler, Microsoft::Web::WebView2::Win32::ICoreWebView2Controller};

pub fn apply_shadow_correction(
    decorations: bool,
    window_size: &mut crate::schema::PhysicalSize<u32>, // oder was auch immer der Typ ist
) -> anyhow::Result<u32> {
    #[allow(unused_mut)]
    let mut shadow_width = 0;

    #[cfg(windows)]
    if decorations {
        use windows::Win32::UI::WindowsAndMessaging::{AdjustWindowRect, WS_OVERLAPPEDWINDOW};
        let mut rect = windows::Win32::Foundation::RECT::default();
        let result = unsafe { AdjustWindowRect(&mut rect, WS_OVERLAPPEDWINDOW, false) };
        if result.is_ok() {
            shadow_width = (rect.right - rect.left) as u32;
            // rect.bottom is made out of shadow, and we don't care about it
            window_size.height += -rect.top as u32;
        }
    }

    Ok(shadow_width)
}

/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
/// Used to prevent duplicated [`WindowEvent::Focused`] events,
/// and to track last focused webview in multi-webview mode for us to restore webview focuses
#[cfg(windows)]
pub fn add_focus_change_listeners(
    window_id: Arc<Mutex<u32>>,
    id: u32,
    focused_webview: Arc<Mutex<FocusState>>,
    label: String,
    controller: &ICoreWebView2Controller,
    token: &mut i64,
    on_focus_change: impl Fn(u32, u32, bool) + Send + Sync + 'static,
) {
    let label_ = label.clone();
    let window_id_ = window_id.clone();
    let focused_webview_ = focused_webview.clone();
    let on_focus_change = Arc::new(on_focus_change);
    let on_focus_change_got = on_focus_change.clone();
    if let Err(error) = unsafe {
        controller.add_GotFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let mut focused_webview = focused_webview_.lock().unwrap();
                // When using multi-webview mode, we should check if the focus change is actually a "webview focus change"
                // instead of a window focus change (here we're patching window events, so we only care about the actual window changing focus)
                let already_focused = matches!(
                    *focused_webview,
                    FocusState::WindowFocused | FocusState::WebviewFocused { .. }
                );
                *focused_webview = FocusState::WebviewFocused {
                    webview_label: label_.clone(),
                };
                if !already_focused {
                    on_focus_change_got(*window_id_.lock().unwrap(), id, true);
                }
                Ok(())
            })),
            token,
        )
    } {
        log::error!(
            "Failed to attach WebView2 `add_GotFocus` handler, `WindowEvent::Focused` will not be sent: {error}"
        );
        return;
    }
    if let Err(error) = unsafe {
        controller.add_LostFocus(
            &FocusChangedEventHandler::create(Box::new(move |_, _| {
                let mut focused_webview = focused_webview.lock().unwrap();
                // When using multi-webview mode, we should handle webview focus changes
                // so we check whether the currently focused webview matches this webview's
                // (in this case, it means we lost the window focus)
                //
                // In multi-webview mode, if we change focus to a different webview
                // we get the gotFocus event of the other webview before the lostFocus
                // so this check makes sense
                if let FocusState::WebviewFocused { ref webview_label } = *focused_webview {
                    let lost_window_focus = webview_label == &label;
                    if lost_window_focus {
                        // Only reset when we lost window focus - otherwise some other webview is focused
                        *focused_webview = FocusState::Blured {
                            last_focused_webview_label: Some(label.clone()),
                        };
                        on_focus_change(*window_id.lock().unwrap(), id, false);
                    }
                }
                Ok(())
            })),
            token,
        )
    } {
        log::error!(
            "Failed to attach WebView2 `add_LostFocus` handler, `WindowEvent::Focused` will not be sent: {error}"
        );
    }
}
