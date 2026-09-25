// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Image types used by this crate and also referenced by the JavaScript API layer.

use std::{borrow::Cow, path::Path, sync::Arc};

use anyhow::{Context, Result};

use crate::resources::{Resource, ResourceId, ResourceTable};

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
    /// Creates an owned RGBA image.
    pub const fn new_owned(rgba: Vec<u8>, width: u32, height: u32) -> Self {
        Self {
            rgba: Cow::Owned(rgba),
            width,
            height,
        }
    }
}

impl<'a> Image<'a> {
    /// Creates a borrowed RGBA image.
    pub const fn new(rgba: &'a [u8], width: u32, height: u32) -> Self {
        Self {
            rgba: Cow::Borrowed(rgba),
            width,
            height,
        }
    }

    /// Creates an image from encoded image bytes.
    ///
    /// The actual formats supported depend on the formats enabled for
    /// the `image` crate itself.
    pub fn from_bytes(bytes: &[u8]) -> Result<Image<'static>> {
        let img = image::load_from_memory(bytes).context("failed to decode image from bytes")?;

        let width = img.width();
        let height = img.height();

        Ok(Image {
            rgba: Cow::Owned(img.into_rgba8().into_raw()),
            width,
            height,
        })
    }

    /// Loads and decodes an image from a filesystem path.
    pub fn from_path<P>(path: P) -> Result<Image<'static>>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();

        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read image from {}", path.display()))?;

        Self::from_bytes(&bytes)
            .with_context(|| format!("failed to decode image from {}", path.display()))
    }

    /// Returns the RGBA data in row-major order from top to bottom.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// Returns the image width.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Returns the image height.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Converts this image into an owned `'static` image.
    pub fn to_owned(self) -> Image<'static> {
        Image {
            rgba: match self.rgba {
                Cow::Owned(data) => Cow::Owned(data),
                Cow::Borrowed(data) => Cow::Owned(data.to_vec()),
            },
            width: self.width,
            height: self.height,
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

/// An image type accepting:
///
/// - filesystem paths
/// - encoded PNG/ICO/etc. bytes
/// - ResourceTable IDs
/// - raw RGBA pixels
///
/// No compile-time feature switching is used here.
#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum JsImage {
    /// Path to an encoded image.
    #[non_exhaustive]
    Path(std::path::PathBuf),

    /// Encoded image data.
    #[non_exhaustive]
    Bytes(Vec<u8>),

    /// Previously loaded image stored in a ResourceTable.
    #[non_exhaustive]
    Resource(ResourceId),

    /// Raw RGBA image.
    #[non_exhaustive]
    Rgba {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
    },
}

impl JsImage {
    /// Converts any supported representation into an Image.
    pub fn into_img(self, resources_table: &ResourceTable) -> Result<Arc<Image<'static>>> {
        match self {
            Self::Resource(rid) => resources_table.get::<Image<'static>>(rid),

            Self::Path(path) => Image::from_path(path).map(Arc::new),

            Self::Bytes(bytes) => Image::from_bytes(&bytes).map(Arc::new),

            Self::Rgba {
                rgba,
                width,
                height,
            } => Ok(Arc::new(Image::new_owned(rgba, width, height))),
        }
    }
}

/// Window icon.
#[derive(Debug, Clone)]
pub struct Icon<'a> {
    /// RGBA bytes of the icon.
    pub rgba: Cow<'a, [u8]>,
    /// Icon width.
    pub width: u32,
    /// Icon height.
    pub height: u32,
}

impl<'a> Image<'a> {
    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba.into_owned()
    }
}

impl TryFrom<Image<'_>> for tao::window::Icon {
    type Error = tao::window::BadIcon;

    fn try_from(image: Image<'_>) -> Result<Self, Self::Error> {
        let width = image.width();
        let height = image.height();
        let rgba = image.into_rgba();

        tao::window::Icon::from_rgba(rgba, width, height)
    }
}

impl TryFrom<Image<'_>> for muda::Icon {
    type Error = muda::BadIcon;

    fn try_from(image: Image<'_>) -> Result<Self, Self::Error> {
        let width = image.width();
        let height = image.height();
        let rgba = image.into_rgba();

        muda::Icon::from_rgba(rgba, width, height)
    }
}

impl TryFrom<Image<'_>> for tray_icon::Icon {
    type Error = tray_icon::BadIcon;

    fn try_from(image: Image<'_>) -> Result<Self, Self::Error> {
        let width = image.width();
        let height = image.height();
        let rgba = image.into_rgba();

        tray_icon::Icon::from_rgba(rgba, width, height)
    }
}
