// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::borrow::Cow;

#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{E_FAIL, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, WIN32_ERROR},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, GetDIBits, HBITMAP,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            GetIconInfo, GetSystemMetrics, HICON, ICONINFO, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON,
            SM_CYICON,
        },
    },
    core::{Owned, PCWSTR},
};

use crate::utils::{Icon, resource::Resource};

/// Resource id of the application icon that `build` embeds into Windows executables
/// and that `tauri::image::Image::from_app_icon_resource` reads back.
///
/// `32512` has no special meaning here: it was picked because we misunderstood
/// `IDI_APPLICATION` (`MAKEINTRESOURCE(32512)`) to be the id an application icon must use,
/// which is not the case. See <https://devblogs.microsoft.com/oldnewthing/20250423-00/?p=111106>.
pub const WINDOWS_APP_ICON_RESOURCE_ID: u16 = 32512;

/// Platform target.
/// Identifies an icon resource embedded in the executable.
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconResource<'a> {
    /// An integer resource identifier (`MAKEINTRESOURCE`).
    Id(u16),
    /// A string resource name.
    Name(&'a str),
}

#[cfg(windows)]
impl From<u16> for IconResource<'_> {
    fn from(id: u16) -> Self {
        Self::Id(id)
    }
}

#[cfg(windows)]
impl<'a> From<&'a str> for IconResource<'a> {
    fn from(name: &'a str) -> Self {
        Self::Name(name)
    }
}

/// Loads the default window icon from the application icon resource, reporting failures.
///
/// Used by [`crate::generate_context!`]; not public API.
#[cfg(windows)]
#[doc(hidden)]
#[allow(dead_code)]
pub fn default_window_icon_from_app_icon_resource() -> Option<Image<'static>> {
    // the window icon is drawn in the title bar and, as a fallback for the taskbar icon,
    // at the system's large icon size (32x32 at 96 DPI, scaled with the system DPI),
    // so pick the entry Windows would use for the taskbar instead of a larger one it has to shrink
    // `GetSystemMetrics` returns 0 on failure
    let metric = |index| match unsafe { GetSystemMetrics(index) } {
        n if n > 0 => n as u32,
        _ => 32,
    };
    let (width, height) = (metric(SM_CXICON), metric(SM_CYICON));
    match Image::from_icon_resource(WINDOWS_APP_ICON_RESOURCE_ID, width, height) {
        Ok(icon) => Some(icon),
        Err(e) => {
            // a logger is usually not installed yet when `generate_context!` runs
            #[cfg(debug_assertions)]
            eprintln!("failed to load the default window icon from the application icon resource: {e}");
            log::warn!("failed to load the default window icon from the application icon resource: {e}");
            None
        }
    }
}

#[cfg(windows)]
const BYTES_PER_PIXEL: usize = 4;

/// Reads `hbm` as a top-down 32bpp BGRA bitmap of the given dimensions.
///
/// # Safety
///
/// `hbm` must be a valid bitmap handle and `width` and `height` must be positive.
#[cfg(windows)]
unsafe fn read_bgra(hbm: HBITMAP, width: i32, height: i32) -> crate::error::Result<Vec<u8>> {
    let image_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
        .ok_or_else(|| resource_error(ERROR_INVALID_PARAMETER, "image size overflows usize"))?;
    let mut bgra = vec![0u8; image_bytes];

    let mut bitmap_info = BITMAPINFO::default();
    bitmap_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as _;
    bitmap_info.bmiHeader.biWidth = width;
    // negative value for top-down
    bitmap_info.bmiHeader.biHeight = -height;
    bitmap_info.bmiHeader.biBitCount = (BYTES_PER_PIXEL * 8) as u16;
    bitmap_info.bmiHeader.biPlanes = 1;
    bitmap_info.bmiHeader.biCompression = BI_RGB.0;

    unsafe {
        let hdc = CreateCompatibleDC(None);
        let scan_lines = GetDIBits(
            hdc,
            hbm,
            0,
            height as u32,
            Some(bgra.as_mut_ptr() as _),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        );
        // capture the error before `DeleteDC` can overwrite it
        let error = (scan_lines != height)
            .then(|| last_error_or(&format!("GetDIBits copied {scan_lines} of {height} scan lines")));
        let _ = DeleteDC(hdc);
        if let Some(error) = error {
            return Err(crate::error::Error::ImageFromResource(error));
        }
    }

    Ok(bgra)
}

#[cfg(windows)]
fn resource_error(code: WIN32_ERROR, message: &str) -> crate::error::Error {
    crate::error::Error::ImageFromResource(windows::core::Error::new(code.to_hresult(), message))
}

/// Returns the calling thread's last error, or a generic `E_FAIL` with `message`
/// when no error code was set (GDI functions do not always set one).
#[cfg(windows)]
fn last_error_or(message: &str) -> windows::core::Error {
    let error = windows::core::Error::from_thread();
    if error.code().is_ok() {
        windows::core::Error::new(E_FAIL, message)
    } else {
        error
    }
}

/// An RGBA Image in row-major order from top to bottom.
#[derive(Clone)]
pub struct Image<'a> {
    rgba: Cow<'a, [u8]>,
    width: u32,
    height: u32,
}

impl std::fmt::Debug for Image<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field(
                "rgba",
                // Reduces the debug size compared to the derived default, as the default
                // would format the raw bytes as numbers `[0, 0, 0, 0]` for 1 pixel.
                // The custom format doesn't grow as much with larger images:
                // `Image { rgba: Cow::Borrowed([u8; 4096]), width: 32, height: 32 }`
                &format_args!(
                    "Cow::{}([u8; {}])",
                    match &self.rgba {
                        Cow::Borrowed(_) => "Borrowed",
                        Cow::Owned(_) => "Owned",
                    },
                    self.rgba.len()
                ),
            )
            .field("width", &self.width)
            .field("height", &self.height)
            .finish()
    }
}

impl Resource for Image<'static> {}

impl Image<'static> {
    /// Creates a new Image using RGBA data, in row-major order from top to bottom, and with specified width and height.
    ///
    /// Similar to [`Self::new`] but avoids cloning the rgba data to get an owned Image.
    pub const fn new_owned(rgba: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            rgba: Cow::Owned(rgba),
            width,
            height,
        }
    }
}

impl<'a> Image<'a> {
    /// Creates a new Image using RGBA data, in row-major order from top to bottom, and with specified width and height.
    pub const fn new(rgba: &'a [u8], width: u32, height: u32) -> Self {
        Self {
            rgba: Cow::Borrowed(rgba),
            width,
            height,
        }
    }

    /// Creates a new image using the provided bytes.
    ///
    /// Only `ico` and `png` are supported (based on activated feature flag).

    pub fn from_bytes(bytes: &[u8]) -> crate::error::Result<Self> {
        let img = image::load_from_memory(bytes)?;
        let (width, height) = (img.width(), img.height());
        Ok(Self {
            rgba: Cow::Owned(img.into_rgba8().into_raw()),
            width,
            height,
        })
    }

    /// Creates a new image using the provided path.
    ///
    /// Only `ico` and `png` are supported (based on activated feature flag).

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> crate::error::Result<Self> {
        let bytes = std::fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    /// Creates a new image from the application icon embedded in the executable of the current process.
    ///
    /// The application icon is the one `tauri-build` embeds with the
    /// [`WINDOWS_APP_ICON_RESOURCE_ID`](WINDOWS_APP_ICON_RESOURCE_ID) id,
    /// this could change in the future.
    #[cfg(windows)]
    #[cfg_attr(docsrs, doc(cfg(windows)))]
    pub fn from_app_icon_resource(size: u32) -> crate::error::Result<Self> {
        Image::from_icon_resource(WINDOWS_APP_ICON_RESOURCE_ID, size, size)
    }

    /// Create a new image from an icon resource embedded in the executable of the current process.
    ///
    /// Resources are looked up in the process executable (`GetModuleHandleW(NULL)`),
    /// not in the DLL containing this code when tauri is built as a library.
    ///
    /// **Note**: This might take ~2ms for [`LoadImageW`] to load the image for the first time.
    ///
    /// ## Examples
    ///
    /// The resource can be identified by its integer id or by its name, see [`IconResource`].
    ///
    /// ```ignore
    /// # use tauri::image::Image;
    /// # fn main() -> tauri::Result<()> {
    /// let icon = Image::from_icon_resource(1, 32, 32)?;
    /// let icon = Image::from_icon_resource("icon", 32, 32)?;
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(windows)]
    #[cfg_attr(docsrs, doc(cfg(windows)))]
    pub fn from_icon_resource<'r>(
        resource: impl Into<IconResource<'r>>,
        width: u32,
        height: u32,
    ) -> crate::error::Result<Self> {
        let (width_i32, height_i32) = match (i32::try_from(width), i32::try_from(height)) {
            (Ok(w), Ok(h)) if w > 0 && h > 0 => (w, h),
            _ => {
                return Err(resource_error(
                    ERROR_INVALID_PARAMETER,
                    "width and height must be between 1 and i32::MAX",
                ));
            }
        };

        // keeps the wide string alive for the `LoadImageW` call
        let name: Vec<u16>;
        let resource_id = match resource.into() {
            // MAKEINTRESOURCE
            IconResource::Id(id) => PCWSTR(id as usize as *const u16),
            IconResource::Name(n) => {
                name = n.encode_utf16().chain(std::iter::once(0)).collect();
                PCWSTR(name.as_ptr())
            }
        };

        let hicon = unsafe {
            Owned::new(HICON(
                LoadImageW(
                    Some(
                        GetModuleHandleW(PCWSTR::null())
                            .map_err(crate::error::Error::ImageFromResource)?
                            .into(),
                    ),
                    resource_id,
                    IMAGE_ICON,
                    width_i32,
                    height_i32,
                    LR_DEFAULTCOLOR,
                )
                .map_err(crate::error::Error::ImageFromResource)?
                .0,
            ))
        };

        let mut icon_info = ICONINFO::default();
        unsafe { GetIconInfo(*hicon, &mut icon_info).map_err(crate::error::Error::ImageFromResource)? };
        let hbm_mask = unsafe { Owned::new(icon_info.hbmMask) };
        let hbm_color = unsafe { Owned::new(icon_info.hbmColor) };

        // monochrome icons only have a mask bitmap (AND mask stacked on top of the XOR mask)
        if hbm_color.is_invalid() {
            return Err(resource_error(
                ERROR_NOT_SUPPORTED,
                "monochrome icons are not supported",
            ));
        }

        let mut bgra = unsafe { read_bgra(*hbm_color, width_i32, height_i32)? };

        // Color bitmaps without an alpha channel (e.g. 24bpp icons) read back with alpha = 0 on every pixel,
        // so recover the alpha channel from the AND mask: a set bit means the pixel is transparent.
        if bgra.as_chunks::<BYTES_PER_PIXEL>().0.iter().all(|px| px[3] == 0) {
            let mask = unsafe { read_bgra(*hbm_mask, width_i32, height_i32)? };
            for (px, mask) in bgra
                .as_chunks_mut::<BYTES_PER_PIXEL>()
                .0
                .iter_mut()
                .zip(mask.as_chunks::<BYTES_PER_PIXEL>().0)
            {
                // the 1bpp mask expands to black (clear bit) or white (set bit)
                px[3] = if mask[0] == 0 { 0xFF } else { 0 };
            }
        }

        let rgba = {
            for px in bgra.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
                // Swap Blue and Red channels
                px.swap(0, 2);
            }
            bgra
        };

        Ok(Image::new_owned(rgba, width, height))
    }

    /// Returns the RGBA data for this image, in row-major order from top to bottom.
    pub fn rgba(&'a self) -> &'a [u8] {
        &self.rgba
    }

    /// Returns the width of this image.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height of this image.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Convert into a 'static owned [`Image`].
    /// This will allocate.
    pub fn to_owned(self) -> Image<'static> {
        Image {
            rgba: match self.rgba {
                Cow::Owned(v) => Cow::Owned(v),
                Cow::Borrowed(v) => Cow::Owned(v.to_vec()),
            },
            height: self.height,
            width: self.width,
        }
    }
}

impl<'a> From<Image<'a>> for Icon<'a> {
    fn from(img: Image<'a>) -> Self {
        Self {
            rgba: img.rgba,
            width: img.width,
            height: img.height,
        }
    }
}

impl TryFrom<Image<'_>> for muda::Icon {
    type Error = crate::error::Error;

    fn try_from(img: Image<'_>) -> Result<Self, Self::Error> {
        muda::Icon::from_rgba(img.rgba.into_owned(), img.width, img.height).map_err(Into::into)
    }
}

impl TryFrom<Image<'_>> for tray_icon::Icon {
    type Error = crate::error::Error;

    fn try_from(img: Image<'_>) -> Result<Self, Self::Error> {
        tray_icon::Icon::from_rgba(img.rgba.into_owned(), img.width, img.height).map_err(Into::into)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{IconResource, Image, default_window_icon_from_app_icon_resource};

    /// The test executable has no icon resources, so every lookup must fail with an error
    /// (instead of panicking or returning a stretched placeholder).
    #[test]
    fn from_icon_resource_missing_resource_is_an_error() {
        for resource in [
            IconResource::Id(u16::MAX),
            IconResource::Name("tauri-image-test-missing-icon"),
        ] {
            let error = Image::from_icon_resource(resource, 32, 32).unwrap_err();
            assert!(
                matches!(error, crate::error::Error::ImageFromResource(_)),
                "{resource:?}: {error:?}"
            );
        }

        assert!(default_window_icon_from_app_icon_resource().is_none());
    }

    #[test]
    fn from_icon_resource_rejects_invalid_sizes() {
        for (width, height) in [(0, 32), (32, 0), (u32::MAX, 32), (32, i32::MAX as u32 + 1)] {
            let error = Image::from_icon_resource(1, width, height).unwrap_err();
            assert!(
                matches!(error, crate::error::Error::ImageFromResource(_)),
                "{width}x{height}: {error:?}"
            );
        }
    }
}
