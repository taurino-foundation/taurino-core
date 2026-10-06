use std::borrow::Cow;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};

use serde::Deserialize;
#[cfg(windows)]
use windows::{
  Win32::{
    Foundation::{E_FAIL, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, WIN32_ERROR},
    Graphics::Gdi::{
      BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, GetDIBits, HBITMAP,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::WindowsAndMessaging::{
      GetIconInfo, GetSystemMetrics, HICON, ICONINFO, IMAGE_ICON, LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CYICON,
    },
  },
  core::{Owned, PCWSTR},
};

use crate::{
  core::resources::{Resource, ResourceId, ResourceTable},
  schema::window::Icon,
};

pub const WINDOWS_APP_ICON_RESOURCE_ID: u16 = 32512;

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

/// Loads the default window icon from the application icon resource,
/// reporting failures.
///
/// Used by [`crate::generate_context!`]; not public API.
#[cfg(windows)]
#[doc(hidden)]
pub fn default_window_icon_from_app_icon_resource() -> Option<Image<'static>> {
  // The window icon is drawn in the title bar and, as a fallback for the
  // taskbar icon, at the system's large icon size
  // (32x32 at 96 DPI, scaled with the system DPI).
  //
  // Pick the entry Windows would use for the taskbar instead of a larger
  // one it has to shrink.
  //
  // `GetSystemMetrics` returns 0 on failure.
  let metric = |index| match unsafe { GetSystemMetrics(index) } {
    n if n > 0 => n as u32,
    _ => 32,
  };

  let (width, height) = (metric(SM_CXICON), metric(SM_CYICON));

  match Image::from_icon_resource(WINDOWS_APP_ICON_RESOURCE_ID, width, height) {
    Ok(icon) => Some(icon),

    Err(error) => {
      // A logger is usually not installed yet when
      // `generate_context!` runs.
      #[cfg(debug_assertions)]
      eprintln!(
        "failed to load the default window icon \
         from the application icon resource: {error:#}"
      );

      log::warn!(
        "failed to load the default window icon \
         from the application icon resource: {error:#}"
      );

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
/// `hbm` must be a valid bitmap handle and `width` and `height`
/// must be positive.
#[cfg(windows)]
unsafe fn read_bgra(hbm: HBITMAP, width: i32, height: i32) -> Result<Vec<u8>> {
  let image_bytes = (width as usize)
    .checked_mul(height as usize)
    .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
    .ok_or_else(|| resource_error(ERROR_INVALID_PARAMETER, "image size overflows usize"))?;

  let mut bgra = vec![0_u8; image_bytes];

  let mut bitmap_info = BITMAPINFO::default();

  bitmap_info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as _;

  bitmap_info.bmiHeader.biWidth = width;

  // Negative value for top-down.
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

    // Capture the error before `DeleteDC` can overwrite it.
    let error =
      (scan_lines != height).then(|| last_error_or(&format!("GetDIBits copied {scan_lines} of {height} scan lines")));

    let _ = DeleteDC(hdc);

    if let Some(error) = error {
      return Err(anyhow!("failed to read icon bitmap: {error}"));
    }
  }

  Ok(bgra)
}

#[cfg(windows)]
fn resource_error(code: WIN32_ERROR, message: &str) -> anyhow::Error {
  let error = windows::core::Error::new(code.to_hresult(), message);

  anyhow!("{message}: {error}")
}

/// Returns the calling thread's last error, or a generic `E_FAIL`
/// with `message` when no error code was set.
///
/// GDI functions do not always set one.
#[cfg(windows)]
fn last_error_or(message: &str) -> windows::core::Error {
  let error = windows::core::Error::from_thread();

  if error.code().is_ok() {
    windows::core::Error::new(E_FAIL, message)
  } else {
    error
  }
}

/// An RGBA image in row-major order from top to bottom.
#[derive(Clone)]
pub struct Image<'a> {
  pub rgba: Cow<'a, [u8]>,
  pub width: u32,
  pub height: u32,
}

impl std::fmt::Debug for Image<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("Image")
      .field(
        "rgba",
        // Reduces the debug size compared to the derived default,
        // as the default would format the raw bytes as numbers
        // `[0, 0, 0, 0]` for 1 pixel.
        //
        // The custom format doesn't grow as much with larger images:
        //
        // `Image {
        //   rgba: Cow::Borrowed([u8; 4096]),
        //   width: 32,
        //   height: 32
        // }`
        &format_args!(
          "Cow::{}([u8; {}])",
          match &self.rgba {
            Cow::Borrowed(_) => "Borrowed",
            Cow::Owned(_) => "Owned",
          },
          self.rgba.len(),
        ),
      )
      .field("width", &self.width)
      .field("height", &self.height)
      .finish()
  }
}

impl Resource for Image<'static> {}

impl Image<'static> {
  /// Creates a new Image using RGBA data, in row-major order
  /// from top to bottom, and with specified width and height.
  ///
  /// Similar to [`Self::new`] but avoids cloning the RGBA data
  /// to get an owned Image.
  pub const fn new_owned(rgba: Vec<u8>, width: u32, height: u32) -> Self {
    Self {
      rgba: Cow::Owned(rgba),
      width,
      height,
    }
  }
}

impl<'a> Image<'a> {
  /// Creates a new Image using RGBA data, in row-major order
  /// from top to bottom, and with specified width and height.
  pub const fn new(rgba: &'a [u8], width: u32, height: u32) -> Self {
    Self {
      rgba: Cow::Borrowed(rgba),
      width,
      height,
    }
  }

  /// Creates a new image using the provided bytes.
  ///
  /// Only `ico` and `png` are supported.
  pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
    let img = image::load_from_memory(bytes).context("failed to decode image bytes")?;

    let (width, height) = (img.width(), img.height());

    Ok(Self {
      rgba: Cow::Owned(img.into_rgba8().into_raw()),
      width,
      height,
    })
  }

  /// Creates a new image using the provided path.
  ///
  /// Only `ico` and `png` are supported.
  pub fn from_path<P>(path: P) -> Result<Self>
  where
    P: AsRef<std::path::Path>,
  {
    let path = path.as_ref();

    let bytes = std::fs::read(path).with_context(|| format!("failed to read image from `{}`", path.display(),))?;

    Self::from_bytes(&bytes).with_context(|| format!("failed to decode image from `{}`", path.display(),))
  }

  /// Creates a new image from the application icon embedded
  /// in the executable of the current process.
  ///
  /// The application icon is the one `tauri-build` embeds with the
  /// [`WINDOWS_APP_ICON_RESOURCE_ID`] id.
  ///
  /// This could change in the future.
  #[cfg(windows)]
  #[cfg_attr(docsrs, doc(cfg(windows)))]
  pub fn from_app_icon_resource(size: u32) -> Result<Self> {
    Image::from_icon_resource(WINDOWS_APP_ICON_RESOURCE_ID, size, size)
  }

  /// ## Examples
  ///
  /// The resource can be identified by its integer ID or by its name,
  /// see [`IconResource`].
  ///
  /// ```no_run
  /// # use taurino_core::core::image::Image;
  /// # fn main() -> anyhow::Result<()> {
  /// let icon = Image::from_icon_resource(1, 32, 32)?;
  /// let icon = Image::from_icon_resource("icon", 32, 32)?;
  /// # Ok(())
  /// # }
  /// ```
  #[cfg(windows)]
  #[cfg_attr(docsrs, doc(cfg(windows)))]
  pub fn from_icon_resource<'r>(resource: impl Into<IconResource<'r>>, width: u32, height: u32) -> Result<Self> {
    let (width_i32, height_i32) = match (i32::try_from(width), i32::try_from(height)) {
      (Ok(width), Ok(height)) if width > 0 && height > 0 => (width, height),

      _ => {
        return Err(resource_error(
          ERROR_INVALID_PARAMETER,
          "width and height must be between 1 and i32::MAX",
        ));
      }
    };

    // Keeps the wide string alive for the `LoadImageW` call.
    let name: Vec<u16>;

    let resource_id = match resource.into() {
      // MAKEINTRESOURCE
      IconResource::Id(id) => PCWSTR(id as usize as *const u16),

      IconResource::Name(resource_name) => {
        name = resource_name.encode_utf16().chain(std::iter::once(0)).collect();

        PCWSTR(name.as_ptr())
      }
    };

    let module = unsafe { GetModuleHandleW(PCWSTR::null()) }
      .map_err(|error| anyhow!("failed to get the current process module handle: {error}"))?;

    let raw_icon = unsafe {
      LoadImageW(
        Some(module.into()),
        resource_id,
        IMAGE_ICON,
        width_i32,
        height_i32,
        LR_DEFAULTCOLOR,
      )
    }
    .map_err(|error| anyhow!("failed to load icon resource: {error}"))?;

    let hicon = unsafe { Owned::new(HICON(raw_icon.0)) };

    let mut icon_info = ICONINFO::default();

    unsafe { GetIconInfo(*hicon, &mut icon_info) }
      .map_err(|error| anyhow!("failed to retrieve icon information: {error}"))?;

    let hbm_mask = unsafe { Owned::new(icon_info.hbmMask) };

    let hbm_color = unsafe { Owned::new(icon_info.hbmColor) };

    // Monochrome icons only have a mask bitmap
    // (AND mask stacked on top of the XOR mask).
    if hbm_color.is_invalid() {
      return Err(resource_error(
        ERROR_NOT_SUPPORTED,
        "monochrome icons are not supported",
      ));
    }

    let mut bgra =
      unsafe { read_bgra(*hbm_color, width_i32, height_i32) }.context("failed to read icon color bitmap")?;

    // Color bitmaps without an alpha channel
    // (e.g. 24bpp icons) read back with alpha = 0 on every pixel.
    //
    // Recover the alpha channel from the AND mask:
    // a set bit means the pixel is transparent.
    if bgra.as_chunks::<BYTES_PER_PIXEL>().0.iter().all(|pixel| pixel[3] == 0) {
      let mask = unsafe { read_bgra(*hbm_mask, width_i32, height_i32) }.context("failed to read icon mask bitmap")?;

      for (pixel, mask) in bgra
        .as_chunks_mut::<BYTES_PER_PIXEL>()
        .0
        .iter_mut()
        .zip(mask.as_chunks::<BYTES_PER_PIXEL>().0)
      {
        // The 1bpp mask expands to black
        // (clear bit) or white (set bit).
        pixel[3] = if mask[0] == 0 { 0xFF } else { 0 };
      }
    }

    let rgba = {
      for pixel in bgra.as_chunks_mut::<BYTES_PER_PIXEL>().0 {
        // Swap Blue and Red channels.
        pixel.swap(0, 2);
      }

      bgra
    };

    Ok(Image::new_owned(rgba, width, height))
  }

  /// Returns the RGBA data for this image,
  /// in row-major order from top to bottom.
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

  /// Converts into a `'static` owned [`Image`].
  ///
  /// This will allocate if the RGBA data is currently borrowed.
  pub fn to_owned(self) -> Image<'static> {
    Image {
      rgba: match self.rgba {
        Cow::Owned(value) => Cow::Owned(value),

        Cow::Borrowed(value) => Cow::Owned(value.to_vec()),
      },

      height: self.height,
      width: self.width,
    }
  }
}

/// An image type that accepts file paths, raw bytes,
/// previously loaded images and image objects.
///
/// This type is meant to be used along the
/// `transformImage` API.
///
/// # Stability
///
/// The stability of the variants is not guaranteed,
/// and matching against them is not recommended.
///
/// Use [`JsImage::into_img`] instead.
#[derive(Deserialize)]
#[non_exhaustive]
pub enum JsImage {
  /// A reference to an ICO or PNG image in the filesystem.
  #[non_exhaustive]
  Path(std::path::PathBuf),

  /// ICO or PNG image in raw bytes.
  #[non_exhaustive]
  Bytes(Vec<u8>),

  /// An image that was previously loaded with the API
  /// and is stored in the resource table.
  #[non_exhaustive]
  Resource(ResourceId),

  /// Raw RGBA definition of an image.
  #[non_exhaustive]
  Rgba {
    /// Image bytes.
    rgba: Vec<u8>,

    /// Image width.
    width: u32,

    /// Image height.
    height: u32,
  },
}

impl JsImage {
  /// Converts this intermediate image format into an actual [`Image`].
  ///
  /// This retrieves the image from the passed [`ResourceTable`]
  /// if it is [`JsImage::Resource`].
  ///
  /// An error is returned if the resource does not exist or has
  /// the wrong type.
  pub fn into_img(self, resources_table: &ResourceTable) -> Result<Arc<Image<'_>>> {
    match self {
      Self::Resource(rid) => resources_table
        .get::<Image<'static>>(rid)
        .with_context(|| format!("failed to retrieve image resource with id {rid}")),

      Self::Path(path) => Image::from_path(&path)
        .map(Arc::new)
        .with_context(|| format!("failed to load image from `{}`", path.display(),)),

      Self::Bytes(bytes) => Image::from_bytes(&bytes)
        .map(Arc::new)
        .context("failed to load image from raw bytes"),

      Self::Rgba { rgba, width, height } => {
        let image = Image::new_owned(rgba, width, height);

        check_rgba_size(&image).context("invalid raw RGBA image")?;

        Ok(Arc::new(image))
      }
    }
  }
}

pub fn check_rgba_size(img: &Image<'_>) -> Result<()> {
  let expected = (img.width as u64)
    .checked_mul(img.height as u64)
    .and_then(|value| value.checked_mul(4))
    .ok_or_else(|| anyhow!("RGBA image dimensions overflow: {}x{}", img.width, img.height,))?;

  let actual = img.rgba.len() as u64;

  if actual != expected {
    return Err(anyhow!(
      "RGBA buffer has {} bytes but a {}x{} image needs {} bytes",
      actual,
      img.width,
      img.height,
      expected,
    ));
  }

  Ok(())
}

fn check_rgba_dimensions(rgba_len: usize, width: u32, height: u32) -> Result<()> {
  let expected = (width as u64)
    .checked_mul(height as u64)
    .and_then(|value| value.checked_mul(4))
    .ok_or_else(|| anyhow!("RGBA image dimensions overflow: {width}x{height}"))?;

  let actual = rgba_len as u64;

  if actual != expected {
    return Err(anyhow!(
      "RGBA buffer has {} bytes but a {}x{} image needs {} bytes",
      actual,
      width,
      height,
      expected,
    ));
  }

  Ok(())
}
impl TryFrom<Icon<'_>> for tao::window::Icon {
  type Error = anyhow::Error;

  fn try_from(icon: Icon<'_>) -> Result<Self, Self::Error> {
    check_rgba_dimensions(icon.rgba.len(), icon.width, icon.height)?;

    tao::window::Icon::from_rgba(icon.rgba.into_owned(), icon.width, icon.height)
      .map_err(|error| anyhow!("failed to create Tao icon from RGBA data: {error}"))
  }
}

impl TryFrom<Image<'_>> for tray_icon::Icon {
  type Error = anyhow::Error;

  fn try_from(image: Image<'_>) -> Result<Self, Self::Error> {
    check_rgba_dimensions(image.rgba.len(), image.width, image.height)?;

    tray_icon::Icon::from_rgba(image.rgba.into_owned(), image.width, image.height)
      .map_err(|error| anyhow!("failed to create Tao icon from RGBA data: {error}"))
  }
}
impl TryFrom<Image<'_>> for muda::Icon {
  type Error = anyhow::Error;

  fn try_from(image: Image<'_>) -> Result<Self, Self::Error> {
    check_rgba_dimensions(image.rgba.len(), image.width, image.height)?;

    muda::Icon::from_rgba(image.rgba.into_owned(), image.width, image.height)
      .map_err(|error| anyhow!("failed to create muda icon from RGBA data: {error}"))
  }
}

impl<'a> From<Image<'a>> for Icon<'a> {
  fn from(image: Image<'a>) -> Self {
    Self {
      rgba: image.rgba,
      width: image.width,
      height: image.height,
    }
  }
}
