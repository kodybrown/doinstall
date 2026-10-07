use crate::{
  config,
  engine::{engine_name, fingerprint, identity, lock, verify_entries},
  files,
  model::*,
};
use anyhow::{ensure, Context, Result};
use std::{
  fs,
  path::{Path, PathBuf},
};

pub fn package(
  config_file: &Path,
  destination: &Path,
  named: bool,
  installer: &Path,
  result_file: Option<&Path>,
) -> Result<PathBuf> {
  let mut package: Package = files::read_yaml(config_file)?;
  config::validate(&package)?;
  ensure!(installer.is_file(), "Installer executable not found: {}", installer.display());
  let payload = files::absolute(Path::new(&package.payload), config_file.parent().unwrap())?;
  ensure!(
    !files::overlaps(destination, &payload),
    "Package output and payload must not overlap"
  );
  if let Some(result) = result_file {
    // Reporting an artifact must never mutate its inputs or the deliverable.
    ensure!(
      result != config_file && result != installer && !files::overlaps(result, &payload),
      "Result file must not overlap the config, installer executable, or payload"
    );
    ensure!(
      result.parent().is_some_and(Path::is_dir) && !result.is_dir(),
      "Result file requires an existing parent directory and a file path: {}",
      result.display()
    );
  }
  // Producers can supply the application's own embedded build information.
  // Otherwise freeze a packaging stamp once; installation never creates one.
  if package.application.build.is_none() {
    package.application.build = Some(Build {
      name: None,
      stamp: crate::build_info::Stamp::now().text(),
      commit: None,
      dirty: false,
    });
  }
  let directory_payload = payload.is_dir();
  let temporary = files::temporary()?;
  let staged_payload = temporary.path().join("payload");
  fs::create_dir(&staged_payload)?;
  if directory_payload {
    let inventory = files::inventory(&payload)?;
    files::copy_payload(&payload, &staged_payload, &inventory)?;
  } else {
    // Extract and distribute the same snapshot, even if a producer replaces its ZIP.
    let archive = temporary.path().join("payload.zip");
    fs::copy(&payload, &archive).with_context(|| format!("Copy payload ZIP {}", payload.display()))?;
    files::extract(&archive, &staged_payload)?;
  }
  verify_entries(&package, &staged_payload)?;
  let inventory = files::inventory(&staged_payload)?;
  ensure!(!inventory.is_empty(), "Payload is empty");
  let identity = identity(&package, &inventory)?;
  let output = if named {
    destination.join(format!("{}-{}", package.application.id, identity))
  } else {
    destination.to_path_buf()
  };
  ensure!(
    !files::overlaps(&output, &payload),
    "Package output and payload must not overlap"
  );
  if let Some(result) = result_file {
    ensure!(
      !files::overlaps(result, &output),
      "Result file must be outside the installer package: {}",
      result.display()
    );
  }
  let _package_lock = lock(&output)?;
  // Check under the same lock used for publication, including explicit outputs.
  if !named {
    ensure!(!output.exists(), "Package output already exists: {}", output.display());
  }
  if named && output.exists() {
    files::safe_path(&output)?;
    let existing: Package = files::read_yaml(&output.join("installer.yaml"))?;
    let existing_payload = files::absolute(Path::new(&existing.payload), &output)?;
    let existing_temporary = files::temporary()?;
    let existing_inventory = if existing_payload.is_dir() {
      files::inventory(&existing_payload)?
    } else {
      files::extract(&existing_payload, existing_temporary.path())?;
      files::inventory(existing_temporary.path())?
    };
    ensure!(
      fingerprint(&existing, &existing_inventory)? == fingerprint(&package, &inventory)?
        && files::file_hash(&output.join(engine_name()))? == files::file_hash(installer)?,
      "Package name already belongs to different contents: {}. Rebuild the application with a new build stamp.",
      output.display()
    );
    println!("Reused installer: {}", output.join(engine_name()).display());
    return Ok(output.join(engine_name()));
  }
  // Publish a complete independent copy by renaming a sibling staging directory.
  files::safe_path(&output)?;
  fs::create_dir_all(output.parent().context("Output needs a parent")?)?;
  let final_stage = tempfile::Builder::new()
    .prefix("installer-package-")
    .tempdir_in(output.parent().unwrap())?;
  if directory_payload {
    let target = final_stage.path().join("payload");
    fs::create_dir(&target)?;
    files::copy_payload(&staged_payload, &target, &files::inventory(&staged_payload)?)?;
    package.payload = "payload".into();
  } else {
    fs::copy(temporary.path().join("payload.zip"), final_stage.path().join("payload.zip"))?;
    package.payload = "payload.zip".into();
  }
  fs::copy(installer, final_stage.path().join(engine_name()))?;
  files::write_yaml(&final_stage.path().join("installer.yaml"), &package)?;
  fs::rename(final_stage.path(), &output)?;
  println!("Created installer: {}", output.join(engine_name()).display());
  Ok(output.join(engine_name()))
}
