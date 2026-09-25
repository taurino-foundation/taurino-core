use std::{fmt, path::PathBuf};

use serde::{Deserialize, Deserializer, Serialize};
use url::Url;




/// Defines the URL or assets to embed in the application.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
#[non_exhaustive]
pub enum FrontendDist {
    Url(Url),
    Directory(PathBuf),
}

impl std::fmt::Display for FrontendDist {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Url(url) => write!(f, "{url}"),
            Self::Directory(p) => write!(f, "{}", p.display()),
        }
    }
}




#[derive(PartialEq, Eq, Debug, Clone, Serialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum WebviewUrl {
    External(Url),
    App(PathBuf),
    CustomProtocol(Url),
}

impl<'de> Deserialize<'de> for WebviewUrl {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WebviewUrlDeserializer {
            Url(Url),
            Path(PathBuf),
        }

        match WebviewUrlDeserializer::deserialize(deserializer)? {
            WebviewUrlDeserializer::Url(u) => {
                if u.scheme() == "https" || u.scheme() == "http" {
                    Ok(Self::External(u))
                } else {
                    Ok(Self::CustomProtocol(u))
                }
            }
            WebviewUrlDeserializer::Path(p) => Ok(Self::App(p)),
        }
    }
}

impl fmt::Display for WebviewUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::External(url) | Self::CustomProtocol(url) => {
                write!(f, "{url}")
            }
            Self::App(path) => write!(f, "{}", path.display()),
        }
    }
}

impl Default for WebviewUrl {
    fn default() -> Self {
        Self::App("index.html".into())
    }
}
