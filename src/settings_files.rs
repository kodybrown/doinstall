//! Initialize editable settings without treating application files as installer
//! templates or adopting a user's preexisting configuration.

use crate::{
  config, files,
  model::{Package, SettingsFile, SettingsFormat},
};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::{
  collections::BTreeMap,
  fs,
  io::Write,
  path::{Path, PathBuf},
};

pub struct Prepared {
  pub destination: PathBuf,
  bytes: Vec<u8>,
}

pub fn validate(package: &Package) -> Result<()> {
  let mut inputs = config::standard_inputs();
  inputs.extend(package.runtime.inputs.clone());
  for file in &package.runtime.settings_files {
    files::relative(&file.source)?;
    ensure!(!file.destination.is_empty(), "Settings destination is empty");
    for (pointer, input) in &file.set {
      ensure!(
        pointer.starts_with('/') && valid_pointer(pointer),
        "Invalid settings JSON pointer: {pointer}"
      );
      ensure!(inputs.contains_key(input), "Unknown settings input: {input}");
    }
  }
  Ok(())
}

fn valid_pointer(pointer: &str) -> bool {
  let mut chars = pointer.chars();
  while let Some(character) = chars.next() {
    if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
      return false;
    }
  }
  true
}

pub fn validate_sources(package: &Package, payload: &Path) -> Result<()> {
  for file in &package.runtime.settings_files {
    render(file, payload, None)?;
  }
  Ok(())
}

fn render(file: &SettingsFile, payload: &Path, values: Option<&BTreeMap<String, Value>>) -> Result<Vec<u8>> {
  let source = payload.join(files::relative(&file.source)?);
  files::safe_path(&source)?;
  ensure!(source.is_file(), "Missing settings defaults: {}", source.display());
  let original = fs::read_to_string(&source)?;
  let text = original.strip_prefix('\u{feff}').unwrap_or(&original);
  let mut document: Value = match file.format {
    SettingsFormat::Yaml => serde_saphyr::from_str(text)?,
    SettingsFormat::Json => serde_json::from_str(text)?,
  };
  for (pointer, input) in &file.set {
    let target = document
      .pointer_mut(pointer)
      .with_context(|| format!("Settings pointer does not exist: {pointer}"))?;
    ensure!(
      !target.is_null() && !target.is_array() && !target.is_object(),
      "Settings pointer must identify a scalar: {pointer}"
    );
    if let Some(values) = values {
      let value = values.get(input).with_context(|| format!("Missing settings input: {input}"))?;
      ensure!(
        (target.is_string() && value.is_string())
          || (target.is_boolean() && value.is_boolean())
          || (target.is_number() && value.is_number()),
        "Settings input type differs from defaults: {input}"
      );
      // Copy typed values, never interpolate serialized YAML. Quotes, backslashes,
      // newlines, and application-owned ${...} tokens remain ordinary content.
      *target = value.clone();
    }
  }
  if file.set.is_empty() {
    return Ok(original.into_bytes());
  }
  Ok(match file.format {
    SettingsFormat::Yaml => serde_saphyr::to_string(&document)?.into_bytes(),
    SettingsFormat::Json => {
      let mut bytes = serde_json::to_vec_pretty(&document)?;
      bytes.push(b'\n');
      bytes
    }
  })
}

pub fn prepare(
  package: &Package,
  payload: &Path,
  root: &Path,
  bin: &Path,
  values: &BTreeMap<String, Value>,
  settings: &[String],
  launchers: &[crate::launchers::Prepared],
) -> Result<Vec<Prepared>> {
  let mut result: Vec<Prepared> = Vec::new();
  for file in &package.runtime.settings_files {
    let expanded = config::expand(&file.destination, values)?;
    let destination = PathBuf::from(expanded);
    files::safe_path(&destination)?;
    ensure!(
      destination.parent().is_some() && destination.file_name().is_some(),
      "Settings destination must be a file"
    );
    let destination = files::absolute(&destination, root)?;
    ensure!(
      !files::overlaps(root, &destination)
        && !files::overlaps(payload, &destination)
        && !files::contains(&destination, bin),
      "Initialized settings must be outside application, payload, and command locations"
    );
    for base in config::builtins()?.values() {
      ensure!(destination != Path::new(base), "Settings cannot claim an entire user directory");
    }
    ensure!(
      !settings.iter().any(|path| files::overlaps(Path::new(path), &destination)),
      "settings-files must not overlap explicitly declared settings paths"
    );
    ensure!(
      !launchers
        .iter()
        .any(|launcher| files::overlaps(&launcher.destination, &destination)),
      "Initialized settings overlap a command or shortcut"
    );
    ensure!(
      !result.iter().any(|prior| files::overlaps(&prior.destination, &destination)),
      "Duplicate or overlapping settings destinations"
    );
    for managed in &package.runtime.managed_files {
      ensure!(
        !files::overlaps(Path::new(&config::expand(managed, values)?), &destination),
        "Initialized settings overlap lifecycle-managed files"
      );
    }
    existing_file(&destination)?;
    result.push(Prepared {
      destination,
      bytes: render(file, payload, Some(values))?,
    });
  }
  Ok(result)
}

fn existing_file(path: &Path) -> Result<bool> {
  files::safe_path(path)?;
  match fs::symlink_metadata(path) {
    Ok(metadata) => {
      ensure!(
        metadata.file_type().is_file(),
        "Settings destination is not a regular file: {}",
        path.display()
      );
      Ok(true)
    }
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
    Err(error) => Err(error.into()),
  }
}

/// Return true only for a file actually created here. A concurrent external
/// creator wins, and its file is preserved without claiming uninstall ownership.
pub fn initialize(file: &Prepared) -> Result<bool> {
  if existing_file(&file.destination)? {
    return Ok(false);
  }
  let parent = file.destination.parent().context("Settings destination has no parent")?;
  fs::create_dir_all(parent)?;
  files::safe_path(parent)?;
  let mut staged = tempfile::NamedTempFile::new_in(parent)?;
  staged.write_all(&file.bytes)?;
  staged.as_file().sync_all()?;
  match staged.persist_noclobber(&file.destination) {
    Ok(_) => Ok(true),
    Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
      existing_file(&file.destination)?;
      Ok(false)
    }
    Err(error) => Err(error.error.into()),
  }
}
