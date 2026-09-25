#[cfg(windows)]
mod windows;

// Takes a `&'static str` here since we convert clickable hyperlinks,
// DO NOT pass in untrusted input
pub fn error(err: &'static str) {
    #[cfg(windows)]
    windows::error(err);

    #[cfg(not(windows))]
    {
        unimplemented!("Error dialog is not implemented for this platform");
    }
}
