pub(crate) mod dialog;
pub(crate) mod monitor;

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
))]
pub(crate) mod undecorated_resizing;

pub(crate) mod util;
pub(crate) mod webview;
pub(crate) mod window;
