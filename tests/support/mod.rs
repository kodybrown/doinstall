//! Native test fixtures use physical paths before recording expected ownership.
use std::fs;
#[cfg(windows)]
use std::path::PathBuf;

pub fn temporary() -> tempfile::TempDir {
  // TEMP may be an 8.3 spelling or junction on Windows; macOS TMPDIR commonly
  // traverses /var -> /private/var. Resolve only this explicit system location,
  // keeping links inside the resulting fixture subject to runtime refusal.
  let parent = fs::canonicalize(std::env::temp_dir()).unwrap();
  #[cfg(windows)]
  let parent = {
    let text = parent.to_string_lossy();
    if let Some(unc) = text.strip_prefix("\\\\?\\UNC\\") {
      PathBuf::from(format!("\\\\{unc}"))
    } else if let Some(dos) = text.strip_prefix("\\\\?\\") {
      PathBuf::from(dos)
    } else {
      parent
    }
  };
  tempfile::tempdir_in(parent).unwrap()
}
