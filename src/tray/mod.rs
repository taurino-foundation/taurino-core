// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Standalone tray icon types and utilities.
//!
//! This module intentionally has no `AppHandle`, `Runtime` or `Manager` dependency.
//! Like the standalone menu wrapper, native tray operations must be called from the
//! GUI/main thread required by `tray-icon` on the target platform.

mod builder;
mod event;
mod icon;

pub use builder::TrayIconBuilder;
pub use event::{MouseButton, MouseButtonState, TrayIconEvent};
pub use icon::TrayIcon;
pub use tray_icon::TrayIconId;
