use std::sync::atomic::Ordering;

use crate::platform::monitor::MonitorExt;
use crate::window::ManagedWindow;
use crate::{
    error::Error,
    types::{
        CursorIcon, DeviceEventFilter, Monitor, ProgressBarState, ProgressBarStatus, Rect,
        UserAttentionType,
    },
    utils::{inner_size, map_theme, Icon},
};
use dpi::{PhysicalPosition, PhysicalSize};
use serde::{Deserialize, Serialize};
use tao::{
    event::WindowEvent as TaoWindowEvent,
    event_loop::DeviceEventFilter as TaoDeviceEventFilter,
    monitor::MonitorHandle,
    window::{
        CursorIcon as TaoCursorIcon, Icon as TaoWindowIcon,
        ProgressBarState as TaoProgressBarState, ProgressState as TaoProgressState,
        UserAttentionType as TaoUserAttentionType,
    },
};
/// Wrapper around a [`tao::window::Icon`] that can be created from an [`Icon`].
pub struct TaoIcon(pub TaoWindowIcon);

impl TryFrom<Icon<'_>> for TaoIcon {
    type Error = Error;
    fn try_from(icon: Icon<'_>) -> std::result::Result<Self, Self::Error> {
        TaoWindowIcon::from_rgba(icon.rgba.to_vec(), icon.width, icon.height)
            .map(Self)
            .map_err(|e| Error::InvalidIcon(Box::new(e)))
    }
}

#[allow(dead_code)]
pub struct DeviceEventFilterWrapper(pub TaoDeviceEventFilter);

impl From<DeviceEventFilter> for DeviceEventFilterWrapper {
    fn from(item: DeviceEventFilter) -> Self {
        match item {
            DeviceEventFilter::Always => Self(TaoDeviceEventFilter::Always),
            DeviceEventFilter::Never => Self(TaoDeviceEventFilter::Never),
            DeviceEventFilter::Unfocused => Self(TaoDeviceEventFilter::Unfocused),
        }
    }
}

pub struct RectWrapper(pub wry::Rect);
impl From<Rect> for RectWrapper {
    fn from(value: Rect) -> Self {
        RectWrapper(wry::Rect {
            position: value.position,
            size: value.size,
        })
    }
}

pub struct MonitorHandleWrapper(pub MonitorHandle);

impl From<MonitorHandleWrapper> for Monitor {
    fn from(monitor: MonitorHandleWrapper) -> Monitor {
        Self {
            name: monitor.0.name(),
            position: monitor.0.position(),
            size: monitor.0.size(),
            work_area: monitor.0.work_area(),
            scale_factor: monitor.0.scale_factor(),
        }
    }
}
#[derive(Debug, Clone)]
pub struct UserAttentionTypeWrapper(pub TaoUserAttentionType);

impl From<UserAttentionType> for UserAttentionTypeWrapper {
    fn from(request_type: UserAttentionType) -> Self {
        let o = match request_type {
            UserAttentionType::Critical => TaoUserAttentionType::Critical,
            UserAttentionType::Informational => TaoUserAttentionType::Informational,
        };
        Self(o)
    }
}

#[derive(Debug)]
pub struct CursorIconWrapper(pub TaoCursorIcon);

impl From<CursorIcon> for CursorIconWrapper {
    fn from(icon: CursorIcon) -> Self {
        use CursorIcon::*;
        let i = match icon {
            Default => TaoCursorIcon::Default,
            Crosshair => TaoCursorIcon::Crosshair,
            Hand => TaoCursorIcon::Hand,
            Arrow => TaoCursorIcon::Arrow,
            Move => TaoCursorIcon::Move,
            Text => TaoCursorIcon::Text,
            Wait => TaoCursorIcon::Wait,
            Help => TaoCursorIcon::Help,
            Progress => TaoCursorIcon::Progress,
            NotAllowed => TaoCursorIcon::NotAllowed,
            ContextMenu => TaoCursorIcon::ContextMenu,
            Cell => TaoCursorIcon::Cell,
            VerticalText => TaoCursorIcon::VerticalText,
            Alias => TaoCursorIcon::Alias,
            Copy => TaoCursorIcon::Copy,
            NoDrop => TaoCursorIcon::NoDrop,
            Grab => TaoCursorIcon::Grab,
            Grabbing => TaoCursorIcon::Grabbing,
            AllScroll => TaoCursorIcon::AllScroll,
            ZoomIn => TaoCursorIcon::ZoomIn,
            ZoomOut => TaoCursorIcon::ZoomOut,
            EResize => TaoCursorIcon::EResize,
            NResize => TaoCursorIcon::NResize,
            NeResize => TaoCursorIcon::NeResize,
            NwResize => TaoCursorIcon::NwResize,
            SResize => TaoCursorIcon::SResize,
            SeResize => TaoCursorIcon::SeResize,
            SwResize => TaoCursorIcon::SwResize,
            WResize => TaoCursorIcon::WResize,
            EwResize => TaoCursorIcon::EwResize,
            NsResize => TaoCursorIcon::NsResize,
            NeswResize => TaoCursorIcon::NeswResize,
            NwseResize => TaoCursorIcon::NwseResize,
            ColResize => TaoCursorIcon::ColResize,
            RowResize => TaoCursorIcon::RowResize,
            #[allow(unreachable_patterns)]
            _ => TaoCursorIcon::Default,
        };
        Self(i)
    }
}

pub struct ProgressStateWrapper(pub TaoProgressState);

impl From<ProgressBarStatus> for ProgressStateWrapper {
    fn from(status: ProgressBarStatus) -> Self {
        let state = match status {
            ProgressBarStatus::None => TaoProgressState::None,
            ProgressBarStatus::Normal => TaoProgressState::Normal,
            ProgressBarStatus::Indeterminate => TaoProgressState::Indeterminate,
            ProgressBarStatus::Paused => TaoProgressState::Paused,
            ProgressBarStatus::Error => TaoProgressState::Error,
        };
        Self(state)
    }
}

pub struct ProgressBarStateWrapper(pub TaoProgressBarState);

impl From<ProgressBarState> for ProgressBarStateWrapper {
    fn from(progress_state: ProgressBarState) -> Self {
        Self(TaoProgressBarState {
            progress: progress_state.progress,
            state: progress_state
                .status
                .map(|state| ProgressStateWrapper::from(state).0),
            desktop_filename: progress_state.desktop_filename,
        })
    }
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WindowEvent {
    Resized(PhysicalSize<u32>),
    Moved(PhysicalPosition<i32>),

    Destroyed,

    ScaleFactorChanged {
        scale_factor: f64,
        new_inner_size: PhysicalSize<u32>,
    },

    Focused(bool),

    ThemeChanged(crate::types::Theme),
}
pub struct WindowEventWrapper(pub Option<WindowEvent>);

impl WindowEventWrapper {
    pub fn map_from_tao(
        event: &TaoWindowEvent<'_>,
        #[cfg(windows)] window: &ManagedWindow,
    ) -> Self {
        let event = match event {
            TaoWindowEvent::Resized(size) => WindowEvent::Resized(*size),
            TaoWindowEvent::Moved(position) => WindowEvent::Moved(*position),
            TaoWindowEvent::Destroyed => WindowEvent::Destroyed,
            TaoWindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => WindowEvent::ScaleFactorChanged {
                scale_factor: *scale_factor,
                new_inner_size: **new_inner_size,
            },
            TaoWindowEvent::Focused(focused) => {
                #[cfg(not(windows))]
                return Self(Some(WindowEvent::Focused(*focused)));
                // on multiwebview mode, if there's no focused webview, it means we're receiving a direct window focus change
                // (without receiving a webview focus, such as when clicking the taskbar app icon or using Alt + Tab)
                // in this case we must send the focus change event here
                #[cfg(windows)]
                if window.has_children.load(Ordering::Relaxed) {
                    use crate::types::FocusState;

                    if !*focused {
                        // Blur events are handled in the webview side (add_LostFocus)
                        return Self(None);
                    }

                    let mut focused_webview = window.focused_webview.lock().unwrap();
                    if let FocusState::Blured {
                        last_focused_webview_label,
                    } = &*focused_webview
                    {
                        let should_focus_webview = last_focused_webview_label.as_deref().and_then(
                            |last_focused_webview_label| {
                                window.webviews.iter().find(|w| {
                                    w.metadata.webview_label == last_focused_webview_label
                                })
                            },
                        );
                        *focused_webview = FocusState::WindowFocused;
                        if let Some(should_focus_webview) = should_focus_webview {
                            drop(focused_webview);
                            let _ = should_focus_webview.focus();
                        }
                        WindowEvent::Focused(true)
                    } else {
                        // Already focused
                        return Self(None);
                    }
                } else if window.webviews.is_empty() {
                    // Raw tao window without webviews, forward the event
                    WindowEvent::Focused(*focused)
                } else {
                    // when not on multiwebview mode, wry will set focus to the webview,
                    // and we will handle focus change events on the webview (add_GotFocus and add_LostFocus)
                    return Self(None);
                }
            }
            TaoWindowEvent::ThemeChanged(theme) => WindowEvent::ThemeChanged(map_theme(theme)),
            _ => return Self(None),
        };
        Self(Some(event))
    }

    pub fn parse(window: &ManagedWindow, event: &TaoWindowEvent<'_>) -> Self {
        match event {
            // resized event from tao doesn't include a reliable size on macOS
            // because wry replaces the NSView
            TaoWindowEvent::Resized(_) => {
                if let Some(w) = &window.inner {
                    let size = inner_size(
                        w,
                        &window.webviews,
                        window.has_children.load(Ordering::Relaxed),
                    );
                    Self(Some(WindowEvent::Resized(size)))
                } else {
                    Self(None)
                }
            }
            e => Self::map_from_tao(
                e,
                #[cfg(windows)]
                window,
            ),
        }
    }
}
