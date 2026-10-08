use std::{
  fs::{self, File, OpenOptions},
  io::{self, BufWriter, Write},
  path::{Path, PathBuf},
  sync::{Mutex, OnceLock},
  time::{SystemTime, UNIX_EPOCH},
};

const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;
const MAX_RECORD_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
  Info,
  Warn,
  Error,
}

impl Level {
  const fn name(self) -> &'static str {
    match self {
      Self::Info => "INFO",
      Self::Warn => "WARN",
      Self::Error => "ERROR",
    }
  }
}

struct FileLogger {
  path: PathBuf,
  writer: Option<BufWriter<File>>,
  bytes: u64,
}

impl FileLogger {
  fn open(path: PathBuf) -> io::Result<Self> {
    if let Some(parent) = path.parent() {
      fs::create_dir_all(parent)?;
    }

    let file = OpenOptions::new().create(true).append(true).open(&path)?;

    let bytes = file.metadata()?.len();

    Ok(Self {
      path,
      writer: Some(BufWriter::new(file)),
      bytes,
    })
  }

  fn write_record(&mut self, record: &[u8]) -> io::Result<()> {
    if self.bytes.saturating_add(record.len() as u64) > MAX_LOG_BYTES {
      self.rotate()?;
    }

    let writer = self
      .writer
      .as_mut()
      .ok_or_else(|| io::Error::other("log writer is unavailable"))?;

    writer.write_all(record)?;
    writer.flush()?;

    self.bytes = self.bytes.saturating_add(record.len() as u64);

    Ok(())
  }

  fn rotate(&mut self) -> io::Result<()> {
    if let Some(mut writer) = self.writer.take() {
      writer.flush()?;
      drop(writer);
    }

    let backup = self.path.with_extension("log.1");

    match fs::remove_file(&backup) {
      Ok(()) => {}
      Err(error) if error.kind() == io::ErrorKind::NotFound => {}
      Err(error) => return Err(error),
    }

    match fs::rename(&self.path, &backup) {
      Ok(()) => {}
      Err(error) if error.kind() == io::ErrorKind::NotFound => {}
      Err(error) => return Err(error),
    }

    let file = OpenOptions::new()
      .create(true)
      .append(true)
      .open(&self.path)?;

    self.writer = Some(BufWriter::new(file));
    self.bytes = 0;

    Ok(())
  }
}

fn logger() -> &'static Mutex<Option<FileLogger>> {
  static LOGGER: OnceLock<Mutex<Option<FileLogger>>> = OnceLock::new();

  LOGGER.get_or_init(|| Mutex::new(None))
}

/// Initializes the process logger.
///
/// Logging is written to `data_dir/product_name`.
/// Initialization errors are propagated to the caller.
pub fn init(data_dir: &Path, product_name: &str) -> io::Result<PathBuf> {
  let path = data_dir.join(product_name);
  let file_logger = FileLogger::open(path.clone())?;

  let mut current = logger()
    .lock()
    .map_err(|_| io::Error::other("log state lock poisoned"))?;

  *current = Some(file_logger);

  Ok(path)
}

/// Internal logging entry point used by Taurino's exported macros.
///
/// This function is public because exported macros may expand inside
/// downstream crates. It is not intended as part of the user-facing API.
#[doc(hidden)]
pub fn __write(level: Level, args: std::fmt::Arguments<'_>) {
  let mut message = args.to_string();

  if message.len() > MAX_RECORD_BYTES {
    let mut end = MAX_RECORD_BYTES;

    while !message.is_char_boundary(end) {
      end -= 1;
    }

    message.truncate(end);
    message.push_str(" [truncated]");
  }

  let timestamp = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|duration| duration.as_millis())
    .unwrap_or_default();

  let record = format!("{timestamp} {} {message}\n", level.name());

  // Logging failures are intentionally silent.
  //
  // Framework diagnostics must never leak into
  // stdin/stdout/stderr or interfere with application execution.
  if let Ok(mut current) = logger().lock()
    && let Some(logger) = current.as_mut()
  {
    let _ = logger.write_record(record.as_bytes());
  }
}

// Optional module-level macro re-exports.
//
// This allows:
//
//   use taurino_core::tools::logging;
//   logging::taurino_log!(...);
//
// in addition to:
//
//   taurino_core::taurino_log!(...);
pub use crate::{taurino_error, taurino_info, taurino_log, taurino_warn};

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn log_file_rotates_at_the_configured_bound() {
    let root = std::env::temp_dir().join(format!(
      "taurino-log-{}-{}",
      std::process::id(),
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
    ));

    let path = root.join("taurino.log");

    let mut file_logger = FileLogger::open(path.clone()).unwrap();

    file_logger
      .write_record(&vec![b'a'; MAX_LOG_BYTES as usize])
      .unwrap();

    file_logger.write_record(b"next\n").unwrap();

    let current = fs::read(&path).unwrap();
    let backup = fs::read(path.with_extension("log.1")).unwrap();

    assert_eq!(current, b"next\n");
    assert_eq!(backup.len(), MAX_LOG_BYTES as usize);

    drop(file_logger);

    let _ = fs::remove_dir_all(root);
  }
}

#[macro_export]
macro_rules! taurino_log {
  ($level:expr, $($argument:tt)*) => {{
    $crate::tools::logging::__write(
      $level,
      format_args!($($argument)*),
    );
  }};
}

#[macro_export]
macro_rules! taurino_info {
  ($($argument:tt)*) => {{
    $crate::taurino_log!(
      $crate::tools::logging::Level::Info,
      $($argument)*
    );
  }};
}

#[macro_export]
macro_rules! taurino_warn {
  ($($argument:tt)*) => {{
    $crate::taurino_log!(
      $crate::tools::logging::Level::Warn,
      $($argument)*
    );
  }};
}

#[macro_export]
macro_rules! taurino_error {
  ($($argument:tt)*) => {{
    $crate::taurino_log!(
      $crate::tools::logging::Level::Error,
      $($argument)*
    );
  }};
}
