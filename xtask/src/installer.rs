//! Doinstall packages its own ordinary tool payload using its public builder.
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

pub fn executable(name: &str) -> String {
  format!("{name}{}", if cfg!(windows) { ".exe" } else { "" })
}
pub fn architecture() -> Result<&'static str> {
  match std::env::consts::ARCH {
    "x86_64" => Ok("x64"),
    "aarch64" => Ok("arm64"),
    other => anyhow::bail!("Unsupported package host architecture: {other}"),
  }
}

pub fn assemble(tools: &Path, output_parent: &Path, version: &str, distribution: Option<&Path>) -> Result<PathBuf> {
  let temporary = tempfile::Builder::new().prefix("doinstall-package-").tempdir()?;
  let stage = fs::canonicalize(temporary.path())?;
  let payload = stage.join("payload");
  fs::create_dir(&payload)?;
  // Copy one complete build snapshot. The template is payload data rather than
  // a runnable command; it must remain beside the real installed builder.
  for name in [
    "doinstall",
    "doinstall-builder",
    "doinstall-launcher",
    "doinstall-server",
  ] {
    let name = executable(name);
    fs::copy(tools.join(&name), payload.join(&name))
      .with_context(|| format!("Missing release tool: {}", tools.join(name).display()))?;
  }
  fs::write(payload.join("LICENSE.md"), include_bytes!("../../LICENSE.md"))?;
  if let Some(distribution) = distribution {
    // The release driver supplies notices and metadata, never a directory of
    // arbitrary build output. Require its tool snapshot to match this build.
    for name in [
      "doinstall",
      "doinstall-builder",
      "doinstall-launcher",
      "doinstall-server",
    ] {
      let name = executable(name);
      ensure!(
        fs::read(distribution.join(&name))? == fs::read(payload.join(&name))?,
        "Distribution tool mismatch"
      );
    }
    for name in ["README.md", "THIRD-PARTY.json", "RELEASE.json"] {
      fs::copy(distribution.join(name), payload.join(name))?;
    }
    copy_notices(&distribution.join("LICENSES"), &payload.join("LICENSES"))?;
  }
  let mut config: Value = serde_saphyr::from_str(include_str!("../../packaging/doinstall.yaml"))?;
  config["application"]["version"] = version.into();
  config["platform"] = std::env::consts::OS.into();
  config["architecture"] = architecture()?.into();
  config["payload"] = payload.to_string_lossy().into_owned().into();
  for command in config["runtime"]["commands"].as_array_mut().context("Missing tool commands")? {
    command["executable"] = executable(command["executable"].as_str().context("Missing tool executable")?).into();
  }
  let source = stage.join("installer.yaml");
  let report = stage.join("result.json");
  fs::write(&source, serde_saphyr::to_string(&config)?)?;
  let output_parent = std::path::absolute(output_parent)?;
  let status = Command::new(tools.join(executable("doinstall-builder")))
    .arg("--installer")
    .arg(payload.join(executable("doinstall")))
    .arg("--launcher-template")
    .arg(payload.join(executable("doinstall-launcher")))
    .arg("--config")
    .arg(&source)
    .arg("--output-parent")
    .arg(&output_parent)
    .arg("--self-extracting")
    .arg("--result-file")
    .arg(&report)
    .status()
    .context("Cannot start the doinstall builder")?;
  ensure!(status.success(), "Doinstall package assembly failed: {status}");
  let result: Value = serde_json::from_slice(&fs::read(report)?)?;
  let artifact = PathBuf::from(result["FilePath"].as_str().context("Builder did not return FilePath")?);
  ensure!(
    artifact.is_absolute() && artifact.is_file(),
    "Builder returned an invalid artifact path"
  );
  Ok(artifact)
}

fn copy_notices(source: &Path, destination: &Path) -> Result<()> {
  ensure!(
    !fs::symlink_metadata(source)?.file_type().is_symlink(),
    "Notice links are not allowed"
  );
  fs::create_dir(destination)?;
  for entry in fs::read_dir(source)? {
    let entry = entry?;
    let kind = entry.file_type()?;
    ensure!(!kind.is_symlink(), "Notice links are not allowed");
    if kind.is_dir() {
      copy_notices(&entry.path(), &destination.join(entry.file_name()))?;
    } else {
      ensure!(kind.is_file(), "Not a regular notice file");
      fs::copy(entry.path(), destination.join(entry.file_name()))?;
    }
  }
  Ok(())
}
