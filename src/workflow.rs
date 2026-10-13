//! Included tools have ordinary argv/env contracts; no task engine is embedded here.
use crate::{config, files, included_process, model::*};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
  collections::BTreeMap,
  fs,
  path::{Path, PathBuf},
  process::Command,
  time::Duration,
};

pub(crate) fn validate(package: &Package) -> Result<()> {
  if let Some(host) = &package.runtime.host {
    ensure!(
      package.platform == "linux" && !host.distributions.is_empty(),
      "Host requirements need Linux and an explicit distribution/version allowlist."
    );
    for (id, versions) in &host.distributions {
      ensure!(
        ["debian", "ubuntu"].contains(&id.as_str()) && !versions.is_empty() && versions.iter().all(|v| !v.is_empty()),
        "Only explicit Debian/Ubuntu versions are supported."
      );
    }
  }
  for (name, tool) in &package.runtime.tools {
    files::name(name)?;
    files::relative(&tool.executable)?;
    ensure!(
      tool.sha256.len() == 64 && tool.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
      "Included tool '{name}' needs its pinned SHA-256."
    );
  }
  for call in package
    .runtime
    .lifecycle
    .prepare
    .iter()
    .chain(&package.runtime.lifecycle.before_activate)
    .chain(&package.runtime.lifecycle.after_activate)
  {
    ensure!(
      package.runtime.tools.contains_key(&call.tool),
      "Unknown included tool '{}'.",
      call.tool
    );
    ensure!(
      (1..=86400).contains(&call.timeout_seconds),
      "Tool timeout must be from 1 to 86400 seconds."
    );
    ensure!(
      call.arguments.iter().all(|s| !s.contains('\0')),
      "Tool arguments cannot contain NUL."
    );
    for (name, value) in &call.environment {
      ensure!(
        !name.is_empty() && !name.contains(['=', '\0']) && !value.contains('\0'),
        "Invalid tool environment."
      );
      ensure!(
        !["DOINSTALL_CONTEXT_FILE", "DOINSTALL_PROCESS_GROUP_OWNER"]
          .iter()
          .any(|reserved| name.eq_ignore_ascii_case(reserved)),
        "Installer context/process ownership environment variables are reserved."
      );
    }
  }
  for name in &package.runtime.managed_files {
    ensure!(!name.is_empty() && !name.contains('\0'), "Managed file paths must be nonempty.");
  }
  ensure!(
    package.runtime.managed_files.is_empty() || !package.runtime.lifecycle.is_empty(),
    "Managed files require a lifecycle."
  );
  Ok(())
}
pub(crate) fn validate_entries(package: &Package, payload: &Path) -> Result<()> {
  for (name, tool) in &package.runtime.tools {
    let path = payload.join(files::relative(&tool.executable)?);
    files::safe_path(&path)?;
    ensure!(
      path.is_file() && files::file_hash(&path)?.eq_ignore_ascii_case(&tool.sha256),
      "Included tool '{name}' is missing or does not match its pinned hash."
    );
  }
  Ok(())
}

fn record(path: &Path) -> Result<ManagedFile> {
  files::safe_path(path)?;
  if !path.exists() {
    return Ok(ManagedFile::default());
  }
  let meta = fs::metadata(path)?;
  ensure!(
    meta.is_file() && meta.len() <= 16 * 1024 * 1024,
    "Managed config must be a regular file of at most 16 MiB: {}",
    path.display()
  );
  #[cfg(unix)]
  let mode = {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o777
  };
  #[cfg(windows)]
  let mode = 0;
  Ok(ManagedFile {
    hash: Some(files::file_hash(path)?),
    mode,
  })
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Snapshot {
  before: ManagedFile,
  observed: ManagedFile,
  backup: Option<String>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Journal {
  schema: u32,
  build: String,
  config_hash: String,
  values_hash: String,
  phase: String,
  call: usize,
  state: String,
  files: BTreeMap<String, Snapshot>,
}
pub(crate) struct Session {
  journal_path: PathBuf,
  journal: Journal,
  context: tempfile::NamedTempFile,
  values: BTreeMap<String, Value>,
}

pub(crate) fn preflight(
  package: &Package,
  root: &Path,
  values: &BTreeMap<String, Value>,
  owned: &BTreeMap<String, ManagedFile>,
  build: &str,
  config_hash: &str,
) -> Result<BTreeMap<String, ManagedFile>> {
  let path = root.join("installer/workflow.yaml");
  let prior: Option<Journal> = if path.exists() {
    Some(files::read_yaml(&path)?)
  } else {
    None
  };
  let interrupted = root.join("installer/pending.yaml").exists();
  let values_hash = files::hash(&serde_json::to_vec(values)?);
  if let Some(prior) = &prior {
    ensure!(prior.schema == 1, "Unknown workflow journal schema.");
    if prior.build == build && prior.config_hash == config_hash {
      ensure!(
        prior.values_hash == values_hash,
        "Workflow inputs changed; use a new package/build for new settings."
      );
    }
    ensure!(
      (prior.state == "complete" && !interrupted) || (prior.build == build && prior.config_hash == config_hash),
      "Retry the original package to finish its interrupted workflow."
    );
  }
  let mut requested = BTreeMap::new();
  for template in &package.runtime.managed_files {
    let path = PathBuf::from(config::expand(template, values)?);
    ensure!(
      path.is_absolute() && !files::overlaps(root, &path),
      "Managed configuration must be an absolute path outside the installation."
    );
    let name = path.to_str().context("Managed configuration paths require UTF-8")?.to_owned();
    ensure!(!requested.contains_key(&name), "Duplicate managed configuration path.");
    let actual = record(&path)?;
    let expected = prior
      .as_ref()
      .filter(|j| j.state != "complete" || interrupted)
      .and_then(|j| j.files.get(&name))
      .map(|s| &s.observed)
      .or_else(|| owned.get(&name));
    if let Some(expected) = expected {
      ensure!(
        &actual == expected,
        "Managed configuration changed outside this installation: {name}"
      );
    } else {
      ensure!(
        actual.hash.is_none(),
        "Refusing to adopt existing unowned configuration: {name}"
      );
    }
    requested.insert(name, actual);
  }
  // Dropping a resource from the manifest cannot silently abandon ownership.
  ensure!(
    owned.keys().all(|name| requested.contains_key(name)),
    "Removing managed configuration requires a separate cleanup operation."
  );
  Ok(requested)
}

impl Session {
  pub(crate) fn begin(
    package: &Package,
    root: &Path,
    directory: &Path,
    receipt: &Receipt,
    configs: BTreeMap<String, ManagedFile>,
  ) -> Result<Self> {
    let build = receipt.active.as_str();
    let config_hash = receipt.config_hash.as_str();
    let values = &receipt.values;
    let previous = receipt.previous.as_deref();
    let management = root.join("installer");
    let journal_path = management.join("workflow.yaml");
    let prior: Option<Journal> = if journal_path.exists() {
      Some(files::read_yaml(&journal_path)?)
    } else {
      None
    };
    let journal = if let Some(journal) = prior.filter(|j| j.build == build && j.config_hash == config_hash) {
      journal
    } else {
      let snapshots = management.join("config").join(build);
      files::safe_path(&snapshots)?;
      ensure!(
        !snapshots.exists(),
        "Configuration snapshot already exists for a different installation attempt."
      );
      fs::create_dir_all(&snapshots)?;
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&snapshots, fs::Permissions::from_mode(0o700))?;
      }
      let mut records = BTreeMap::new();
      for (index, (path, before)) in configs.into_iter().enumerate() {
        let backup = if before.hash.is_some() {
          let destination = snapshots.join(index.to_string());
          files::write_atomic(&destination, &fs::read(&path)?)?;
          ensure!(
            Some(files::file_hash(&destination)?) == before.hash,
            "Configuration changed while backing up: {path}"
          );
          ensure!(
            record(Path::new(&path))? == before,
            "Configuration changed while backing up: {path}"
          );
          #[cfg(unix)]
          {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&destination, fs::Permissions::from_mode(0o600))?;
          }
          Some(destination.to_string_lossy().into_owned())
        } else {
          None
        };
        records.insert(
          path,
          Snapshot {
            observed: before.clone(),
            before,
            backup,
          },
        );
      }
      Journal {
        schema: 1,
        build: build.into(),
        config_hash: config_hash.into(),
        values_hash: files::hash(&serde_json::to_vec(values)?),
        phase: "prepare".into(),
        call: 0,
        state: "ready".into(),
        files: records,
      }
    };
    let mut resolved = values.clone();
    let mut context = tempfile::NamedTempFile::new()?;
    use std::io::Write;
    context.write_all(&serde_json::to_vec(&json!({
      "schema":1, "application-id":package.application.id, "install-dir":root, "build-dir":directory,
      "previous-build-dir":previous.map(|id| root.join("app").join(id)), "management-dir":management, "values":values
    }))?)?;
    context.as_file().sync_all()?;
    resolved.insert("build-dir".into(), json!(directory));
    resolved.insert("context-file".into(), json!(context.path()));
    files::write_yaml(&journal_path, &journal)?;
    Ok(Self {
      journal_path,
      journal,
      context,
      values: resolved,
    })
  }
  pub(crate) fn phase(&mut self, package: &Package, directory: &Path, name: &str, calls: &[ToolCall]) -> Result<()> {
    for (index, call) in calls.iter().enumerate() {
      self.journal.phase = name.into();
      self.journal.call = index;
      self.journal.state = "running".into();
      files::write_yaml(&self.journal_path, &self.journal)?;
      let tool = &package.runtime.tools[&call.tool];
      let executable = directory.join(files::relative(&tool.executable)?);
      files::safe_path(&executable)?;
      files::verify_file(&executable, &tool.sha256.to_ascii_lowercase())?;
      let mut command = Command::new(executable);
      command
        .current_dir(directory)
        .env("DOINSTALL_CONTEXT_FILE", self.context.path());
      for argument in &call.arguments {
        command.arg(config::expand(argument, &self.values)?);
      }
      for (key, value) in &call.environment {
        command.env(key, config::expand(value, &self.values)?);
      }
      crate::console::checkpoint(&format!("Running included tool '{}' ({name})...", call.tool), false)?;
      let result = included_process::run_timeout(&mut command, Some(Duration::from_secs(call.timeout_seconds)));
      // Record partial file changes on failure. A hard crash between tool writes
      // and this checkpoint requires inspection rather than adoption on retry.
      for (path, snapshot) in &mut self.journal.files {
        snapshot.observed = record(Path::new(path))?;
      }
      self.journal.state = if result.as_ref().is_ok_and(|code| *code == 0) {
        "ready"
      } else {
        "failed"
      }
      .into();
      files::write_yaml(&self.journal_path, &self.journal)?;
      let code = result?;
      ensure!(code == 0, "Included tool '{}' failed during {name} with exit code {code}. Retry the original package; application/database rollback is not implied.", call.tool);
    }
    Ok(())
  }
  pub(crate) fn complete(&mut self) -> Result<BTreeMap<String, ManagedFile>> {
    self.journal.state = "complete".into();
    files::write_yaml(&self.journal_path, &self.journal)?;
    Ok(
      self
        .journal
        .files
        .iter()
        .map(|(path, snapshot)| (path.clone(), snapshot.observed.clone()))
        .collect(),
    )
  }
}
