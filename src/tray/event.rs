use dpi::PhysicalPosition;
use serde::Serialize;

pub use tray_icon::TrayIconId;

use crate::types::Rect;

/// Describes the mouse button state.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum MouseButtonState {
    /// Mouse button pressed.
    #[default]
    Up,
    /// Mouse button released.
    Down,
}

impl From<tray_icon::MouseButtonState> for MouseButtonState {
    fn from(value: tray_icon::MouseButtonState) -> Self {
        match value {
            tray_icon::MouseButtonState::Up => Self::Up,
            tray_icon::MouseButtonState::Down => Self::Down,
        }
    }
}

/// Describes which mouse button triggered the event.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Default)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl From<tray_icon::MouseButton> for MouseButton {
    fn from(value: tray_icon::MouseButton) -> Self {
        match value {
            tray_icon::MouseButton::Left => Self::Left,
            tray_icon::MouseButton::Right => Self::Right,
            tray_icon::MouseButton::Middle => Self::Middle,
        }
    }
}

/// Describes a tray icon event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[non_exhaustive]
pub enum TrayIconEvent {
    #[serde(rename_all = "camelCase")]
    Click {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
        button: MouseButton,
        button_state: MouseButtonState,
    },
    DoubleClick {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
        button: MouseButton,
    },
    Enter {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },
    Move {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },
    Leave {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },
}

impl TrayIconEvent {
    pub fn id(&self) -> &TrayIconId {
        match self {
            Self::Click { id, .. }
            | Self::DoubleClick { id, .. }
            | Self::Enter { id, .. }
            | Self::Move { id, .. }
            | Self::Leave { id, .. } => id,
        }
    }
}

impl From<tray_icon::TrayIconEvent> for TrayIconEvent {
    fn from(value: tray_icon::TrayIconEvent) -> Self {
        match value {
            tray_icon::TrayIconEvent::Click {
                id,
                position,
                rect,
                button,
                button_state,
            } => Self::Click {
                id,
                position,
                rect: Rect {
                    position: rect.position.into(),
                    size: rect.size.into(),
                },
                button: button.into(),
                button_state: button_state.into(),
            },
            tray_icon::TrayIconEvent::DoubleClick {
                id,
                position,
                rect,
                button,
            } => Self::DoubleClick {
                id,
                position,
                rect: Rect {
                    position: rect.position.into(),
                    size: rect.size.into(),
                },
                button: button.into(),
            },
            tray_icon::TrayIconEvent::Enter { id, position, rect } => {
                Self::Enter {
                    id,
                    position,
                    rect: Rect {
                        position: rect.position.into(),
                        size: rect.size.into(),
                    },
                }
            }
            tray_icon::TrayIconEvent::Move { id, position, rect } => {
                Self::Move {
                    id,
                    position,
                    rect: Rect {
                        position: rect.position.into(),
                        size: rect.size.into(),
                    },
                }
            }
            tray_icon::TrayIconEvent::Leave { id, position, rect } => {
                Self::Leave {
                    id,
                    position,
                    rect: Rect {
                        position: rect.position.into(),
                        size: rect.size.into(),
                    },
                }
            }
            _ => unreachable!("unsupported tray-icon event variant"),
        }
    }
}
