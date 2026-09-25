use crate::types::PhysicalRect;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub trait MonitorExt {
    /// Get the work area of this monitor
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn work_area(&self) -> PhysicalRect<i32, u32>;
}
