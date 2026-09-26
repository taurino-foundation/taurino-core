// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// a modified version of https://github.com/denoland/deno/blob/0ae83847f498a2886ae32172e50fd5bdbab2f524/core/resources.rs#L220

use std::{
    any::{Any, TypeId, type_name},
    borrow::Cow,
    collections::BTreeMap,
    sync::Arc,
};
    use dpi::Position;
use tao::window::Window;
use muda::{ContextMenu as MudaContextMenu, MenuId};
// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

// Image types used by this crate and also referenced by the JavaScript API layer.

use serde::Deserialize;
#[cfg(windows)]
use windows::{
    Win32::{
        Foundation::{
            E_FAIL, ERROR_INVALID_PARAMETER, ERROR_NOT_SUPPORTED, WIN32_ERROR,
        },
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC,
            DIB_RGB_COLORS, DeleteDC, GetDIBits, HBITMAP,
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            GetIconInfo, GetSystemMetrics, HICON, ICONINFO, IMAGE_ICON,
            LR_DEFAULTCOLOR, LoadImageW, SM_CXICON, SM_CYICON,
        },
    },
    core::{Owned, PCWSTR},
};

use crate::{menu::sealed::{ContextMenuBase, IsMenuItemBase}, utils::Icon};

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
    match Image::from_icon_resource(WINDOWS_APP_ICON_RESOURCE_ID, width, height)
    {
        Ok(icon) => Some(icon),
        Err(e) => {
            // a logger is usually not installed yet when `generate_context!` runs
            #[cfg(debug_assertions)]
            eprintln!(
                "failed to load the default window icon from the application icon resource: {e}"
            );
            log::warn!(
                "failed to load the default window icon from the application icon resource: {e}"
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
/// `hbm` must be a valid bitmap handle and `width` and `height` must be positive.
#[cfg(windows)]
unsafe fn read_bgra(
    hbm: HBITMAP,
    width: i32,
    height: i32,
) -> crate::Result<Vec<u8>> {
    let image_bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(BYTES_PER_PIXEL))
        .ok_or_else(|| {
            resource_error(
                ERROR_INVALID_PARAMETER,
                "image size overflows usize",
            )
        })?;
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
        let error = (scan_lines != height).then(|| {
            last_error_or(&format!(
                "GetDIBits copied {scan_lines} of {height} scan lines"
            ))
        });
        let _ = DeleteDC(hdc);
        if let Some(error) = error {
            return Err(crate::Error::ImageFromResource(error));
        }
    }

    Ok(bgra)
}

#[cfg(windows)]
fn resource_error(code: WIN32_ERROR, message: &str) -> crate::Error {
    crate::Error::ImageFromResource(windows::core::Error::new(
        code.to_hresult(),
        message,
    ))
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

    pub fn from_bytes(bytes: &[u8]) -> crate::Result<Self> {
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

    pub fn from_path<P: AsRef<std::path::Path>>(
        path: P,
    ) -> crate::Result<Self> {
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
    pub fn from_app_icon_resource(size: u32) -> crate::Result<Self> {
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
    /// ```no_run
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
    ) -> crate::Result<Self> {
        let (width_i32, height_i32) =
            match (i32::try_from(width), i32::try_from(height)) {
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
                            .map_err(crate::Error::ImageFromResource)?
                            .into(),
                    ),
                    resource_id,
                    IMAGE_ICON,
                    width_i32,
                    height_i32,
                    LR_DEFAULTCOLOR,
                )
                .map_err(crate::Error::ImageFromResource)?
                .0,
            ))
        };

        let mut icon_info = ICONINFO::default();
        unsafe {
            GetIconInfo(*hicon, &mut icon_info)
                .map_err(crate::Error::ImageFromResource)?
        };
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
        if bgra
            .as_chunks::<BYTES_PER_PIXEL>()
            .0
            .iter()
            .all(|px| px[3] == 0)
        {
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
    type Error = crate::Error;

    fn try_from(img: Image<'_>) -> Result<Self, Self::Error> {
        muda::Icon::from_rgba(img.rgba.into_owned(), img.width, img.height)
            .map_err(Into::into)
    }
}

impl TryFrom<Image<'_>> for tray_icon::Icon {
    type Error = crate::Error;

    fn try_from(img: Image<'_>) -> Result<Self, Self::Error> {
        tray_icon::Icon::from_rgba(img.rgba.into_owned(), img.width, img.height)
            .map_err(Into::into)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::{
        IconResource, Image, default_window_icon_from_app_icon_resource,
    };

    /// The test executable has no icon resources, so every lookup must fail with an error
    /// (instead of panicking or returning a stretched placeholder).
    #[test]
    fn from_icon_resource_missing_resource_is_an_error() {
        for resource in [
            IconResource::Id(u16::MAX),
            IconResource::Name("tauri-image-test-missing-icon"),
        ] {
            let error =
                Image::from_icon_resource(resource, 32, 32).unwrap_err();
            assert!(
                matches!(error, crate::Error::ImageFromResource(_)),
                "{resource:?}: {error:?}"
            );
        }

        assert!(default_window_icon_from_app_icon_resource().is_none());
    }

    #[test]
    fn from_icon_resource_rejects_invalid_sizes() {
        for (width, height) in
            [(0, 32), (32, 0), (u32::MAX, 32), (32, i32::MAX as u32 + 1)]
        {
            let error =
                Image::from_icon_resource(1, width, height).unwrap_err();
            assert!(
                matches!(error, crate::Error::ImageFromResource(_)),
                "{width}x{height}: {error:?}"
            );
        }
    }
}
/// Resources are Rust objects that are stored in [ResourceTable] and managed by tauri.
///
/// They are identified in JS by a numeric ID (the resource ID, or rid).
/// Resources can be created in commands. Resources can also be retrieved in commands by
/// their rid. Resources are thread-safe.
///
/// Resources are reference counted in Rust. This means that they can be
/// cloned and passed around. When the last reference is dropped, the resource
/// is automatically closed. As long as the resource exists in the resource
/// table, the reference count is at least 1.
pub trait Resource: Any + 'static + Send + Sync {
    /// Returns a string representation of the resource. The default implementation
    /// returns the Rust type name, but specific resource types may override this
    /// trait method.
    fn name(&self) -> Cow<'_, str> {
        type_name::<Self>().into()
    }

    /// Resources may implement the `close()` trait method if they need to do
    /// resource specific clean-ups, such as cancelling pending futures, after a
    /// resource has been removed from the resource table.
    fn close(self: Arc<Self>) {}
}

impl dyn Resource {
    #[inline(always)]
    fn is<T: Resource>(&self) -> bool {
        self.type_id() == TypeId::of::<T>()
    }

    #[inline(always)]
    pub(crate) fn downcast_arc<T: Resource>(
        self: &Arc<Self>,
    ) -> Option<&Arc<T>> {
        if self.is::<T>() {
            // A resource is stored as `Arc<T>` in a BTreeMap
            // and is safe to cast to `Arc<T>` because of the runtime
            // check done in `self.is::<T>()`
            let ptr = self as *const Arc<_> as *const Arc<T>;
            Some(unsafe { &*ptr })
        } else {
            None
        }
    }
}

/// A `ResourceId` is an integer value referencing a resource. It could be
/// considered to be the tauri equivalent of a `file descriptor` in POSIX like
/// operating systems.
pub type ResourceId = u32;

/// Map-like data structure storing Tauri's resources (equivalent to file
/// descriptors).
///
/// Provides basic methods for element access. A resource can be of any type.
/// Different types of resources can be stored in the same map, and provided
/// with a name for description.
///
/// Each resource is identified through a _resource ID (rid)_, which acts as
/// the key in the map.
#[derive(Default)]
pub struct ResourceTable {
    index: BTreeMap<ResourceId, Arc<dyn Resource>>,
}

impl ResourceTable {
    fn new_random_rid() -> u32 {
        let mut bytes = [0_u8; 4];
        getrandom::fill(&mut bytes).expect("failed to get random bytes");
        u32::from_ne_bytes(bytes)
    }

    /// Inserts resource into the resource table, which takes ownership of it.
    ///
    /// The resource type is erased at runtime and must be statically known
    /// when retrieving it through `get()`.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add<T: Resource>(&mut self, resource: T) -> ResourceId {
        self.add_arc(Arc::new(resource))
    }

    /// Inserts a `Arc`-wrapped resource into the resource table.
    ///
    /// The resource type is erased at runtime and must be statically known
    /// when retrieving it through `get()`.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc<T: Resource>(&mut self, resource: Arc<T>) -> ResourceId {
        let resource = resource as Arc<dyn Resource>;
        self.add_arc_dyn(resource)
    }

    /// Inserts a `Arc`-wrapped resource into the resource table.
    ///
    /// The resource type is erased at runtime and must be statically known
    /// when retrieving it through `get()`.
    ///
    /// Returns a unique resource ID, which acts as a key for this resource.
    pub fn add_arc_dyn(&mut self, resource: Arc<dyn Resource>) -> ResourceId {
        let mut rid = Self::new_random_rid();
        while self.index.contains_key(&rid) {
            rid = Self::new_random_rid();
        }

        let removed_resource = self.index.insert(rid, resource);
        assert!(removed_resource.is_none());
        rid
    }

    /// Returns true if any resource with the given `rid` exists.
    pub fn has(&self, rid: ResourceId) -> bool {
        self.index.contains_key(&rid)
    }

    /// Returns a reference counted pointer to the resource of type `T` with the
    /// given `rid`. If `rid` is not present or has a type different than `T`,
    /// this function returns [`Error::BadResourceId`](crate::Error::BadResourceId).
    pub fn get<T: Resource>(&self, rid: ResourceId) -> crate::Result<Arc<T>> {
        self.index
            .get(&rid)
            .and_then(|rc| rc.downcast_arc::<T>())
            .cloned()
            .ok_or_else(|| crate::Error::BadResourceId(rid))
    }

    /// Returns a reference counted pointer to the resource of the given `rid`.
    /// If `rid` is not present, this function returns [`crate::Error::BadResourceId`].
    pub fn get_any(&self, rid: ResourceId) -> crate::Result<Arc<dyn Resource>> {
        self.index
            .get(&rid)
            .ok_or_else(|| crate::Error::BadResourceId(rid))
            .cloned()
    }

    /// Replaces a resource with a new resource.
    ///
    /// Panics if the resource does not exist.
    pub fn replace<T: Resource>(&mut self, rid: ResourceId, resource: T) {
        let result = self
            .index
            .insert(rid, Arc::new(resource) as Arc<dyn Resource>);
        assert!(result.is_some());
    }

    /// Removes a resource of type `T` from the resource table and returns it.
    /// If a resource with the given `rid` exists but its type does not match `T`,
    /// it is not removed from the resource table. Note that the resource's
    /// `close()` method is *not* called.
    ///
    /// Also note that there might be a case where
    /// the returned `Arc<T>` is referenced by other variables. That is, we cannot
    /// assume that `Arc::strong_count(&returned_arc)` is always equal to 1 on success.
    /// In particular, be really careful when you want to extract the inner value of
    /// type `T` from `Arc<T>`.
    pub fn take<T: Resource>(
        &mut self,
        rid: ResourceId,
    ) -> crate::Result<Arc<T>> {
        let resource = self.get::<T>(rid)?;
        self.index.remove(&rid);
        Ok(resource)
    }

    /// Removes a resource from the resource table and returns it. Note that the
    /// resource's `close()` method is *not* called.
    ///
    /// Also note that there might be a
    /// case where the returned `Arc<T>` is referenced by other variables. That is,
    /// we cannot assume that `Arc::strong_count(&returned_arc)` is always equal to 1
    /// on success. In particular, be really careful when you want to extract the
    /// inner value of type `T` from `Arc<T>`.
    pub fn take_any(
        &mut self,
        rid: ResourceId,
    ) -> crate::Result<Arc<dyn Resource>> {
        self.index
            .remove(&rid)
            .ok_or_else(|| crate::Error::BadResourceId(rid))
    }

    /// Returns an iterator that yields a `(id, name)` pair for every resource
    /// that's currently in the resource table. This can be used for debugging
    /// purposes. Note that the order in
    /// which items appear is not specified.
    pub fn names(&self) -> impl Iterator<Item = (ResourceId, Cow<'_, str>)> {
        self.index
            .iter()
            .map(|(&id, resource)| (id, resource.name()))
    }

    /// Removes the resource with the given `rid` from the resource table. If the
    /// only reference to this resource existed in the resource table, this will
    /// cause the resource to be dropped. However, since resources are reference
    /// counted, therefore pending ops are not automatically cancelled. A resource
    /// may implement the `close()` method to perform clean-ups such as canceling
    /// ops.
    pub fn close(&mut self, rid: ResourceId) -> crate::Result<()> {
        self.index
            .remove(&rid)
            .ok_or_else(|| crate::Error::BadResourceId(rid))
            .map(|resource| resource.close())
    }

    /// Removes and frees all resources stored. Note that the
    /// resource's `close()` method is *not* called.
    pub(crate) fn clear(&mut self) {
        self.index.clear()
    }
}

/// Application metadata for the [`PredefinedMenuItem::about`].
#[derive(Debug, Clone, Default)]
pub struct AboutMetadata<'a> {
    /// Sets the application name.
    pub name: Option<String>,
    /// The application version.
    pub version: Option<String>,
    /// The short version, e.g. "1.0".
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / Linux:** Appended to the end of `version` in parentheses.
    pub short_version: Option<String>,
    /// The authors of the application.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub authors: Option<Vec<String>>,
    /// Application comments.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub comments: Option<String>,
    /// The copyright of the application.
    pub copyright: Option<String>,
    /// The license of the application.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub license: Option<String>,
    /// The application website.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub website: Option<String>,
    /// The website label.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub website_label: Option<String>,
    /// The credits.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / Linux:** Unsupported.
    pub credits: Option<String>,
    /// The application icon.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:** Unsupported.
    pub icon: Option<Image<'a>>,
}

/// A builder type for [`AboutMetadata`].
#[derive(Clone, Debug, Default)]
pub struct AboutMetadataBuilder<'a>(AboutMetadata<'a>);

impl<'a> AboutMetadataBuilder<'a> {
    /// Create a new about metadata builder.
    pub fn new() -> Self {
        Default::default()
    }

    /// Sets the application name.
    pub fn name<S: Into<String>>(mut self, name: Option<S>) -> Self {
        self.0.name = name.map(|s| s.into());
        self
    }
    /// Sets the application version.
    pub fn version<S: Into<String>>(mut self, version: Option<S>) -> Self {
        self.0.version = version.map(|s| s.into());
        self
    }
    /// Sets the short version, e.g. "1.0".
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / Linux:** Appended to the end of `version` in parentheses.
    pub fn short_version<S: Into<String>>(
        mut self,
        short_version: Option<S>,
    ) -> Self {
        self.0.short_version = short_version.map(|s| s.into());
        self
    }
    /// Sets the authors of the application.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub fn authors(mut self, authors: Option<Vec<String>>) -> Self {
        self.0.authors = authors;
        self
    }
    /// Application comments.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub fn comments<S: Into<String>>(mut self, comments: Option<S>) -> Self {
        self.0.comments = comments.map(|s| s.into());
        self
    }
    /// Sets the copyright of the application.
    pub fn copyright<S: Into<String>>(mut self, copyright: Option<S>) -> Self {
        self.0.copyright = copyright.map(|s| s.into());
        self
    }
    /// Sets the license of the application.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub fn license<S: Into<String>>(mut self, license: Option<S>) -> Self {
        self.0.license = license.map(|s| s.into());
        self
    }
    /// Sets the application website.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub fn website<S: Into<String>>(mut self, website: Option<S>) -> Self {
        self.0.website = website.map(|s| s.into());
        self
    }
    /// Sets the website label.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Unsupported.
    pub fn website_label<S: Into<String>>(
        mut self,
        website_label: Option<S>,
    ) -> Self {
        self.0.website_label = website_label.map(|s| s.into());
        self
    }
    /// Sets the credits.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / Linux:** Unsupported.
    pub fn credits<S: Into<String>>(mut self, credits: Option<S>) -> Self {
        self.0.credits = credits.map(|s| s.into());
        self
    }
    /// Sets the application icon.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:** Unsupported.
    pub fn icon(mut self, icon: Option<Image<'a>>) -> Self {
        self.0.icon = icon;
        self
    }

    /// Construct the final [`AboutMetadata`]
    pub fn build(self) -> AboutMetadata<'a> {
        self.0
    }
}

impl TryFrom<AboutMetadata<'_>> for muda::AboutMetadata {
    type Error = crate::Error;

    fn try_from(value: AboutMetadata<'_>) -> Result<Self, Self::Error> {
        let icon = match value.icon {
            Some(i) => Some(i.try_into()?),
            None => None,
        };

        Ok(Self {
            authors: value.authors,
            name: value.name,
            version: value.version,
            short_version: value.short_version,
            comments: value.comments,
            copyright: value.copyright,
            license: value.license,
            website: value.website,
            website_label: value.website_label,
            credits: value.credits,
            icon,
        })
    }
}

/// A native Icon to be used for the menu item
///
/// ## Platform-specific:
///
/// - **Windows / Linux**: Unsupported.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NativeIcon {
    /// An add item template image.
    Add,
    /// Advanced preferences toolbar icon for the preferences window.
    Advanced,
    /// A Bluetooth template image.
    Bluetooth,
    /// Bookmarks image suitable for a template.
    Bookmarks,
    /// A caution image.
    Caution,
    /// A color panel toolbar icon.
    ColorPanel,
    /// A column view mode template image.
    ColumnView,
    /// A computer icon.
    Computer,
    /// An enter full-screen mode template image.
    EnterFullScreen,
    /// Permissions for all users.
    Everyone,
    /// An exit full-screen mode template image.
    ExitFullScreen,
    /// A cover flow view mode template image.
    FlowView,
    /// A folder image.
    Folder,
    /// A burnable folder icon.
    FolderBurnable,
    /// A smart folder icon.
    FolderSmart,
    /// A link template image.
    FollowLinkFreestanding,
    /// A font panel toolbar icon.
    FontPanel,
    /// A `go back` template image.
    GoLeft,
    /// A `go forward` template image.
    GoRight,
    /// Home image suitable for a template.
    Home,
    /// An iChat Theater template image.
    IChatTheater,
    /// An icon view mode template image.
    IconView,
    /// An information toolbar icon.
    Info,
    /// A template image used to denote invalid data.
    InvalidDataFreestanding,
    /// A generic left-facing triangle template image.
    LeftFacingTriangle,
    /// A list view mode template image.
    ListView,
    /// A locked padlock template image.
    LockLocked,
    /// An unlocked padlock template image.
    LockUnlocked,
    /// A horizontal dash, for use in menus.
    MenuMixedState,
    /// A check mark template image, for use in menus.
    MenuOnState,
    /// A MobileMe icon.
    MobileMe,
    /// A drag image for multiple items.
    MultipleDocuments,
    /// A network icon.
    Network,
    /// A path button template image.
    Path,
    /// General preferences toolbar icon for the preferences window.
    PreferencesGeneral,
    /// A Quick Look template image.
    QuickLook,
    /// A refresh template image.
    RefreshFreestanding,
    /// A refresh template image.
    Refresh,
    /// A remove item template image.
    Remove,
    /// A reveal contents template image.
    RevealFreestanding,
    /// A generic right-facing triangle template image.
    RightFacingTriangle,
    /// A share view template image.
    Share,
    /// A slideshow template image.
    Slideshow,
    /// A badge for a `smart` item.
    SmartBadge,
    /// Small green indicator, similar to iChat's available image.
    StatusAvailable,
    /// Small clear indicator.
    StatusNone,
    /// Small yellow indicator, similar to iChat's idle image.
    StatusPartiallyAvailable,
    /// Small red indicator, similar to iChat's unavailable image.
    StatusUnavailable,
    /// A stop progress template image.
    StopProgressFreestanding,
    /// A stop progress button template image.
    StopProgress,
    /// An image of the empty trash can.
    TrashEmpty,
    /// An image of the full trash can.
    TrashFull,
    /// Permissions for a single user.
    User,
    /// User account toolbar icon for the preferences window.
    UserAccounts,
    /// Permissions for a group of users.
    UserGroup,
    /// Permissions for guests.
    UserGuest,
}

impl From<NativeIcon> for muda::NativeIcon {
    fn from(value: NativeIcon) -> Self {
        match value {
            NativeIcon::Add => muda::NativeIcon::Add,
            NativeIcon::Advanced => muda::NativeIcon::Advanced,
            NativeIcon::Bluetooth => muda::NativeIcon::Bluetooth,
            NativeIcon::Bookmarks => muda::NativeIcon::Bookmarks,
            NativeIcon::Caution => muda::NativeIcon::Caution,
            NativeIcon::ColorPanel => muda::NativeIcon::ColorPanel,
            NativeIcon::ColumnView => muda::NativeIcon::ColumnView,
            NativeIcon::Computer => muda::NativeIcon::Computer,
            NativeIcon::EnterFullScreen => muda::NativeIcon::EnterFullScreen,
            NativeIcon::Everyone => muda::NativeIcon::Everyone,
            NativeIcon::ExitFullScreen => muda::NativeIcon::ExitFullScreen,
            NativeIcon::FlowView => muda::NativeIcon::FlowView,
            NativeIcon::Folder => muda::NativeIcon::Folder,
            NativeIcon::FolderBurnable => muda::NativeIcon::FolderBurnable,
            NativeIcon::FolderSmart => muda::NativeIcon::FolderSmart,
            NativeIcon::FollowLinkFreestanding => {
                muda::NativeIcon::FollowLinkFreestanding
            }
            NativeIcon::FontPanel => muda::NativeIcon::FontPanel,
            NativeIcon::GoLeft => muda::NativeIcon::GoLeft,
            NativeIcon::GoRight => muda::NativeIcon::GoRight,
            NativeIcon::Home => muda::NativeIcon::Home,
            NativeIcon::IChatTheater => muda::NativeIcon::IChatTheater,
            NativeIcon::IconView => muda::NativeIcon::IconView,
            NativeIcon::Info => muda::NativeIcon::Info,
            NativeIcon::InvalidDataFreestanding => {
                muda::NativeIcon::InvalidDataFreestanding
            }
            NativeIcon::LeftFacingTriangle => {
                muda::NativeIcon::LeftFacingTriangle
            }
            NativeIcon::ListView => muda::NativeIcon::ListView,
            NativeIcon::LockLocked => muda::NativeIcon::LockLocked,
            NativeIcon::LockUnlocked => muda::NativeIcon::LockUnlocked,
            NativeIcon::MenuMixedState => muda::NativeIcon::MenuMixedState,
            NativeIcon::MenuOnState => muda::NativeIcon::MenuOnState,
            NativeIcon::MobileMe => muda::NativeIcon::MobileMe,
            NativeIcon::MultipleDocuments => {
                muda::NativeIcon::MultipleDocuments
            }
            NativeIcon::Network => muda::NativeIcon::Network,
            NativeIcon::Path => muda::NativeIcon::Path,
            NativeIcon::PreferencesGeneral => {
                muda::NativeIcon::PreferencesGeneral
            }
            NativeIcon::QuickLook => muda::NativeIcon::QuickLook,
            NativeIcon::RefreshFreestanding => {
                muda::NativeIcon::RefreshFreestanding
            }
            NativeIcon::Refresh => muda::NativeIcon::Refresh,
            NativeIcon::Remove => muda::NativeIcon::Remove,
            NativeIcon::RevealFreestanding => {
                muda::NativeIcon::RevealFreestanding
            }
            NativeIcon::RightFacingTriangle => {
                muda::NativeIcon::RightFacingTriangle
            }
            NativeIcon::Share => muda::NativeIcon::Share,
            NativeIcon::Slideshow => muda::NativeIcon::Slideshow,
            NativeIcon::SmartBadge => muda::NativeIcon::SmartBadge,
            NativeIcon::StatusAvailable => muda::NativeIcon::StatusAvailable,
            NativeIcon::StatusNone => muda::NativeIcon::StatusNone,
            NativeIcon::StatusPartiallyAvailable => {
                muda::NativeIcon::StatusPartiallyAvailable
            }
            NativeIcon::StatusUnavailable => {
                muda::NativeIcon::StatusUnavailable
            }
            NativeIcon::StopProgressFreestanding => {
                muda::NativeIcon::StopProgressFreestanding
            }
            NativeIcon::StopProgress => muda::NativeIcon::StopProgress,
            NativeIcon::TrashEmpty => muda::NativeIcon::TrashEmpty,
            NativeIcon::TrashFull => muda::NativeIcon::TrashFull,
            NativeIcon::User => muda::NativeIcon::User,
            NativeIcon::UserAccounts => muda::NativeIcon::UserAccounts,
            NativeIcon::UserGroup => muda::NativeIcon::UserGroup,
            NativeIcon::UserGuest => muda::NativeIcon::UserGuest,
        }
    }
}

// -----------------------------------------------------------------------------
// wrappers
// -----------------------------------------------------------------------------

macro_rules! gen_wrapper {
    ($ty:ident, $inner:ident) => {
        struct $inner {
            inner: muda::$ty,
        }

        impl $inner {
            #[inline]
            fn new(inner: muda::$ty) -> Self {
                Self { inner }
            }
        }

        // Kept only because crate::Resource requires Send + Sync.
        // This relies on the caller's single-thread invariant.
        unsafe impl Send for $inner {}
        unsafe impl Sync for $inner {}

        pub struct $ty(Arc<$inner>);

        impl Clone for $ty {
            fn clone(&self) -> Self {
                Self(self.0.clone())
            }
        }

        impl Resource for $ty {}
    };
}

gen_wrapper!(Menu, MenuInner);
gen_wrapper!(MenuItem, MenuItemInner);
gen_wrapper!(Submenu, SubmenuInner);
gen_wrapper!(PredefinedMenuItem, PredefinedMenuItemInner);
gen_wrapper!(CheckMenuItem, CheckMenuItemInner);
gen_wrapper!(IconMenuItem, IconMenuItemInner);

// -----------------------------------------------------------------------------
// generic menu-item facade
// -----------------------------------------------------------------------------

pub enum MenuItemKind {
    MenuItem(MenuItem),
    Submenu(Submenu),
    Predefined(PredefinedMenuItem),
    Check(CheckMenuItem),
    Icon(IconMenuItem),
}

impl Clone for MenuItemKind {
    fn clone(&self) -> Self {
        match self {
            Self::MenuItem(v) => Self::MenuItem(v.clone()),
            Self::Submenu(v) => Self::Submenu(v.clone()),
            Self::Predefined(v) => Self::Predefined(v.clone()),
            Self::Check(v) => Self::Check(v.clone()),
            Self::Icon(v) => Self::Icon(v.clone()),
        }
    }
}

impl MenuItemKind {
    pub fn id(&self) -> &MenuId {
        match self {
            Self::MenuItem(v) => v.id(),
            Self::Submenu(v) => v.id(),
            Self::Predefined(v) => v.id(),
            Self::Check(v) => v.id(),
            Self::Icon(v) => v.id(),
        }
    }

    fn inner_muda(&self) -> &dyn muda::IsMenuItem {
        match self {
            Self::MenuItem(v) => v.inner_muda(),
            Self::Submenu(v) => v.inner_muda(),
            Self::Predefined(v) => v.inner_muda(),
            Self::Check(v) => v.inner_muda(),
            Self::Icon(v) => v.inner_muda(),
        }
    }
}


/// A trait that defines a generic item in a menu, which may be one of [`MenuItemKind`]
///
/// # Safety
///
/// This trait is ONLY meant to be implemented internally by the crate.
pub trait IsMenuItem: sealed::IsMenuItemBase {
  /// Returns the kind of this menu item.
  fn kind(&self) -> MenuItemKind;

  /// Returns a unique identifier associated with this menu.
  fn id(&self) -> &MenuId;
}


macro_rules! impl_menu_item {
    ($ty:ident, $variant:ident) => {
        impl sealed::IsMenuItemBase for $ty {
            fn inner_muda(&self) -> &dyn muda::IsMenuItem {
                &self.0.inner
            }
        }

        impl IsMenuItem for $ty {
            fn kind(&self) -> MenuItemKind {
                MenuItemKind::$variant(self.clone())
            }

            fn id(&self) -> &MenuId {
                self.id()
            }
        }
    };
}

impl_menu_item!(MenuItem, MenuItem);
impl_menu_item!(Submenu, Submenu);
impl_menu_item!(PredefinedMenuItem, Predefined);
impl_menu_item!(CheckMenuItem, Check);
impl_menu_item!(IconMenuItem, Icon);

// -----------------------------------------------------------------------------
// normal item
// -----------------------------------------------------------------------------

impl MenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::MenuItem::new(text.as_ref(), enabled, accelerator);
        Ok(Self(Arc::new(MenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner =
            muda::MenuItem::with_id(id, text.as_ref(), enabled, accelerator);
        Ok(Self(Arc::new(MenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }

    pub fn text(&self) -> crate::Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> crate::Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }

    pub fn is_enabled(&self) -> crate::Result<bool> {
        Ok(self.0.inner.is_enabled())
    }

    pub fn set_enabled(&self, enabled: bool) -> crate::Result<()> {
        self.0.inner.set_enabled(enabled);
        Ok(())
    }

    pub fn set_accelerator<S: AsRef<str>>(
        &self,
        accelerator: Option<S>,
    ) -> crate::Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
    }
}

// -----------------------------------------------------------------------------
// check item
// -----------------------------------------------------------------------------

impl CheckMenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        checked: bool,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::CheckMenuItem::new(
            text.as_ref(),
            enabled,
            checked,
            accelerator,
        );
        Ok(Self(Arc::new(CheckMenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        checked: bool,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::CheckMenuItem::with_id(
            id,
            text.as_ref(),
            enabled,
            checked,
            accelerator,
        );
        Ok(Self(Arc::new(CheckMenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> crate::Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> crate::Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }

    pub fn is_enabled(&self) -> crate::Result<bool> {
        Ok(self.0.inner.is_enabled())
    }

    pub fn set_enabled(&self, enabled: bool) -> crate::Result<()> {
        self.0.inner.set_enabled(enabled);
        Ok(())
    }

    pub fn set_accelerator<S: AsRef<str>>(
        &self,
        accelerator: Option<S>,
    ) -> crate::Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
    }

    pub fn is_checked(&self) -> crate::Result<bool> {
        Ok(self.0.inner.is_checked())
    }

    pub fn set_checked(&self, checked: bool) -> crate::Result<()> {
        self.0.inner.set_checked(checked);
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// icon item
// -----------------------------------------------------------------------------

impl IconMenuItem {
    pub fn new<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        icon: Option<Image<'_>>,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let icon = icon.map(TryInto::try_into).transpose()?;
        let inner =
            muda::IconMenuItem::new(text.as_ref(), enabled, icon, accelerator);
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_id<I: Into<MenuId>, T: AsRef<str>, A: AsRef<str>>(
        id: I,
        text: T,
        enabled: bool,
        icon: Option<Image<'_>>,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let icon = icon.map(TryInto::try_into).transpose()?;
        let inner = muda::IconMenuItem::with_id(
            id,
            text.as_ref(),
            enabled,
            icon,
            accelerator,
        );
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_native_icon<T: AsRef<str>, A: AsRef<str>>(
        text: T,
        enabled: bool,
        icon: Option<NativeIcon>,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::IconMenuItem::with_native_icon(
            text.as_ref(),
            enabled,
            icon.map(Into::into),
            accelerator,
        );
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn with_id_and_native_icon<
        I: Into<MenuId>,
        T: AsRef<str>,
        A: AsRef<str>,
    >(
        id: I,
        text: T,
        enabled: bool,
        icon: Option<NativeIcon>,
        accelerator: Option<A>,
    ) -> crate::Result<Self> {
        let accelerator = accelerator.and_then(|s| s.as_ref().parse().ok());
        let inner = muda::IconMenuItem::with_id_and_native_icon(
            id,
            text.as_ref(),
            enabled,
            icon.map(Into::into),
            accelerator,
        );
        Ok(Self(Arc::new(IconMenuItemInner::new(inner))))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> crate::Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> crate::Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }

    pub fn is_enabled(&self) -> crate::Result<bool> {
        Ok(self.0.inner.is_enabled())
    }

    pub fn set_enabled(&self, enabled: bool) -> crate::Result<()> {
        self.0.inner.set_enabled(enabled);
        Ok(())
    }

    pub fn set_accelerator<S: AsRef<str>>(
        &self,
        accelerator: Option<S>,
    ) -> crate::Result<()> {
        let accel = accelerator.and_then(|s| s.as_ref().parse().ok());
        self.0.inner.set_accelerator(accel).map_err(Into::into)
    }

    pub fn set_icon(&self, icon: Option<Image<'_>>) -> crate::Result<()> {
        let icon = icon.map(TryInto::try_into).transpose()?;
        self.0.inner.set_icon(icon);
        Ok(())
    }

    pub fn set_native_icon(
        &self,
        icon: Option<NativeIcon>,
    ) -> crate::Result<()> {
        #[cfg(target_os = "macos")]
        self.0.inner.set_native_icon(icon.map(Into::into));
        let _ = icon;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// submenu
// -----------------------------------------------------------------------------

impl Submenu {
    pub fn new<S: AsRef<str>>(text: S, enabled: bool) -> crate::Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::new(
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
    ) -> crate::Result<Self> {
        Ok(Self(Arc::new(SubmenuInner::new(muda::Submenu::with_id(
            id,
            text.as_ref(),
            enabled,
        )))))
    }

    pub fn new_with_icon<S: AsRef<str>>(
        text: S,
        enabled: bool,
        icon: Option<Image<'_>>,
    ) -> crate::Result<Self> {
        let submenu = muda::Submenu::new(text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_icon(Some(icon.try_into()?));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_id_and_icon<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        icon: Option<Image<'_>>,
    ) -> crate::Result<Self> {
        let submenu = muda::Submenu::with_id(id, text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_icon(Some(icon.try_into()?));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn new_with_native_icon<S: AsRef<str>>(
        text: S,
        enabled: bool,
        icon: Option<NativeIcon>,
    ) -> crate::Result<Self> {
        let submenu = muda::Submenu::new(text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_native_icon(Some(icon.into()));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_id_and_native_icon<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        icon: Option<NativeIcon>,
    ) -> crate::Result<Self> {
        let submenu = muda::Submenu::with_id(id, text.as_ref(), enabled);
        if let Some(icon) = icon {
            submenu.set_native_icon(Some(icon.into()));
        }
        Ok(Self(Arc::new(SubmenuInner::new(submenu))))
    }

    pub fn with_items<S: AsRef<str>>(
        text: S,
        enabled: bool,
        items: &[&dyn IsMenuItem],
    ) -> crate::Result<Self> {
        let submenu = Self::new(text, enabled)?;
        submenu.append_items(items)?;
        Ok(submenu)
    }

    pub fn with_id_and_items<I: Into<MenuId>, S: AsRef<str>>(
        id: I,
        text: S,
        enabled: bool,
        items: &[&dyn IsMenuItem],
    ) -> crate::Result<Self> {
        let submenu = Self::with_id(id, text, enabled)?;
        submenu.append_items(items)?;
        Ok(submenu)
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> crate::Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn append(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.append(item.inner_muda()).map_err(Into::into)
    }

    pub fn append_items(&self, items: &[&dyn IsMenuItem]) -> crate::Result<()> {
        for item in items {
            self.append(*item)?;
        }
        Ok(())
    }

    pub fn prepend(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.prepend(item.inner_muda()).map_err(Into::into)
    }

    pub fn insert(
        &self,
        item: &dyn IsMenuItem,
        position: usize,
    ) -> crate::Result<()> {
        self.0
            .inner
            .insert(item.inner_muda(), position)
            .map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> crate::Result<Vec<MenuItemKind>> {
        Ok(self
            .0
            .inner
            .items()
            .into_iter()
            .map(MenuItemKind::from_muda)
            .collect())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> crate::Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }

    pub fn is_enabled(&self) -> crate::Result<bool> {
        Ok(self.0.inner.is_enabled())
    }

    pub fn set_enabled(&self, enabled: bool) -> crate::Result<()> {
        self.0.inner.set_enabled(enabled);
        Ok(())
    }

    pub fn set_icon(&self, icon: Option<Image<'_>>) -> crate::Result<()> {
        let icon = icon.map(TryInto::try_into).transpose()?;
        self.0.inner.set_icon(icon);
        Ok(())
    }

    pub fn set_native_icon(
        &self,
        icon: Option<NativeIcon>,
    ) -> crate::Result<()> {
        #[cfg(target_os = "macos")]
        self.0.inner.set_native_icon(icon.map(Into::into));
        let _ = icon;
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// menu
// -----------------------------------------------------------------------------

impl Menu {
    pub fn new() -> crate::Result<Self> {
        Ok(Self(Arc::new(MenuInner::new(muda::Menu::new()))))
    }

    pub fn with_id<I: Into<MenuId>>(id: I) -> crate::Result<Self> {
        Ok(Self(Arc::new(MenuInner::new(muda::Menu::with_id(id)))))
    }

    pub fn with_items(items: &[&dyn IsMenuItem]) -> crate::Result<Self> {
        let menu = Self::new()?;
        menu.append_items(items)?;
        Ok(menu)
    }

    pub fn with_id_and_items<I: Into<MenuId>>(
        id: I,
        items: &[&dyn IsMenuItem],
    ) -> crate::Result<Self> {
        let menu = Self::with_id(id)?;
        menu.append_items(items)?;
        Ok(menu)
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }

    pub fn append(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.append(item.inner_muda()).map_err(Into::into)
    }

    pub fn append_items(&self, items: &[&dyn IsMenuItem]) -> crate::Result<()> {
        for item in items {
            self.append(*item)?;
        }
        Ok(())
    }

    pub fn prepend(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.prepend(item.inner_muda()).map_err(Into::into)
    }

    pub fn insert(
        &self,
        item: &dyn IsMenuItem,
        position: usize,
    ) -> crate::Result<()> {
        self.0
            .inner
            .insert(item.inner_muda(), position)
            .map_err(Into::into)
    }

    pub fn remove(&self, item: &dyn IsMenuItem) -> crate::Result<()> {
        self.0.inner.remove(item.inner_muda()).map_err(Into::into)
    }

    pub fn items(&self) -> crate::Result<Vec<MenuItemKind>> {
        Ok(self
            .0
            .inner
            .items()
            .into_iter()
            .map(MenuItemKind::from_muda)
            .collect())
    }
}

impl MenuItemKind {
    fn from_muda(item: muda::MenuItemKind) -> Self {
        match item {
            muda::MenuItemKind::MenuItem(v) => {
                Self::MenuItem(MenuItem(Arc::new(MenuItemInner::new(v))))
            }
            muda::MenuItemKind::Submenu(v) => {
                Self::Submenu(Submenu(Arc::new(SubmenuInner::new(v))))
            }
            muda::MenuItemKind::Predefined(v) => Self::Predefined(
                PredefinedMenuItem(Arc::new(PredefinedMenuItemInner::new(v))),
            ),
            muda::MenuItemKind::Check(v) => {
                Self::Check(CheckMenuItem(Arc::new(CheckMenuItemInner::new(v))))
            }
            muda::MenuItemKind::Icon(v) => {
                Self::Icon(IconMenuItem(Arc::new(IconMenuItemInner::new(v))))
            }
        }
    }
}

// -----------------------------------------------------------------------------
// predefined items: direct muda calls, no manager and no main-thread dispatch
// -----------------------------------------------------------------------------

impl PredefinedMenuItem {
    fn wrap(inner: muda::PredefinedMenuItem) -> crate::Result<Self> {
        Ok(Self(Arc::new(PredefinedMenuItemInner::new(inner))))
    }

    pub fn separator() -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::separator())
    }
    pub fn copy(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::copy(text))
    }
    pub fn cut(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::cut(text))
    }
    pub fn paste(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::paste(text))
    }
    pub fn select_all(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::select_all(text))
    }
    pub fn undo(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::undo(text))
    }
    pub fn redo(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::redo(text))
    }
    pub fn minimize(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::minimize(text))
    }
    pub fn maximize(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::maximize(text))
    }
    pub fn fullscreen(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::fullscreen(text))
    }
    pub fn hide(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::hide(text))
    }
    pub fn hide_others(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::hide_others(text))
    }
    pub fn show_all(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::show_all(text))
    }
    pub fn close_window(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::close_window(text))
    }
    pub fn quit(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::quit(text))
    }
    pub fn services(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::services(text))
    }
    pub fn bring_all_to_front(text: Option<&str>) -> crate::Result<Self> {
        Self::wrap(muda::PredefinedMenuItem::bring_all_to_front(text))
    }

    pub fn about(
        text: Option<&str>,
        metadata: Option<AboutMetadata<'_>>,
    ) -> crate::Result<Self> {
        let metadata = metadata.map(TryInto::try_into).transpose()?;
        Self::wrap(muda::PredefinedMenuItem::about(text, metadata))
    }

    pub fn id(&self) -> &MenuId {
        self.0.inner.id()
    }
    pub fn text(&self) -> crate::Result<String> {
        Ok(self.0.inner.text())
    }

    pub fn set_text<S: AsRef<str>>(&self, text: S) -> crate::Result<()> {
        self.0.inner.set_text(text.as_ref());
        Ok(())
    }
}

// -----------------------------------------------------------------------------
// item builders
// -----------------------------------------------------------------------------

pub struct MenuItemBuilder {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    accelerator: Option<String>,
}

impl MenuItemBuilder {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn build(self) -> crate::Result<MenuItem> {
        match self.id {
            Some(id) => {
                MenuItem::with_id(id, self.text, self.enabled, self.accelerator)
            }
            None => MenuItem::new(self.text, self.enabled, self.accelerator),
        }
    }
}

pub struct CheckMenuItemBuilder {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    checked: bool,
    accelerator: Option<String>,
}

impl CheckMenuItemBuilder {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            checked: true,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            checked: true,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn checked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn build(self) -> crate::Result<CheckMenuItem> {
        match self.id {
            Some(id) => CheckMenuItem::with_id(
                id,
                self.text,
                self.enabled,
                self.checked,
                self.accelerator,
            ),
            None => CheckMenuItem::new(
                self.text,
                self.enabled,
                self.checked,
                self.accelerator,
            ),
        }
    }
}

pub struct IconMenuItemBuilder<'a> {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    icon: Option<Image<'a>>,
    native_icon: Option<NativeIcon>,
    accelerator: Option<String>,
}

impl<'a> IconMenuItemBuilder<'a> {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            icon: None,
            native_icon: None,
            accelerator: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            icon: None,
            native_icon: None,
            accelerator: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
    pub fn accelerator<S: AsRef<str>>(mut self, accelerator: S) -> Self {
        self.accelerator = Some(accelerator.as_ref().into());
        self
    }

    pub fn icon(mut self, icon: Image<'a>) -> Self {
        self.icon = Some(icon);
        self.native_icon = None;
        self
    }

    pub fn native_icon(mut self, icon: NativeIcon) -> Self {
        self.native_icon = Some(icon);
        self.icon = None;
        self
    }

    pub fn build(self) -> crate::Result<IconMenuItem> {
        match (self.id, self.icon, self.native_icon) {
            (Some(id), Some(icon), _) => IconMenuItem::with_id(
                id,
                self.text,
                self.enabled,
                Some(icon),
                self.accelerator,
            ),
            (None, Some(icon), _) => IconMenuItem::new(
                self.text,
                self.enabled,
                Some(icon),
                self.accelerator,
            ),
            (Some(id), None, native) => IconMenuItem::with_id_and_native_icon(
                id,
                self.text,
                self.enabled,
                native,
                self.accelerator,
            ),
            (None, None, native) => IconMenuItem::with_native_icon(
                self.text,
                self.enabled,
                native,
                self.accelerator,
            ),
        }
    }
}

// -----------------------------------------------------------------------------
// fluent MenuBuilder / SubmenuBuilder
// -----------------------------------------------------------------------------

pub struct MenuBuilder {
    id: Option<MenuId>,
    items: Vec<crate::Result<MenuItemKind>>,
}

impl MenuBuilder {
    pub fn new() -> Self {
        Self {
            id: None,
            items: Vec::new(),
        }
    }

    pub fn with_id<I: Into<MenuId>>(id: I) -> Self {
        Self {
            id: Some(id.into()),
            items: Vec::new(),
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn item(mut self, item: &dyn IsMenuItem) -> Self {
        self.items.push(Ok(item.kind()));
        self
    }

    pub fn items(mut self, items: &[&dyn IsMenuItem]) -> Self {
        for item in items {
            self = self.item(*item);
        }
        self
    }

    pub fn text<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
    ) -> Self {
        self.items.push(
            MenuItem::with_id(id, text, true, None::<&str>)
                .map(MenuItemKind::MenuItem),
        );
        self
    }

    pub fn check<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
    ) -> Self {
        self.items.push(
            CheckMenuItem::with_id(id, text, true, true, None::<&str>)
                .map(MenuItemKind::Check),
        );
        self
    }

    pub fn icon<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
        icon: Image<'_>,
    ) -> Self {
        self.items.push(
            IconMenuItem::with_id(id, text, true, Some(icon), None::<&str>)
                .map(MenuItemKind::Icon),
        );
        self
    }

    pub fn native_icon<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
        icon: NativeIcon,
    ) -> Self {
        self.items.push(
            IconMenuItem::with_id_and_native_icon(
                id,
                text,
                true,
                Some(icon),
                None::<&str>,
            )
            .map(MenuItemKind::Icon),
        );
        self
    }

    pub fn separator(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::separator().map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn copy(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::copy(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn cut(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::cut(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn paste(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::paste(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn select_all(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::select_all(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn undo(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::undo(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn redo(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::redo(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn minimize(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::minimize(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn maximize(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::maximize(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn fullscreen(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::fullscreen(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn hide(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::hide(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn hide_others(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::hide_others(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn show_all(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::show_all(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn close_window(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::close_window(None)
                .map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn quit(mut self) -> Self {
        self.items
            .push(PredefinedMenuItem::quit(None).map(MenuItemKind::Predefined));
        self
    }
    pub fn services(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::services(None).map(MenuItemKind::Predefined),
        );
        self
    }
    pub fn bring_all_to_front(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::bring_all_to_front(None)
                .map(MenuItemKind::Predefined),
        );
        self
    }

    pub fn about(mut self, metadata: Option<AboutMetadata<'_>>) -> Self {
        self.items.push(
            PredefinedMenuItem::about(None, metadata)
                .map(MenuItemKind::Predefined),
        );
        self
    }

    pub fn build(self) -> crate::Result<Menu> {
        let menu = match self.id {
            Some(id) => Menu::with_id(id)?,
            None => Menu::new()?,
        };
        for item in self.items {
            menu.append(&item?)?;
        }
        Ok(menu)
    }
}

impl Default for MenuBuilder {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SubmenuBuilder<'a> {
    id: Option<MenuId>,
    text: String,
    enabled: bool,
    items: Vec<crate::Result<MenuItemKind>>,
    icon: Option<Image<'a>>,
    native_icon: Option<NativeIcon>,
}

impl<'a> SubmenuBuilder<'a> {
    pub fn new<S: AsRef<str>>(text: S) -> Self {
        Self {
            id: None,
            text: text.as_ref().into(),
            enabled: true,
            items: Vec::new(),
            icon: None,
            native_icon: None,
        }
    }

    pub fn with_id<I: Into<MenuId>, S: AsRef<str>>(id: I, text: S) -> Self {
        Self {
            id: Some(id.into()),
            text: text.as_ref().into(),
            enabled: true,
            items: Vec::new(),
            icon: None,
            native_icon: None,
        }
    }

    pub fn id<I: Into<MenuId>>(mut self, id: I) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn submenu_icon(mut self, icon: Image<'a>) -> Self {
        self.icon = Some(icon);
        self.native_icon = None;
        self
    }

    pub fn submenu_native_icon(mut self, icon: NativeIcon) -> Self {
        self.native_icon = Some(icon);
        self.icon = None;
        self
    }

    pub fn item(mut self, item: &dyn IsMenuItem) -> Self {
        self.items.push(Ok(item.kind()));
        self
    }

    pub fn items(mut self, items: &[&dyn IsMenuItem]) -> Self {
        for item in items {
            self = self.item(*item);
        }
        self
    }

    pub fn text_item<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
    ) -> Self {
        self.items.push(
            MenuItem::with_id(id, text, true, None::<&str>)
                .map(MenuItemKind::MenuItem),
        );
        self
    }

    pub fn check<I: Into<MenuId>, S: AsRef<str>>(
        mut self,
        id: I,
        text: S,
    ) -> Self {
        self.items.push(
            CheckMenuItem::with_id(id, text, true, true, None::<&str>)
                .map(MenuItemKind::Check),
        );
        self
    }

    pub fn separator(mut self) -> Self {
        self.items.push(
            PredefinedMenuItem::separator().map(MenuItemKind::Predefined),
        );
        self
    }

    pub fn build(self) -> crate::Result<Submenu> {
        let submenu = match (self.id, self.icon, self.native_icon) {
            (Some(id), Some(icon), _) => Submenu::with_id_and_icon(
                id,
                self.text,
                self.enabled,
                Some(icon),
            )?,
            (None, Some(icon), _) => {
                Submenu::new_with_icon(self.text, self.enabled, Some(icon))?
            }
            (Some(id), None, Some(icon)) => Submenu::with_id_and_native_icon(
                id,
                self.text,
                self.enabled,
                Some(icon),
            )?,
            (None, None, Some(icon)) => Submenu::new_with_native_icon(
                self.text,
                self.enabled,
                Some(icon),
            )?,
            (Some(id), None, None) => {
                Submenu::with_id(id, self.text, self.enabled)?
            }
            (None, None, None) => Submenu::new(self.text, self.enabled)?,
        };
        for item in self.items {
            submenu.append(&item?)?;
        }
        Ok(submenu)
    }
}


impl sealed::IsMenuItemBase for MenuItemKind {
  fn inner_muda(&self) -> &dyn muda::IsMenuItem {
    self.inner_muda()
  }
}

impl IsMenuItem for MenuItemKind {
  fn kind(&self) -> MenuItemKind {
    self.clone()
  }

  fn id(&self) -> &MenuId {
    self.id()
  }
}

 


/// A helper trait with methods to help creating a context menu.
///
/// # Safety
///
/// This trait is ONLY meant to be implemented internally by the crate.
pub trait ContextMenu: sealed::ContextMenuBase + Send + Sync {
  /// Get the popup [`HMENU`] for this menu.
  ///
  /// The returned [`HMENU`] is valid as long as the [`ContextMenu`] is.
  ///
  /// [`HMENU`]: https://learn.microsoft.com/en-us/windows/win32/winprog/windows-data-types#HMENU
  #[cfg(windows)]
  #[cfg_attr(docsrs, doc(cfg(windows)))]
  fn hpopupmenu(&self) -> crate::Result<isize>;

  /// Popup this menu as a context menu on the specified window at the cursor position.
  fn popup(&self, window:&Window) -> crate::Result<()>;

  /// Popup this menu as a context menu on the specified window at the specified position.
  ///
  /// The position is relative to the window's top-left corner.
  fn popup_at<P: Into<Position>>(
    &self,
    window:&Window,
    position: P,
  ) -> crate::Result<()>;
}

pub(crate) mod sealed {
    use dpi::Position;
    use tao::window::Window;



  pub trait IsMenuItemBase {
    fn inner_muda(&self) -> &dyn muda::IsMenuItem;
  }

  pub trait ContextMenuBase {
    fn inner_context(&self) -> &dyn muda::ContextMenu;
    fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu>;
    fn popup_inner<P: Into<Position>>(
      &self,
      window:&Window,
      position: Option<P>,
    ) -> crate::Result<()>;
  }
}





fn show_context_menu(
    menu: &dyn MudaContextMenu,
    window: &Window,
    position: Option<Position>,
) -> crate::Result<()> {
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;

        unsafe {
            menu.show_context_menu_for_hwnd(window.hwnd(), position);
        }
    }

    #[cfg(target_os = "macos")]
    {
        use tao::platform::macos::WindowExtMacOS;

        unsafe {
            menu.show_context_menu_for_nsview(window.ns_view() as _, position);
        }
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    {
        use tao::platform::unix::WindowExtUnix;

        menu.show_context_menu_for_gtk_window(window.gtk_window().as_ref(), position);
    }

    Ok(())
}

impl ContextMenu for Menu {
    #[cfg(windows)]
    fn hpopupmenu(&self) -> crate::Result<isize> {
        Ok(self.0.inner.hpopupmenu())
    }

    fn popup(&self, window: &Window) -> crate::Result<()> {
        self.popup_inner(window, None::<Position>)
    }

    fn popup_at<P: Into<Position>>(
        &self,
        window: &Window,
        position: P,
    ) -> crate::Result<()> {
        self.popup_inner(window, Some(position))
    }
}

impl sealed::ContextMenuBase for Menu {
    fn inner_context(&self) -> &dyn muda::ContextMenu {
        &self.0.inner
    }

    fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu> {
        Box::new(self.0.inner.clone())
    }

    fn popup_inner<P: Into<Position>>(
        &self,
        window: &Window,
        position: Option<P>,
    ) -> crate::Result<()> {
        show_context_menu(&self.0.inner, window, position.map(Into::into))
    }
}

impl ContextMenu for Submenu {
    #[cfg(windows)]
    fn hpopupmenu(&self) -> crate::Result<isize> {
        Ok(self.0.inner.hpopupmenu())
    }

    fn popup(&self, window: &Window) -> crate::Result<()> {
        self.popup_inner(window, None::<Position>)
    }

    fn popup_at<P: Into<Position>>(
        &self,
        window: &Window,
        position: P,
    ) -> crate::Result<()> {
        self.popup_inner(window, Some(position))
    }
}

impl sealed::ContextMenuBase for Submenu {
    fn inner_context(&self) -> &dyn muda::ContextMenu {
        &self.0.inner
    }

    fn inner_context_owned(&self) -> Box<dyn muda::ContextMenu> {
        Box::new(self.0.inner.clone())
    }

    fn popup_inner<P: Into<Position>>(
        &self,
        window: &Window,
        position: Option<P>,
    ) -> crate::Result<()> {
        show_context_menu(&self.0.inner, window, position.map(Into::into))
    }
}















/// ---------------------------------------------------------------------------------------------------------------------------------------------
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// PROTOCOL
// ============================================================================

pub const MENU_PROTOCOL_VERSION: u32 = 1;


// ============================================================================
// INCOMING COMMAND
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum MenuCommand {
    CreateMenu {
        version: u32,
        request_id: String,
        menu: MenuDefinition,
    },
}


// ============================================================================
// MENU DEFINITION
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MenuDefinition {
    pub id: Option<String>,

    #[serde(default)]
    pub items: Vec<MenuEntry>,
}


// ============================================================================
// MENU ENTRY
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "snake_case"
)]
pub enum MenuEntry {
    Item {
        id: String,
        text: String,

        #[serde(default = "default_true")]
        enabled: bool,

        #[serde(default)]
        accelerator: Option<String>,
    },

    Check {
        id: String,
        text: String,

        #[serde(default = "default_true")]
        enabled: bool,

        #[serde(default)]
        checked: bool,

        #[serde(default)]
        accelerator: Option<String>,
    },

    Icon {
        id: String,
        text: String,

        #[serde(default = "default_true")]
        enabled: bool,

        #[serde(default)]
        accelerator: Option<String>,

        icon: IconDefinition,
    },

    Submenu {
        #[serde(default)]
        id: Option<String>,

        text: String,

        #[serde(default = "default_true")]
        enabled: bool,

        #[serde(default)]
        icon: Option<IconDefinition>,

        #[serde(default)]
        items: Vec<MenuEntry>,
    },

    Separator,

    Predefined {
        item: PredefinedKind,

        #[serde(default)]
        text: Option<String>,
    },
}


fn default_true() -> bool {
    true
}


// ============================================================================
// PREDEFINED ITEMS
// ============================================================================

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PredefinedKind {
    Copy,
    Cut,
    Paste,
    SelectAll,

    Undo,
    Redo,

    Minimize,
    Maximize,
    Fullscreen,

    Hide,
    HideOthers,
    ShowAll,

    CloseWindow,
    Quit,

    Services,
    BringAllToFront,
}


// ============================================================================
// ICON DEFINITION
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "snake_case"
)]
pub enum IconDefinition {
    Rgba {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },

    Native {
        icon: NativeIcon,
    },
}


// ============================================================================
// BUILD ERROR
// ============================================================================

#[derive(Debug)]
pub enum MenuBuildError {
    Native(crate::Error),

    Json(serde_json::Error),

    EmptyId,

    DuplicateId(String),

    InvalidImage {
        expected: usize,
        actual: usize,
    },

    UnsupportedProtocolVersion(u32),
}


impl From<crate::Error> for MenuBuildError {
    fn from(error: crate::Error) -> Self {
        Self::Native(error)
    }
}


impl From<serde_json::Error> for MenuBuildError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}


impl std::fmt::Display for MenuBuildError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Native(error) => {
                write!(f, "{error}")
            }

            Self::Json(error) => {
                write!(f, "invalid menu json: {error}")
            }

            Self::EmptyId => {
                write!(f, "menu id must not be empty")
            }

            Self::DuplicateId(id) => {
                write!(f, "duplicate menu id: {id}")
            }

            Self::InvalidImage {
                expected,
                actual,
            } => {
                write!(
                    f,
                    "invalid RGBA image size: expected {expected} bytes, got {actual}"
                )
            }

            Self::UnsupportedProtocolVersion(version) => {
                write!(
                    f,
                    "unsupported menu protocol version: {version}"
                )
            }
        }
    }
}


impl std::error::Error for MenuBuildError {}


// ============================================================================
// BUILD CONTEXT
// ============================================================================

#[derive(Debug)]
pub struct MenuBuildContext {
    root_menu_id: Option<String>,

    // Alle IDs, damit wir doppelte IDs verhindern.
    seen_ids: HashSet<String>,

    // IDs, die später MenuEvents erzeugen können.
    event_ids: HashSet<String>,
}


impl MenuBuildContext {
    pub fn new(root_menu_id: Option<String>) -> Self {
        Self {
            root_menu_id,
            seen_ids: HashSet::new(),
            event_ids: HashSet::new(),
        }
    }


    pub fn root_menu_id(&self) -> Option<&str> {
        self.root_menu_id.as_deref()
    }


    pub fn event_ids(&self) -> &HashSet<String> {
        &self.event_ids
    }


    fn register_id(
        &mut self,
        id: &str,
    ) -> Result<(), MenuBuildError> {
        if id.trim().is_empty() {
            return Err(MenuBuildError::EmptyId);
        }

        if !self.seen_ids.insert(id.to_string()) {
            return Err(
                MenuBuildError::DuplicateId(
                    id.to_string()
                )
            );
        }

        Ok(())
    }


    fn register_event_id(
        &mut self,
        id: &str,
    ) -> Result<(), MenuBuildError> {
        self.register_id(id)?;

        self.event_ids.insert(
            id.to_string()
        );

        Ok(())
    }
}


// ============================================================================
// IMAGE VALIDATION
// ============================================================================

fn validate_rgba(
    width: u32,
    height: u32,
    data: &[u8],
) -> Result<(), MenuBuildError> {
    let expected = width as usize
        * height as usize
        * 4;

    if data.len() != expected {
        return Err(
            MenuBuildError::InvalidImage {
                expected,
                actual: data.len(),
            }
        );
    }

    Ok(())
}


// ============================================================================
// PREDEFINED BUILDER
// ============================================================================

fn build_predefined(
    kind: PredefinedKind,
    text: Option<String>,
) -> Result<MenuItemKind, MenuBuildError> {
    let text = text.as_deref();

    let item = match kind {
        PredefinedKind::Copy => {
            PredefinedMenuItem::copy(text)?
        }

        PredefinedKind::Cut => {
            PredefinedMenuItem::cut(text)?
        }

        PredefinedKind::Paste => {
            PredefinedMenuItem::paste(text)?
        }

        PredefinedKind::SelectAll => {
            PredefinedMenuItem::select_all(text)?
        }

        PredefinedKind::Undo => {
            PredefinedMenuItem::undo(text)?
        }

        PredefinedKind::Redo => {
            PredefinedMenuItem::redo(text)?
        }

        PredefinedKind::Minimize => {
            PredefinedMenuItem::minimize(text)?
        }

        PredefinedKind::Maximize => {
            PredefinedMenuItem::maximize(text)?
        }

        PredefinedKind::Fullscreen => {
            PredefinedMenuItem::fullscreen(text)?
        }

        PredefinedKind::Hide => {
            PredefinedMenuItem::hide(text)?
        }

        PredefinedKind::HideOthers => {
            PredefinedMenuItem::hide_others(text)?
        }

        PredefinedKind::ShowAll => {
            PredefinedMenuItem::show_all(text)?
        }

        PredefinedKind::CloseWindow => {
            PredefinedMenuItem::close_window(text)?
        }

        PredefinedKind::Quit => {
            PredefinedMenuItem::quit(text)?
        }

        PredefinedKind::Services => {
            PredefinedMenuItem::services(text)?
        }

        PredefinedKind::BringAllToFront => {
            PredefinedMenuItem::bring_all_to_front(text)?
        }
    };

    Ok(
        MenuItemKind::Predefined(item)
    )
}


// ============================================================================
// ICON MENU ITEM BUILDER
// ============================================================================

fn build_icon_item(
    id: String,
    text: String,
    enabled: bool,
    accelerator: Option<String>,
    icon: IconDefinition,
) -> Result<IconMenuItem, MenuBuildError> {
    match icon {
        IconDefinition::Rgba {
            width,
            height,
            data,
        } => {
            validate_rgba(
                width,
                height,
                &data,
            )?;

            let image = Image::new_owned(
                data,
                width,
                height,
            );

            Ok(
                IconMenuItem::with_id(
                    id,
                    text,
                    enabled,
                    Some(image),
                    accelerator,
                )?
            )
        }

        IconDefinition::Native {
            icon,
        } => {
            Ok(
                IconMenuItem::with_id_and_native_icon(
                    id,
                    text,
                    enabled,
                    Some(icon),
                    accelerator,
                )?
            )
        }
    }
}


// ============================================================================
// SUBMENU BUILDER
// ============================================================================

fn build_submenu(
    id: Option<String>,
    text: String,
    enabled: bool,
    icon: Option<IconDefinition>,
    items: Vec<MenuEntry>,
    context: &mut MenuBuildContext,
) -> Result<Submenu, MenuBuildError> {
    if let Some(id) = id.as_deref() {
        context.register_id(id)?;
    }

    let submenu = match (id, icon) {
        (
            Some(id),
            Some(
                IconDefinition::Rgba {
                    width,
                    height,
                    data,
                }
            ),
        ) => {
            validate_rgba(
                width,
                height,
                &data,
            )?;

            let image = Image::new_owned(
                data,
                width,
                height,
            );

            Submenu::with_id_and_icon(
                id,
                text,
                enabled,
                Some(image),
            )?
        }

        (
            None,
            Some(
                IconDefinition::Rgba {
                    width,
                    height,
                    data,
                }
            ),
        ) => {
            validate_rgba(
                width,
                height,
                &data,
            )?;

            let image = Image::new_owned(
                data,
                width,
                height,
            );

            Submenu::new_with_icon(
                text,
                enabled,
                Some(image),
            )?
        }

        (
            Some(id),
            Some(
                IconDefinition::Native {
                    icon,
                }
            ),
        ) => {
            Submenu::with_id_and_native_icon(
                id,
                text,
                enabled,
                Some(icon),
            )?
        }

        (
            None,
            Some(
                IconDefinition::Native {
                    icon,
                }
            ),
        ) => {
            Submenu::new_with_native_icon(
                text,
                enabled,
                Some(icon),
            )?
        }

        (
            Some(id),
            None,
        ) => {
            Submenu::with_id(
                id,
                text,
                enabled,
            )?
        }

        (
            None,
            None,
        ) => {
            Submenu::new(
                text,
                enabled,
            )?
        }
    };

    // Rekursiv alle Kinder bauen.
    for child in items {
        let child = build_entry(
            child,
            context,
        )?;

        submenu.append(
            &child
        )?;
    }

    Ok(submenu)
}


// ============================================================================
// BUILD ENTRY
// ============================================================================

pub fn build_entry(
    entry: MenuEntry,
    context: &mut MenuBuildContext,
) -> Result<MenuItemKind, MenuBuildError> {
    match entry {
        // --------------------------------------------------------------------
        // NORMAL ITEM
        // --------------------------------------------------------------------

        MenuEntry::Item {
            id,
            text,
            enabled,
            accelerator,
        } => {
            context.register_event_id(
                &id
            )?;

            let item = MenuItem::with_id(
                id,
                text,
                enabled,
                accelerator,
            )?;

            Ok(
                MenuItemKind::MenuItem(
                    item
                )
            )
        }

        // --------------------------------------------------------------------
        // CHECK ITEM
        // --------------------------------------------------------------------

        MenuEntry::Check {
            id,
            text,
            enabled,
            checked,
            accelerator,
        } => {
            context.register_event_id(
                &id
            )?;

            let item = CheckMenuItem::with_id(
                id,
                text,
                enabled,
                checked,
                accelerator,
            )?;

            Ok(
                MenuItemKind::Check(
                    item
                )
            )
        }

        // --------------------------------------------------------------------
        // ICON ITEM
        // --------------------------------------------------------------------

        MenuEntry::Icon {
            id,
            text,
            enabled,
            accelerator,
            icon,
        } => {
            context.register_event_id(
                &id
            )?;

            let item = build_icon_item(
                id,
                text,
                enabled,
                accelerator,
                icon,
            )?;

            Ok(
                MenuItemKind::Icon(
                    item
                )
            )
        }

        // --------------------------------------------------------------------
        // SEPARATOR
        // --------------------------------------------------------------------

        MenuEntry::Separator => {
            Ok(
                MenuItemKind::Predefined(
                    PredefinedMenuItem::separator()?
                )
            )
        }

        // --------------------------------------------------------------------
        // PREDEFINED
        // --------------------------------------------------------------------

        MenuEntry::Predefined {
            item,
            text,
        } => {
            build_predefined(
                item,
                text,
            )
        }

        // --------------------------------------------------------------------
        // SUBMENU
        // --------------------------------------------------------------------

        MenuEntry::Submenu {
            id,
            text,
            enabled,
            icon,
            items,
        } => {
            let submenu = build_submenu(
                id,
                text,
                enabled,
                icon,
                items,
                context,
            )?;

            Ok(
                MenuItemKind::Submenu(
                    submenu
                )
            )
        }
    }
}


// ============================================================================
// BUILT MENU RESULT
// ============================================================================

pub struct BuiltMenu {
    pub menu: Menu,

    pub event_ids: HashSet<String>,
}


// ============================================================================
// BUILD ROOT MENU
// ============================================================================

pub fn build_menu(
    definition: MenuDefinition,
) -> Result<BuiltMenu, MenuBuildError> {
    let MenuDefinition {
        id,
        items,
    } = definition;

    let mut context =
        MenuBuildContext::new(
            id.clone()
        );

    if let Some(id) = id.as_deref() {
        context.register_id(
            id
        )?;
    }

    let menu = match id {
        Some(id) => {
            Menu::with_id(
                id
            )?
        }

        None => {
            Menu::new()?
        }
    };

    for entry in items {
        let item = build_entry(
            entry,
            &mut context,
        )?;

        menu.append(
            &item
        )?;
    }

    Ok(
        BuiltMenu {
            menu,
            event_ids: context.event_ids,
        }
    )
}


// ============================================================================
// PROCESSED CREATE RESULT
// ============================================================================

pub struct CreatedMenu {
    pub request_id: String,

    pub menu: Menu,

    pub event_ids: HashSet<String>,
}


// ============================================================================
// PROCESS COMMAND
// ============================================================================

pub fn process_menu_command(
    command: MenuCommand,
) -> Result<CreatedMenu, MenuBuildError> {
    match command {
        MenuCommand::CreateMenu {
            version,
            request_id,
            menu,
        } => {
            if version != MENU_PROTOCOL_VERSION {
                return Err(
                    MenuBuildError::
                        UnsupportedProtocolVersion(
                            version
                        )
                );
            }

            let built =
                build_menu(
                    menu
                )?;

            Ok(
                CreatedMenu {
                    request_id,
                    menu: built.menu,
                    event_ids:
                        built.event_ids,
                }
            )
        }
    }
}


// ============================================================================
// RESPONSE
// ============================================================================

#[derive(Debug, Serialize)]
pub struct CreateMenuResponse {
    pub request_id: String,

    pub resource_id: ResourceId,

    pub menu_id: String,
}


// ============================================================================
// FULL JSON -> MENU PROCESS
// ============================================================================

pub fn create_menu_from_json(
    json: &[u8],
    resources: &mut ResourceTable,
) -> Result<CreateMenuResponse, MenuBuildError> {
    // ------------------------------------------------------------------------
    // 1. JSON -> Serde
    // ------------------------------------------------------------------------

    let command: MenuCommand =
        serde_json::from_slice(
            json
        )?;

    // ------------------------------------------------------------------------
    // 2. Serde model -> native menu
    // ------------------------------------------------------------------------

    let CreatedMenu {
        request_id,
        menu,
        event_ids: _event_ids,
    } = process_menu_command(
        command
    )?;

    // ------------------------------------------------------------------------
    // 3. Menu ID sichern bevor Menu verschoben wird
    // ------------------------------------------------------------------------

    let menu_id =
        menu
            .id()
            .0
            .clone();

    // ------------------------------------------------------------------------
    // 4. Native Menu Resource speichern
    // ------------------------------------------------------------------------

    let resource_id =
        resources.add(
            menu
        );

    // ------------------------------------------------------------------------
    // 5. Antwort an Fremdsprache
    // ------------------------------------------------------------------------

    Ok(
        CreateMenuResponse {
            request_id,
            resource_id,
            menu_id,
        }
    )
}


// ============================================================================
// OUTGOING MENU EVENT
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(
    tag = "event",
    rename_all = "snake_case"
)]
pub enum OutgoingEvent {
    Menu {
        item_id: String,
    },
}


// ============================================================================
// EVENT HANDLER
// ============================================================================

pub fn install_menu_event_handler<
    F
>(
    send_to_client: F,
)
where
    F: Fn(
            OutgoingEvent
        )
        + Send
        + Sync
        + 'static,
{
    let send_to_client =
        std::sync::Arc::new(
            send_to_client
        );

    let sender =
        send_to_client.clone();

    muda::MenuEvent::
        set_event_handler(
            Some(
                move |event| {
                    let outgoing =
                        OutgoingEvent::Menu {
                            item_id:
                                event
                                    .id
                                    .0
                                    .clone(),
                        };

                    sender(
                        outgoing
                    );
                }
            )
        );
}


// ============================================================================
// EXAMPLE INPUT JSON
// ============================================================================

pub const EXAMPLE_CREATE_MENU_JSON: &str = r#"
{
    "version": 1,
    "request_id": "req-001",
    "command": "create_menu",

    "menu": {
        "id": "main",

        "items": [
            {
                "type": "submenu",
                "id": "file",
                "text": "File",
                "enabled": true,

                "items": [
                    {
                        "type": "item",
                        "id": "main.file.open",
                        "text": "Open",
                        "enabled": true,
                        "accelerator": "Ctrl+O"
                    },

                    {
                        "type": "item",
                        "id": "main.file.save",
                        "text": "Save",
                        "enabled": true,
                        "accelerator": "Ctrl+S"
                    },

                    {
                        "type": "separator"
                    },

                    {
                        "type": "predefined",
                        "item": "quit"
                    }
                ]
            },

            {
                "type": "submenu",
                "id": "view",
                "text": "View",

                "items": [
                    {
                        "type": "check",
                        "id": "main.view.dark_mode",
                        "text": "Dark Mode",
                        "checked": true
                    }
                ]
            }
        ]
    }
}
"#;


// ============================================================================
// EXAMPLE USE
// ============================================================================

pub fn example(
    resources: &mut ResourceTable,
) -> Result<(), MenuBuildError> {
    let response =
        create_menu_from_json(
            EXAMPLE_CREATE_MENU_JSON
                .as_bytes(),
            resources,
        )?;

    println!(
        "created menu: resource_id={}, menu_id={}",
        response.resource_id,
        response.menu_id,
    );

    Ok(())
}












/* 

Python



from __future__ import annotations

from collections.abc import Callable
from typing import Annotated, Any, Literal

from pydantic import BaseModel, Field, PrivateAttr


Callback = Callable[[], Any]


# ---------------------------------------------------------
# Serializable models
# ---------------------------------------------------------

class MenuItemModel(BaseModel):
    type: Literal["item"] = "item"

    id: str
    text: str
    enabled: bool = True
    accelerator: str | None = None


class CheckMenuItemModel(BaseModel):
    type: Literal["check"] = "check"

    id: str
    text: str
    enabled: bool = True
    checked: bool = False
    accelerator: str | None = None


class SeparatorModel(BaseModel):
    type: Literal["separator"] = "separator"


class PredefinedMenuItemModel(BaseModel):
    type: Literal["predefined"] = "predefined"

    item: Literal[
        "copy",
        "cut",
        "paste",
        "select_all",
        "undo",
        "redo",
        "minimize",
        "maximize",
        "fullscreen",
        "hide",
        "hide_others",
        "show_all",
        "close_window",
        "quit",
        "services",
        "bring_all_to_front",
    ]

    text: str | None = None


class SubmenuModel(BaseModel):
    type: Literal["submenu"] = "submenu"

    id: str | None = None
    text: str
    enabled: bool = True
    items: list["MenuEntry"] = Field(default_factory=list)


MenuEntry = Annotated[
    MenuItemModel
    | CheckMenuItemModel
    | SeparatorModel
    | PredefinedMenuItemModel
    | SubmenuModel,
    Field(discriminator="type"),
]


class MenuDefinition(BaseModel):
    id: str | None = None
    items: list[MenuEntry] = Field(default_factory=list)


class CreateMenuCommand(BaseModel):
    command: Literal["create_menu"] = "create_menu"
    menu: MenuDefinition


# ---------------------------------------------------------
# Python-side ergonomic API
# ---------------------------------------------------------

class Menu:
    def __init__(self, menu_id: str | None = None):
        self.id = menu_id
        self._items: list[MenuEntry] = []
        self._callbacks: dict[str, Callback] = {}

    def item(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> Menu:
        self._items.append(
            MenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._callbacks[id] = on_click

        return self

    def check(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        checked: bool = False,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> Menu:
        self._items.append(
            CheckMenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                checked=checked,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._callbacks[id] = on_click

        return self

    def separator(self) -> Menu:
        self._items.append(
            SeparatorModel()
        )
        return self

    def predefined(
        self,
        item: Literal[
            "copy",
            "cut",
            "paste",
            "select_all",
            "undo",
            "redo",
            "minimize",
            "maximize",
            "fullscreen",
            "hide",
            "hide_others",
            "show_all",
            "close_window",
            "quit",
            "services",
            "bring_all_to_front",
        ],
        *,
        text: str | None = None,
    ) -> Menu:
        self._items.append(
            PredefinedMenuItemModel(
                item=item,
                text=text,
            )
        )
        return self

    def submenu(
        self,
        text: str,
        *,
        id: str | None = None,
        enabled: bool = True,
    ) -> Submenu:
        submenu = Submenu(
            text=text,
            id=id,
            enabled=enabled,
            root=self,
        )

        return submenu

    def to_model(self) -> MenuDefinition:
        return MenuDefinition(
            id=self.id,
            items=self._items,
        )

    def to_command(self) -> CreateMenuCommand:
        return CreateMenuCommand(
            menu=self.to_model()
        )

    def to_dict(self) -> dict[str, Any]:
        return self.to_command().model_dump(
            mode="json",
            exclude_none=True,
        )

    def dispatch(self, item_id: str) -> bool:
        callback = self._callbacks.get(item_id)

        if callback is None:
            return False

        callback()
        return True


class Submenu:
    def __init__(
        self,
        *,
        text: str,
        root: Menu,
        id: str | None = None,
        enabled: bool = True,
    ):
        self._root = root

        self._model = SubmenuModel(
            id=id,
            text=text,
            enabled=enabled,
        )

        root._items.append(self._model)

    def item(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> Submenu:
        self._model.items.append(
            MenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._root._callbacks[id] = on_click

        return self

    def check(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        checked: bool = False,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> Submenu:
        self._model.items.append(
            CheckMenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                checked=checked,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._root._callbacks[id] = on_click

        return self

    def separator(self) -> Submenu:
        self._model.items.append(
            SeparatorModel()
        )
        return self

    def predefined(
        self,
        item: str,
        *,
        text: str | None = None,
    ) -> Submenu:
        self._model.items.append(
            PredefinedMenuItemModel(
                item=item,
                text=text,
            )
        )
        return self

    def submenu(
        self,
        text: str,
        *,
        id: str | None = None,
        enabled: bool = True,
    ) -> NestedSubmenu:
        submenu = NestedSubmenu(
            root=self._root,
            parent=self._model,
            text=text,
            id=id,
            enabled=enabled,
        )

        return submenu


class NestedSubmenu:
    def __init__(
        self,
        *,
        root: Menu,
        parent: SubmenuModel,
        text: str,
        id: str | None = None,
        enabled: bool = True,
    ):
        self._root = root

        self._model = SubmenuModel(
            id=id,
            text=text,
            enabled=enabled,
        )

        parent.items.append(self._model)

    def item(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> NestedSubmenu:
        self._model.items.append(
            MenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._root._callbacks[id] = on_click

        return self

    def check(
        self,
        text: str,
        *,
        id: str,
        enabled: bool = True,
        checked: bool = False,
        accelerator: str | None = None,
        on_click: Callback | None = None,
    ) -> NestedSubmenu:
        self._model.items.append(
            CheckMenuItemModel(
                id=id,
                text=text,
                enabled=enabled,
                checked=checked,
                accelerator=accelerator,
            )
        )

        if on_click is not None:
            self._root._callbacks[id] = on_click

        return self

    def separator(self) -> NestedSubmenu:
        self._model.items.append(
            SeparatorModel()
        )
        return self








Example usage



def open_file():
    print("OPEN")


def save_file():
    print("SAVE")


def toggle_dark_mode():
    print("DARK MODE")


menu = Menu("main")

file_menu = menu.submenu(
    "File",
    id="file",
)

file_menu.item(
    "Open",
    id="file.open",
    accelerator="Ctrl+O",
    on_click=open_file,
)

file_menu.item(
    "Save",
    id="file.save",
    accelerator="Ctrl+S",
    on_click=save_file,
)

file_menu.separator()

file_menu.predefined(
    "quit",
)

view_menu = menu.submenu(
    "View",
    id="view",
)

view_menu.check(
    "Dark Mode",
    id="view.dark_mode",
    checked=True,
    on_click=toggle_dark_mode,
)


payload = menu.to_dict()



{
    "command": "create_menu",
    "menu": {
        "id": "main",
        "items": [
            {
                "type": "submenu",
                "id": "file",
                "text": "File",
                "enabled": True,
                "items": [
                    {
                        "type": "item",
                        "id": "file.open",
                        "text": "Open",
                        "enabled": True,
                        "accelerator": "Ctrl+O",
                    },
                    {
                        "type": "item",
                        "id": "file.save",
                        "text": "Save",
                        "enabled": True,
                        "accelerator": "Ctrl+S",
                    },
                    {
                        "type": "separator",
                    },
                    {
                        "type": "predefined",
                        "item": "quit",
                    },
                ],
            },
            {
                "type": "submenu",
                "id": "view",
                "text": "View",
                "enabled": True,
                "items": [
                    {
                        "type": "check",
                        "id": "view.dark_mode",
                        "text": "Dark Mode",
                        "enabled": True,
                        "checked": True,
                    }
                ],
            },
        ],
    },
}


Rust sendet also beispielsweise:




#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Menu {
        menu_id: Option<String>,
        item_id: String,
    },
}


use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

type MenuRegistry = Arc<Mutex<HashMap<String, String>>>;




{
    "event": "menu",
    "item_id": "file.save",
}

fn register_item(
    registry: &MenuRegistry,
    menu_id: &str,
    item_id: &str,
) {
    registry
        .lock()
        .unwrap()
        .insert(
            item_id.to_string(),
            menu_id.to_string(),
        );
}

let proxy = event_loop.create_proxy();

let registry = registry.clone();

muda::MenuEvent::set_event_handler(Some(move |event| {
    let item_id = event.id.0.clone();

    let menu_id = registry
        .lock()
        .unwrap()
        .get(&item_id)
        .cloned();

    let event = Event::Menu {
        menu_id,
        item_id,
    };

    send_to_python(event);
}));


*/