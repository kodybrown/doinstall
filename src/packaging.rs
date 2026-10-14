//! Producer-only settings are consumed by the builder, never migrated into runtime YAML.
use crate::{files, model::Package};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Settings {
  pub installer_name: Option<String>,
  pub launcher_name: Option<String>,
}
pub struct Names {
  pub installer: String,
  pub yaml: String,
  pub launcher: String,
}
pub fn read(path: &Path) -> Result<(Package, Settings)> {
  let mut source: serde_json::Value = files::read_yaml(path)?;
  let mapping = source.as_object_mut().context("Installer YAML must be a mapping")?;
  let settings = match mapping.remove("packaging") {
    Some(value) => serde_json::from_value(value).context("Invalid packaging settings")?,
    None => Settings::default(),
  };
  Ok((
    serde_json::from_value(source).context("Invalid installer configuration")?,
    settings,
  ))
}
impl Settings {
  pub fn names(&self, package: &Package) -> Result<Names> {
    let extension = if package.platform == "windows" {
      ".exe"
    } else {
      ""
    };
    let version = crate::build_info::version_component(&package.application.version).map_err(anyhow::Error::msg)?;
    let base = format!("{}-setup-v{version}", package.application.id);
    let installer = self.installer_name.clone().unwrap_or_else(|| format!("{base}{extension}"));
    let launcher = self.launcher_name.clone().unwrap_or_else(|| format!("{base}-se{extension}"));
    for name in [&installer, &launcher] {
      files::name(name)?;
      ensure!(
        !["payload", "payload.zip", "installation.yaml"]
          .iter()
          .any(|reserved| name.eq_ignore_ascii_case(reserved)),
        "Reserved package filename: {name}"
      );
      ensure!(
        package.platform != "windows" || name.to_ascii_lowercase().ends_with(".exe"),
        "Windows package executables must end in .exe: {name}"
      );
    }
    // Unix executables have no mandatory extension. Append .yaml there so the
    // periods in a version are not mistaken for a file extension.
    let yaml = crate::files::installer_yaml_name(&installer);
    files::name(&yaml)?;
    ensure!(
      !installer.eq_ignore_ascii_case(&yaml),
      "Installer conflicts with its YAML filename"
    );
    ensure!(
      !launcher.eq_ignore_ascii_case(&installer) && !launcher.eq_ignore_ascii_case(&yaml),
      "Launcher and installer/YAML names must be distinct"
    );
    Ok(Names {
      installer,
      yaml,
      launcher,
    })
  }
}
