//! Schema definitions for the Taurino engine.
//!
//! This crate provides the serializable types that describe the engine's
//! configuration, build metadata, and runtime permissions.

pub use ::dpi::*;
pub mod event;
pub mod menu;
pub mod package;
pub mod trayicon;
pub mod webview;
pub mod window;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{
  fmt::{self, Display, Formatter},
  path::PathBuf,
  str::FromStr,
};
use url::Url;

use crate::schema::window::WindowConfig;
/// System theme.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Theme {
  /// Light theme.
  Light,
  /// Dark theme.
  Dark,
}

impl Serialize for Theme {
  fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
  where
    S: Serializer,
  {
    serializer.serialize_str(self.to_string().as_ref())
  }
}

impl<'de> Deserialize<'de> for Theme {
  fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let s = String::deserialize(deserializer)?;
    Ok(match s.to_lowercase().as_str() {
      "dark" => Self::Dark,
      _ => Self::Light,
    })
  }
}

impl Display for Theme {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "{}",
      match self {
        Self::Light => "light",
        Self::Dark => "dark",
      }
    )
  }
}
/// A rectangular region.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Rect {
  /// Rect position.
  pub position: dpi::Position,
  /// Rect size.
  pub size: dpi::Size,
}

impl Default for Rect {
  fn default() -> Self {
    Self {
      position: Position::Logical((0, 0).into()),
      size: Size::Logical((0, 0).into()),
    }
  }
}

/// A rectangular region in physical pixels.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PhysicalRect<P: dpi::Pixel, S: dpi::Pixel> {
  /// Rect position.
  pub position: dpi::PhysicalPosition<P>,
  /// Rect size.
  pub size: dpi::PhysicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for PhysicalRect<P, S> {
  fn default() -> Self {
    Self {
      position: (0, 0).into(),
      size: (0, 0).into(),
    }
  }
}

/// A rectangular region in logical pixels.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LogicalRect<P: dpi::Pixel, S: dpi::Pixel> {
  /// Rect position.
  pub position: dpi::LogicalPosition<P>,
  /// Rect size.
  pub size: dpi::LogicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for LogicalRect<P, S> {
  fn default() -> Self {
    Self {
      position: (0, 0).into(),
      size: (0, 0).into(),
    }
  }
}

// ============================================================================
// FrontendDist
// ============================================================================
//
// Enum describing how the frontend is delivered. Three variants:
//
//   Url(Url)          -> load the UI from a URL, no local files embedded
//   Directory(PathBuf)-> a directory of frontend files, embedded recursively,
//                        with `index.html` as the entry point
//   Files(Vec<PathBuf>)-> an explicit list of individual files to embed
//
// Serde attributes:
//
//   `untagged`
//     The config value is NOT tagged with the variant name. Serde tries each
//     variant in order and picks the first that matches. Examples:
//         "https://example.com"       -> Url
//         "./dist"                    -> Directory
//         ["./a.html", "./b.js"]      -> Files
//
//   `deny_unknown_fields`
//     Unknown keys cause an error. Mainly relevant for struct-like variants;
//     for these newtype variants it has limited impact.
//
// Derives:
//
//   Debug, PartialEq, Eq, Clone
//     Standard utility traits. `Eq` is valid here because `Url`, `PathBuf`
//     and `Vec<PathBuf>` all implement `Eq`.
//
//   Deserialize AND Serialize
//     Unlike the config structs above, this type is also serializable,
//     because it may be written back out (e.g. by tooling or for display).
//
//   #[non_exhaustive]
//     Outside this crate, any `match` on this enum must include a wildcard
//     arm. This lets us add new variants later without a breaking change.
//
/// Describes where the user interface is loaded from.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
#[non_exhaustive]
pub enum FrontendDist {
  /// URL of the user interface; no local files are embedded for it.
  Url(Url),

  /// Directory containing the frontend files to be served.
  Directory(PathBuf),

  /// Individual files that should be embedded into the application.
  Files(Vec<PathBuf>),
}

// ============================================================================
// impl Display for FrontendDist
// ============================================================================
//
// Provides a human-readable (and for `Files`, JSON) representation.
//
//   Url        -> prints the URL as-is.
//   Directory  -> prints the path via `Path::display()` (lossy, readable).
//   Files      -> serializes the path list to JSON and prints that.
//
// Notes / potential issues to review:
//
//   1. The `Files` arm uses `.unwrap()` on `serde_json::to_string(paths)`.
//      Serializing `Vec<PathBuf>` should not fail in practice, but
//      `unwrap()` inside a `Display` impl can panic during formatting.
//      Safer: fall back to `Debug` formatting or return `fmt::Error`.
//
//   2. `Display` for `Url` and `Directory` is human-oriented, while `Files`
//      produces JSON. That inconsistency may be intentional (for logging)
//      or may be worth reconsidering.
//
//   3. Because `FrontendDist` is both `Serialize` and `Display`, there are
//      two string representations. Make sure callers use the intended one.
//
impl Display for FrontendDist {
  fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::Url(address) => write!(formatter, "{address}"),
      Self::Directory(directory) => {
        write!(formatter, "{}", directory.display())
      }
      Self::Files(paths) => {
        write!(formatter, "{}", serde_json::to_string(paths).unwrap())
      }
    }
  }
}
/// A tuple struct of RGBA colors. Each value has minimum of 0 and maximum of 255.
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<Color> for (u8, u8, u8, u8) {
  fn from(value: Color) -> Self {
    (value.0, value.1, value.2, value.3)
  }
}

impl From<Color> for (u8, u8, u8) {
  fn from(value: Color) -> Self {
    (value.0, value.1, value.2)
  }
}

impl From<(u8, u8, u8, u8)> for Color {
  fn from(value: (u8, u8, u8, u8)) -> Self {
    Color(value.0, value.1, value.2, value.3)
  }
}

impl From<(u8, u8, u8)> for Color {
  fn from(value: (u8, u8, u8)) -> Self {
    Color(value.0, value.1, value.2, 255)
  }
}

impl From<Color> for [u8; 4] {
  fn from(value: Color) -> Self {
    [value.0, value.1, value.2, value.3]
  }
}

impl From<Color> for [u8; 3] {
  fn from(value: Color) -> Self {
    [value.0, value.1, value.2]
  }
}

impl From<[u8; 4]> for Color {
  fn from(value: [u8; 4]) -> Self {
    Color(value[0], value[1], value[2], value[3])
  }
}

impl From<[u8; 3]> for Color {
  fn from(value: [u8; 3]) -> Self {
    Color(value[0], value[1], value[2], 255)
  }
}

impl FromStr for Color {
  type Err = String;
  fn from_str(mut color: &str) -> Result<Self, Self::Err> {
    color = color.trim().strip_prefix('#').unwrap_or(color);
    let color = match color.len() {
      3 => color
        .chars()
        .flat_map(|c| std::iter::repeat_n(c, 2))
        .chain(std::iter::repeat_n('f', 2))
        .collect(),
      6 => format!("{color}FF"),
      8 => color.to_string(),
      _ => {
        return Err(
          "Invalid hex color length, must be either 3, 6 or 8, for example: #fff, #ffffff, or #ffffffff".into(),
        );
      }
    };

    let r = u8::from_str_radix(&color[0..2], 16).map_err(|e| e.to_string())?;
    let g = u8::from_str_radix(&color[2..4], 16).map_err(|e| e.to_string())?;
    let b = u8::from_str_radix(&color[4..6], 16).map_err(|e| e.to_string())?;
    let a = u8::from_str_radix(&color[6..8], 16).map_err(|e| e.to_string())?;

    Ok(Color(r, g, b, a))
  }
}

fn default_alpha() -> u8 {
  255
}

#[derive(Deserialize)]
#[serde(untagged)]
enum InnerColor {
  /// Color hex string, for example: #fff, #ffffff, or #ffffffff.
  String(String),
  /// Array of RGB colors. Each value has minimum of 0 and maximum of 255.
  Rgb((u8, u8, u8)),
  /// Array of RGBA colors. Each value has minimum of 0 and maximum of 255.
  Rgba((u8, u8, u8, u8)),
  /// Object of red, green, blue, alpha color values. Each value has minimum of 0 and maximum of 255.
  RgbaObject {
    red: u8,
    green: u8,
    blue: u8,
    #[serde(default = "default_alpha")]
    alpha: u8,
  },
}

impl<'de> Deserialize<'de> for Color {
  fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
  where
    D: Deserializer<'de>,
  {
    let color = InnerColor::deserialize(deserializer)?;
    let color = match color {
      InnerColor::String(string) => string.parse().map_err(serde::de::Error::custom)?,
      InnerColor::Rgb(rgb) => Color(rgb.0, rgb.1, rgb.2, 255),
      InnerColor::Rgba(rgb) => rgb.into(),
      InnerColor::RgbaObject {
        red,
        green,
        blue,
        alpha,
      } => Color(red, green, blue, alpha),
    };

    Ok(color)
  }
}

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
pub enum FocusState {
  WindowFocused,
  WebviewFocused {
    webview_label: String,
  },
  Blured {
    last_focused_webview_label: Option<String>,
  },
}

#[cfg(windows)]
impl Default for FocusState {
  fn default() -> Self {
    Self::Blured {
      last_focused_webview_label: None,
    }
  }
}

/// Re-exports of the [`serde`] crate for consumers of this engine.
///
// ============================================================================
// Imports
// ============================================================================
//
// std::fmt::{self, Display, Formatter}
//   - `Display` and `Formatter` are needed to implement the Display trait
//     for `FrontendDist`.
//   - `self` imports the `fmt` module itself so we can refer to `fmt::Result`.
//
// std::path::PathBuf
//   - Owned, heap-allocated filesystem path.
//   - Used for the `Directory` and `Files` variants of `FrontendDist`.
//
// serde::{Deserialize, Serialize}
//   - Derive macros to read from / write to config formats (JSON, TOML, ...).
//
// url::Url
//   - Parsed URL type, used by `dev_url` and `FrontendDist::Url`.
//
// crate::window::WindowConfig
//   - Per-window configuration, defined elsewhere in this crate.
//

// ============================================================================
// ConnecionConfig
// ============================================================================
//
// Small struct holding the IPC endpoint path.
//
// - Only `Deserialize` is derived: this is meant to be read from a config
//   file, not written back out.
// - NOTE: the name is misspelled (`ConnecionConfig` instead of
//   `ConnectionConfig`). This is a real identifier and must either be
//   renamed consistently everywhere or kept as-is.
//
/// Settings for inter-process communication.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnecionConfig {
  /// Path of the IPC endpoint.
  pub path: String,
}

// ============================================================================
// AppConfig
// ============================================================================
//
// Top-level application configuration. All fields are read from the config
// file via serde.
//
// Field-by-field notes:
//
// product_name:
//   - Optional.
//   - `#[serde(alias = "product-name")]` allows either `product_name` or
//     `product-name` as the key in the config file.
//   - Human-readable name used for system display. May also influence
//     package names, install paths, and metadata.
//
// version:
//   - Optional.
//   - Uses a custom deserializer `crate::package::version_deserializer`.
//   - Accepts either a SemVer string OR a path to a `package.json`, from
//     which the `version` field is read.
//   - `default` means the field may be missing entirely.
//
// connection:
//   - Required (not Option, no default).
//   - Holds IPC connection settings (see ConnecionConfig above).
//
// windows:
//   - Required list of window definitions.
//   - Each entry is a WindowConfig describing one application window.
//
// dev_url:
//   - Optional.
//   - `#[serde(alias = "dev-url")]` allows `dev_url` or `dev-url`.
//   - URL of the frontend dev server during development.
//
// frontend_dist:
//   - Optional.
//   - `#[serde(alias = "frontend-dist")]` allows `frontend_dist` or
//     `frontend-dist`.
//   - Describes where the frontend content comes from (URL, directory, or
//     explicit file list). See FrontendDist below.
//   - Relative directory paths are resolved relative to the config file.
//
// identifier:
//   - Required.
//   - Reverse-DNS application identifier, e.g. `com.example.desktop`.
//   - Used to associate app-specific system data and paths.
//   - Intended characters: ASCII letters, digits, hyphens, and dots.
//   - NOTE: the doc says "intended", but the type does NOT enforce this.
//     Validation must happen elsewhere.
//
/// General configuration of the application.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
  /// Visible name of the application.
  ///
  /// This name is used for display in the system and can
  /// also influence package names, install paths, and metadata.
  #[serde(alias = "product-name")]
  pub product_name: Option<String>,

  /// Version of the application.
  ///
  /// Supports a version number in SemVer format or a path to
  /// a `package.json` whose `version` entry should be read.
  /// Processing is handled by the project-specific version deserializer.
  #[serde(
    deserialize_with = "crate::schema::package::version_deserializer",
    default
  )]
  pub version: Option<String>,

  /// Configuration of the IPC connection.
  pub connection: ConnecionConfig,

  /// Settings of the application windows.
  pub windows: Vec<WindowConfig>,

  /// Address of the user interface during development.
  ///
  /// Usually this URL points to a development server
  /// that serves the frontend files and delivers changes directly.
  #[serde(alias = "dev-url")]
  pub dev_url: Option<Url>,

  /// Source of the frontend content.
  ///
  /// Possible values are a URL, a local directory, or individual file paths.
  /// A relative directory path is resolved starting from the configuration
  /// file.
  ///
  /// For a directory, its contents are embedded recursively;
  /// `index.html` serves as the entry point. A file list allows
  /// targeted selection of the resources to embed.
  ///
  /// If a URL is specified, the application loads its content from that
  /// address instead of embedding local frontend files for it.
  #[serde(alias = "frontend-dist")]
  pub frontend_dist: Option<FrontendDist>,

  /// Unique application identifier in reverse domain notation,
  /// for example `com.example.desktop`.
  ///
  /// It is used to associate application-specific system data and paths.
  /// Intended are ASCII letters, digits, hyphens, and dots.
  pub identifier: String,
}
