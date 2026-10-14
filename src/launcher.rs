//! Precompiled console template. Application packaging appends data, never code.
#![allow(dead_code)]
mod binary;
mod files;
mod model;
mod self_extract;
use anyhow::{ensure, Context, Result};
use std::{ffi::OsString, fs, path::PathBuf, process::Command};

// A role record exists only in the launcher binary. The builder compares its
// components separately, so its own search string cannot identify it as a stub.
#[repr(C)]
struct TemplateIdentity {
  prefix: [u8; 32],
  role_a: u64,
  role_b: u64,
}
static TEMPLATE_IDENTITY: TemplateIdentity = TemplateIdentity {
  prefix: *self_extract::TEMPLATE_MARKER,
  role_a: self_extract::TEMPLATE_ROLE_A,
  role_b: self_extract::TEMPLATE_ROLE_B,
};

fn main() {
  // Retain the template identifier in the release binary even with LTO/stripping.
  std::hint::black_box(&TEMPLATE_IDENTITY);
  match run() {
    Ok(code) => std::process::exit(code),
    Err(error) => {
      eprintln!("doinstall launcher: {error:#}");
      std::process::exit(1);
    }
  }
}
fn run() -> Result<i32> {
  let arguments: Vec<_> = std::env::args_os().skip(1).collect();
  if arguments == ["--launcher-help"] {
    println!("Self-extracting doinstall launcher\n  --extract-to PATH  Retain installer sources in a new or empty directory\n  --launcher-help    Show launcher help without extracting\n  --                 Forward all following arguments to the installer\nDefault: a fresh retained directory under the OS temporary root.\nInstaller input/output uses this console directly; its exit status is preserved.");
    return Ok(0);
  }
  let (extract_to, arguments) = options(arguments.into_iter())?;
  let executable = std::env::current_exe()?;
  let bundle = self_extract::Bundle::open(&executable)?;
  let platform = if cfg!(windows) {
    "windows"
  } else if cfg!(target_os = "macos") {
    "macos"
  } else {
    "linux"
  };
  let architecture = if cfg!(target_arch = "aarch64") {
    "arm64"
  } else {
    "x64"
  };
  ensure!(
    bundle.manifest.platform == platform && bundle.manifest.architecture == architecture,
    "Launcher target mismatch"
  );
  let installer_name = bundle.manifest.installer.clone();
  let directory = match extract_to {
    Some(path) => {
      // Check the original spelling before canonicalizing: --extract-to must
      // not silently follow a user-supplied symlink/junction into another tree.
      let path = if path.is_absolute() {
        path
      } else {
        std::env::current_dir()?.join(path)
      };
      files::safe_path(&path)?;
      if path.exists() {
        ensure!(
          path.is_dir() && fs::read_dir(&path)?.next().is_none(),
          "--extract-to requires a new or empty directory"
        );
      } else {
        fs::create_dir_all(&path)?;
      }
      fs::canonicalize(path)?
    }
    None => {
      let name = executable
        .file_name()
        .and_then(|s| s.to_str())
        .context("Launcher filename is not UTF-8")?;
      let stem = files::executable_stem(name);
      let root = fs::canonicalize(std::env::temp_dir())?;
      // Retention is intentional, including on failure. Never leave cleanup to
      // a TempDir destructor or try to sweep directories from previous runs.
      tempfile::Builder::new().prefix(&format!("{stem}-")).tempdir_in(root)?.keep()
    }
  };
  eprintln!("Installer source: {}", directory.display());
  bundle
    .extract(&directory)
    .with_context(|| format!("Extraction incomplete; files retained at {}", directory.display()))?;
  let installer = directory.join(installer_name);
  binary::validate(&installer, platform, architecture)?;
  let mut command = Command::new(&installer);
  // Inherit the caller's cwd/environment and standard handles. Neither platform
  // captures, relays, or translates installer input/output.
  command.args(arguments);
  #[cfg(unix)]
  {
    use std::os::unix::process::CommandExt;
    Err(command.exec()).context("Cannot start extracted installer")
  }
  #[cfg(windows)]
  {
    // Console Ctrl+C is delivered to both processes. While waiting, suppress
    // only the launcher's default exit so the installer can cancel and restore
    // its terminal before we return its status. Do not forward a second signal.
    ctrlc::set_handler(|| {}).context("Cannot install launcher console handler")?;
    let mut child = command.spawn().context("Cannot start extracted installer")?;
    let status = child.wait().context("Cannot wait for extracted installer")?;
    Ok(status.code().unwrap_or(1))
  }
}
fn options(args: impl Iterator<Item = OsString>) -> Result<(Option<PathBuf>, Vec<OsString>)> {
  let mut args = args.peekable();
  let mut path = None;
  let mut installer = Vec::new();
  while let Some(argument) = args.next() {
    if argument == "--" {
      installer.extend(args);
      break;
    }
    if argument == "--extract-to" {
      ensure!(path.is_none(), "Duplicate --extract-to");
      let value = args.next().context("--extract-to requires a path")?;
      ensure!(
        !value.is_empty() && !value.to_string_lossy().starts_with("--"),
        "--extract-to requires a path"
      );
      path = Some(PathBuf::from(value));
    } else {
      installer.push(argument);
    }
  }
  Ok((path, installer))
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn extractor_arguments_do_not_change_installer_values() {
    let (path, args) = options(
      [
        "--extract-to",
        "space 日本語",
        "--",
        "--set",
        "value=--extract-to",
      ]
      .into_iter()
      .map(Into::into),
    )
    .unwrap();
    assert_eq!(path.unwrap(), PathBuf::from("space 日本語"));
    assert_eq!(args, ["--set", "value=--extract-to"]);
    for arguments in [
      vec!["--extract-to"],
      vec!["--extract-to", "--validate"],
      vec!["--extract-to", "a", "--extract-to", "b"],
    ] {
      assert!(options(arguments.into_iter().map(Into::into)).is_err());
    }
  }
}
