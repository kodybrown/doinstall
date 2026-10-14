use crate::{
  config,
  engine::{fingerprint, identity, lock, verify_entries},
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
  launcher_template: Option<&Path>,
) -> Result<PathBuf> {
  let (mut package, settings) = crate::packaging::read(config_file)?;
  config::validate(&package)?;
  let names = settings.names(&package)?;
  ensure!(installer.is_file(), "Installer executable not found: {}", installer.display());
  crate::binary::validate(installer, &package.platform, &package.architecture)?;
  let engine_name = names.installer.as_str();
  if let Some(template) = launcher_template {
    crate::binary::validate_launcher(template, &package.platform, &package.architecture)?;
    crate::self_extract::template_hash(template)?;
  }
  let payload = files::absolute(Path::new(&package.payload), config_file.parent().unwrap())?;
  ensure!(
    !files::overlaps(destination, &payload),
    "Package output and payload must not overlap"
  );
  if let Some(result) = result_file {
    // Reporting an artifact must never mutate its inputs or the deliverable.
    ensure!(
      result != config_file
        && result != installer
        && launcher_template != Some(result)
        && !files::overlaps(result, &payload),
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
  let cross_unix = package.platform != "windows" && config::platform() != package.platform;
  let directory_payload = payload.is_dir() && !cross_unix;
  let temporary = files::temporary()?;
  let staged_payload = temporary.path().join("payload");
  fs::create_dir(&staged_payload)?;
  if payload.is_dir() {
    let inventory = files::inventory(&payload)?;
    files::copy_payload(&payload, &staged_payload, &inventory)?;
  } else {
    // Extract and distribute the same snapshot, even if a producer replaces its ZIP.
    let archive = temporary.path().join("payload.zip");
    fs::copy(&payload, &archive).with_context(|| format!("Copy payload ZIP {}", payload.display()))?;
    files::extract(&archive, &staged_payload)?;
  }
  verify_entries(&package, &staged_payload)?;
  let mut inventory = files::inventory(&staged_payload)?;
  if cross_unix {
    // Carry target modes in a ZIP when the builder cannot represent them.
    target_modes(
      &package,
      &mut inventory,
      (!payload.is_dir()).then(|| temporary.path().join("payload.zip")).as_deref(),
    )?;
    let archive = temporary.path().join("unix-payload.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive)?);
    zip_directories(&mut zip, &staged_payload, &staged_payload)?;
    for (name, record) in &inventory {
      zip.start_file(name, zip::write::SimpleFileOptions::default().unix_permissions(record.mode))?;
      std::io::copy(&mut fs::File::open(staged_payload.join(name))?, &mut zip)?;
    }
    zip.finish()?;
  }
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
  if let Some(template) = launcher_template {
    ensure!(
      !files::overlaps(&output, template),
      "Package output must not overlap the launcher template"
    );
  }
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
    let extracted = files::temporary()?;
    let existing_root = if let Some(template) = launcher_template {
      let bundle = crate::self_extract::Bundle::open(&output.join(&names.launcher))?;
      ensure!(
        bundle.manifest.template_hash == crate::self_extract::template_hash(template)?,
        "Package name already belongs to a different launcher template"
      );
      bundle.extract(extracted.path())?;
      extracted.path()
    } else {
      output.as_path()
    };
    let existing: Package = files::read_yaml(&existing_root.join(&names.yaml))?;
    let existing_payload = files::absolute(Path::new(&existing.payload), existing_root)?;
    let existing_temporary = files::temporary()?;
    let mut existing_inventory = if existing_payload.is_dir() {
      files::inventory(&existing_payload)?
    } else {
      files::extract(&existing_payload, existing_temporary.path())?;
      files::inventory(existing_temporary.path())?
    };
    if cross_unix {
      target_modes(
        &existing,
        &mut existing_inventory,
        (!existing_payload.is_dir()).then_some(existing_payload.as_path()),
      )?;
    }
    ensure!(
      fingerprint(&existing, &existing_inventory)? == fingerprint(&package, &inventory)?
        && files::file_hash(&existing_root.join(engine_name))? == files::file_hash(installer)?,
      "Package name already belongs to different contents: {}. Rebuild the application with a new build stamp.",
      output.display()
    );
    let artifact = output.join(if launcher_template.is_some() {
      &names.launcher
    } else {
      engine_name
    });
    println!("Reused installer: {}", artifact.display());
    return Ok(artifact);
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
    let archive = temporary.path().join(if cross_unix {
      "unix-payload.zip"
    } else {
      "payload.zip"
    });
    fs::copy(archive, final_stage.path().join("payload.zip"))?;
    package.payload = "payload.zip".into();
  }
  fs::copy(installer, final_stage.path().join(engine_name))?;
  files::write_yaml(&final_stage.path().join(&names.yaml), &package)?;
  if let Some(template) = launcher_template {
    let delivery = tempfile::Builder::new()
      .prefix("installer-launcher-")
      .tempdir_in(output.parent().unwrap())?;
    crate::self_extract::assemble(
      template,
      final_stage.path(),
      engine_name,
      &package.platform,
      &package.architecture,
      &fingerprint(&package, &inventory)?,
      &delivery.path().join(&names.launcher),
    )?;
    fs::rename(delivery.path(), &output)?;
    let artifact = output.join(&names.launcher);
    println!("Created launcher: {}", artifact.display());
    return Ok(artifact);
  }
  fs::rename(final_stage.path(), &output)?;
  println!("Created installer: {}", output.join(engine_name).display());
  Ok(output.join(engine_name))
}

fn target_modes(
  package: &Package,
  inventory: &mut std::collections::BTreeMap<String, FileRecord>,
  archive: Option<&Path>,
) -> Result<()> {
  let executable = package
    .runtime
    .commands
    .iter()
    .map(|c| c.executable.as_str())
    .chain(package.runtime.shortcuts.iter().map(|s| s.executable.as_str()))
    .chain(package.runtime.tools.values().map(|t| t.executable.as_str()))
    .collect::<std::collections::BTreeSet<_>>();
  let mut modes = std::collections::BTreeMap::new();
  if let Some(archive) = archive {
    let mut zip = zip::ZipArchive::new(fs::File::open(archive)?)?;
    for index in 0..zip.len() {
      let entry = zip.by_index(index)?;
      if let Some(mode) = entry.unix_mode()
        && mode & 0o777 != 0
      {
        modes.insert(entry.name().to_owned(), mode & 0o777);
      }
    }
  }
  for (name, record) in inventory {
    record.mode = modes.get(name).copied().unwrap_or(if executable.contains(name.as_str()) {
      0o755
    } else {
      0o644
    });
    ensure!(
      !executable.contains(name.as_str()) || record.mode & 0o111 != 0,
      "Declared entry/tool is not executable in payload ZIP: {name}"
    );
  }
  Ok(())
}
fn zip_directories(zip: &mut zip::ZipWriter<fs::File>, root: &Path, current: &Path) -> Result<()> {
  for item in fs::read_dir(current)? {
    let path = item?.path();
    if path.is_dir() {
      let name = path
        .strip_prefix(root)?
        .to_str()
        .context("Non UTF-8 payload directory")?
        .replace('\\', "/")
        + "/";
      zip.add_directory(name, zip::write::SimpleFileOptions::default().unix_permissions(0o755))?;
      zip_directories(zip, root, &path)?;
    }
  }
  Ok(())
}
