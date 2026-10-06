use std::ffi::OsString;

#[allow(dead_code)]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Env {
  #[cfg(target_os = "linux")]
  pub appimage: Option<std::ffi::OsString>,
  #[cfg(target_os = "linux")]
  pub appdir: Option<std::ffi::OsString>,
  pub args_os: Vec<OsString>,
}

#[allow(clippy::derivable_impls)]
impl Default for Env {
  fn default() -> Self {
    let args_os = std::env::args_os().collect();
    #[cfg(target_os = "linux")]
    {
      let env = Self {
        #[cfg(target_os = "linux")]
        appimage: std::env::var_os("APPIMAGE"),
        #[cfg(target_os = "linux")]
        appdir: std::env::var_os("APPDIR"),
        args_os,
      };
      if env.appimage.is_some() || env.appdir.is_some() {
        let is_temp = std::env::current_exe()
          .map(|p| {
            p.display()
              .to_string()
              .starts_with(&format!("{}/.mount_", std::env::temp_dir().display()))
          })
          .unwrap_or(true);

        if !is_temp {
          log::warn!(
            "`APPDIR` or `APPIMAGE` environment variable found but this application was not detected as an AppImage; this might be a security issue."
          );
        }
      }
      env
    }
    #[cfg(not(target_os = "linux"))]
    {
      Self { args_os }
    }
  }
}
