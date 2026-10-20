//! Native release assembly and acceptance, bootstrapped solely by Cargo.
//! Only allowlisted files enter deliveries; never archive a checkout/build tree.
use anyhow::{bail, ensure, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
  collections::{BTreeMap, BTreeSet},
  fs,
  io::{Cursor, Read, Write},
  path::{Path, PathBuf},
  process::Command,
};
#[path = "../../src/tool_version.rs"]
mod clock;
pub const TOOLS: [&str; 4] = [
  "doinstall",
  "doinstall-builder",
  "doinstall-launcher",
  "doinstall-server",
];
type Inventory = BTreeMap<String, String>;
type Entry = (String, Vec<u8>, u32);

pub fn capture(command: &mut Command) -> Result<String> {
  let output = command.output().context("Cannot start release tool")?;
  ensure!(
    output.status.success(),
    "Release tool failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  Ok(String::from_utf8(output.stdout)?.trim().into())
}
fn run(command: &mut Command) -> Result<()> {
  super::execute(command)
}
pub fn sha(data: &[u8]) -> String {
  format!("{:x}", Sha256::digest(data))
}
fn write_json(path: &Path, value: &Value) -> Result<()> {
  fs::write(path, serde_json::to_string_pretty(value)? + "\n")?;
  Ok(())
}
pub fn version(tag: Option<&str>) -> Result<String> {
  let data = super::metadata()?;
  let version = data["packages"]
    .as_array()
    .context("Missing Cargo packages")?
    .iter()
    .find(|p| p["name"] == "doinstall")
    .context("Missing doinstall package")?["version"]
    .as_str()
    .context("Missing version")?
    .to_owned();
  ensure!(
    Regex::new(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")?.is_match(&version),
    "Release version must be MAJOR.MINOR.PATCH"
  );
  if let Some(tag) = tag {
    ensure!(tag == format!("v{version}"), "Tag must exactly match Cargo.toml: v{version}");
  }
  Ok(version)
}
pub fn markers() -> Result<BTreeSet<String>> {
  let mut paths = vec![
    super::root(),
    super::target()?,
    PathBuf::from(capture(Command::new("rustc").args(["--print", "sysroot"]))?),
  ];
  for key in [
    "HOME",
    "USERPROFILE",
    "CARGO_HOME",
    "RUNNER_TEMP",
    "GITHUB_WORKSPACE",
    "CARGO_TARGET_DIR",
  ] {
    if let Some(value) = std::env::var_os(key) {
      paths.push(value.into());
    }
  }
  Ok(
    paths
      .into_iter()
      .map(|p| p.to_string_lossy().into_owned())
      .filter(|p| p.len() > 3)
      .collect(),
  )
}
pub fn scan(data: &[u8], label: &str, markers: &BTreeSet<String>) -> Result<()> {
  // Inspect ASCII and UTF-16LE without printing matched credentials or paths.
  // Pattern checks complement the closed inventory, not a complete secret detector.
  let ascii: String = data.iter().map(|v| char::from(*v)).collect();
  let utf16 = String::from_utf16_lossy(
    &data
      .chunks_exact(2)
      .map(|v| u16::from_le_bytes([v[0], v[1]]))
      .collect::<Vec<_>>(),
  );
  let text = ascii + "\n" + &utf16;
  let folded = text.replace('\\', "/").to_lowercase();
  for marker in markers {
    let marker = marker.replace('\\', "/").trim_start_matches("//?/").to_lowercase();
    ensure!(!folded.contains(&marker), "Private build path in {label}");
  }
  for pattern in [
    r"[A-Za-z]:[/\\](?:Users|agent|actions-runner|tmp)[/\\]",
    r"/(?:home/[^/\s]+|Users/[^/\s]+|root/\.cargo|__w/|github/workspace/)",
    r"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
    r"\b(?:gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{50,}|AKIA[A-Z0-9]{16})\b",
    r"(?i)(?:GITHUB_TOKEN|ACTIONS_RUNTIME_TOKEN|AWS_SECRET_ACCESS_KEY)\s*=\s*\S+",
  ] {
    ensure!(!Regex::new(pattern)?.is_match(&text), "Private content pattern in {label}");
  }
  Ok(())
}
pub fn configure(command: &mut Command, seconds: u64) -> Result<()> {
  // Encoded argument boundaries preserve Windows paths containing spaces. Own
  // these flags for distribution builds rather than capturing the environment.
  let mut flags = Vec::<String>::new();
  let root = super::root();
  let sysroot = capture(Command::new("rustc").args(["--print", "sysroot"]))?;
  let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME"))?;
  let cargo = std::env::var("CARGO_HOME").unwrap_or_else(|_| format!("{home}/.cargo"));
  for (private, public) in [
    (root.to_string_lossy().into_owned(), "/doinstall"),
    (sysroot, "/rust-toolchain"),
    (cargo, "/cargo"),
    (home, "/build-home"),
  ] {
    for spelling in BTreeSet::from([
      private.clone(),
      private.replace('\\', "/"),
      private.trim_start_matches(r"\\?\").into(),
    ]) {
      flags.push(format!("--remap-path-prefix={spelling}={public}"));
    }
  }
  if cfg!(windows) {
    flags.extend(
      [
        "-C",
        "target-feature=+crt-static",
        "-C",
        "link-arg=/PDBALTPATH:doinstall.pdb",
      ]
      .map(String::from),
    );
  }
  command
    .env_remove("RUSTFLAGS")
    .env("CARGO_ENCODED_RUSTFLAGS", flags.join("\x1f"))
    .env("RUST_BUILD_TIMESTAMP", seconds.to_string());
  Ok(())
}
pub fn build_tools() -> Result<()> {
  let metadata = super::metadata()?;
  let root = fs::canonicalize(super::root())?;
  for key in ["target_directory", "build_directory"] {
    if let Some(path) = metadata[key].as_str() {
      ensure!(
        !destination(Path::new(path))?.starts_with(&root),
        "Cargo output is inside source"
      );
    }
  }
  let mut command = Command::new("cargo");
  command
    .current_dir(super::root())
    .args(["build", "--package", "doinstall", "--locked", "--release"]);
  configure(&mut command, clock::now())?;
  run(&mut command)?;
  let markers = markers()?;
  for name in TOOLS {
    scan(
      &fs::read(super::target()?.join("release").join(super::installer::executable(name)))?,
      name,
      &markers,
    )?;
  }
  Ok(())
}
fn files(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
  for entry in fs::read_dir(directory)? {
    let entry = entry?;
    ensure!(!entry.file_type()?.is_symlink(), "No links in release distribution");
    if entry.file_type()?.is_dir() {
      files(root, &entry.path(), output)?;
    } else {
      ensure!(entry.file_type()?.is_file(), "Nonregular distribution file");
      output.push(entry.path().strip_prefix(root)?.into());
    }
  }
  output.sort();
  Ok(())
}
fn inventory(root: &Path) -> Result<Inventory> {
  let mut paths = vec![];
  files(root, root, &mut paths)?;
  paths
    .into_iter()
    .map(|p| Ok((p.to_string_lossy().replace('\\', "/"), sha(&fs::read(root.join(&p))?))))
    .collect()
}
fn licenses(stage: &Path) -> Result<()> {
  // Carry the upstream license/notice texts, including both dual-license
  // alternatives; an SPDX expression alone is not a distribution notice.
  let triple = if cfg!(windows) {
    "x86_64-pc-windows-msvc"
  } else {
    "x86_64-unknown-linux-gnu"
  };
  let metadata: Value = serde_json::from_str(&capture(Command::new("cargo").current_dir(super::root()).args([
    "metadata",
    "--locked",
    "--format-version",
    "1",
    "--filter-platform",
    triple,
  ]))?)?;
  let packages = metadata["packages"].as_array().context("Missing packages")?;
  let nodes = metadata["resolve"]["nodes"].as_array().context("Missing dependency graph")?;
  let mut pending = vec![packages
    .iter()
    .find(|p| p["name"] == "doinstall" && p["source"].is_null())
    .context("Missing root")?["id"]
    .as_str()
    .unwrap()
    .to_owned()];
  let mut included = BTreeSet::new();
  while let Some(id) = pending.pop() {
    if included.insert(id.clone()) {
      pending.extend(
        nodes.iter().find(|n| n["id"] == id).context("Missing dependency")?["dependencies"]
          .as_array()
          .context("Missing dependencies")?
          .iter()
          .map(|v| v.as_str().unwrap().to_owned()),
      );
    }
  }
  let mut notices = vec![];
  for package in packages {
    if package["source"].is_null() || !included.contains(package["id"].as_str().unwrap()) {
      continue;
    }
    let name = format!("{}-{}", package["name"].as_str().unwrap(), package["version"].as_str().unwrap());
    let mut directory = PathBuf::from(package["manifest_path"].as_str().unwrap())
      .parent()
      .unwrap()
      .to_path_buf();
    let mut selected = vec![];
    for entry in fs::read_dir(&directory)? {
      let entry = entry?;
      let name = entry.file_name().to_string_lossy().to_uppercase();
      if ["LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE"]
        .iter()
        .any(|s| name.starts_with(s))
      {
        if entry.file_type()?.is_dir() {
          files(&directory, &entry.path(), &mut selected)?;
        } else if entry.file_type()?.is_file() {
          selected.push(entry.path().strip_prefix(&directory)?.into());
        }
      }
    }
    if selected.is_empty() {
      // Reviewed exceptions are pinned by crate name/version and upstream source
      // under packaging/licenses; missing notices still fail closed.
      directory = super::root().join("packaging/licenses").join(&name);
      files(&directory, &directory, &mut selected)
        .with_context(|| format!("Missing dependency license text: {name}"))?;
    }
    ensure!(!selected.is_empty(), "Missing dependency license text: {name}");
    selected.sort();
    let mut texts = vec![];
    for path in selected {
      let relative = format!("LICENSES/{name}/{}", path.to_string_lossy().replace('\\', "/"));
      let destination = stage.join(&relative);
      fs::create_dir_all(destination.parent().unwrap())?;
      fs::copy(directory.join(path), destination)?;
      texts.push(relative);
    }
    notices.push(
      json!({"name": package["name"], "version": package["version"], "license": package["license"], "texts": texts}),
    );
  }
  write_json(&stage.join("THIRD-PARTY.json"), &json!(notices))
}
pub fn archive_tools(stage: &Path, archive: &Path, windows: bool) -> Result<()> {
  let names = inventory(stage)?;
  if windows {
    let mut writer = zip::ZipWriter::new(fs::File::create(archive)?);
    for name in names.keys() {
      writer.start_file(
        name,
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated),
      )?;
      writer.write_all(&fs::read(stage.join(name))?)?;
    }
    writer.finish()?;
  } else {
    // Retain executable modes but normalize owner names/IDs and archive times;
    // private machine ownership must never become part of the delivery.
    let gzip = flate2::write::GzEncoder::new(fs::File::create(archive)?, flate2::Compression::default());
    let mut writer = tar::Builder::new(gzip);
    for name in names.keys() {
      let bytes = fs::read(stage.join(name))?;
      let mut header = tar::Header::new_gnu();
      header.set_size(bytes.len() as u64);
      header.set_uid(0);
      header.set_gid(0);
      header.set_mtime(0);
      #[cfg(unix)]
      let mode = {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(stage.join(name))?.permissions().mode() & 0o777
      };
      #[cfg(not(unix))]
      let mode = 0o755;
      header.set_mode(mode);
      header.set_cksum();
      writer.append_data(&mut header, name, bytes.as_slice())?;
    }
    writer.into_inner()?.finish()?;
  }
  Ok(())
}
fn safe_name(name: &str) -> bool {
  !name.starts_with('/') && !name.contains(['\\', ':']) && name.split('/').all(|p| !["", ".", ".."].contains(&p))
}
fn zip_entries(reader: impl Read + std::io::Seek) -> Result<Vec<Entry>> {
  let mut archive = zip::ZipArchive::new(reader)?;
  let mut result = vec![];
  for index in 0..archive.len() {
    let mut entry = archive.by_index(index)?;
    let mode = entry.unix_mode().unwrap_or(0o100644);
    ensure!(
      !entry.is_dir() && (mode & 0o170000 == 0 || mode & 0o170000 == 0o100000),
      "Only regular archive files allowed"
    );
    let name = entry.name().to_owned();
    let mut bytes = vec![];
    entry.read_to_end(&mut bytes)?;
    result.push((name, bytes, mode));
  }
  validate_entries(&result)?;
  Ok(result)
}
fn validate_entries(entries: &[Entry]) -> Result<()> {
  let mut names = BTreeSet::new();
  for (name, _, _) in entries {
    ensure!(safe_name(name) && names.insert(name), "Unsafe or duplicate archive path");
  }
  Ok(())
}
fn archive_entries(path: &Path) -> Result<Vec<Entry>> {
  if path.extension().is_some_and(|s| s == "zip") {
    return zip_entries(fs::File::open(path)?);
  }
  let mut result = vec![];
  for item in tar::Archive::new(flate2::read::GzDecoder::new(fs::File::open(path)?)).entries()? {
    let mut item = item?;
    ensure!(item.header().entry_type().is_file(), "Only regular archive files allowed");
    let name = String::from_utf8(item.path_bytes().to_vec())?;
    let mode = item.header().mode()?;
    let mut bytes = vec![];
    item.read_to_end(&mut bytes)?;
    result.push((name, bytes, mode));
  }
  validate_entries(&result)?;
  Ok(result)
}
fn query_tools(directory: &Path) -> Result<()> {
  let query: Value = serde_json::from_str(&capture(
    Command::new(directory.join(super::installer::executable("doinstall-builder"))).arg("--print-tool-paths"),
  )?)?;
  for (field, name) in [
    ("Installer", "doinstall"),
    ("LauncherTemplate", "doinstall-launcher"),
  ] {
    ensure!(
      fs::canonicalize(query[field].as_str().context("Missing tool path")?)?
        == fs::canonicalize(directory.join(super::installer::executable(name)))?,
      "Adjacent tool discovery mismatch"
    );
  }
  let mut identities = BTreeSet::new();
  for name in TOOLS {
    let output = capture(Command::new(directory.join(super::installer::executable(name))).arg("--version"))?;
    ensure!(output.starts_with(&format!("{name} ")), "Missing tool build identity");
    identities.insert(output[name.len()..].to_owned());
  }
  ensure!(identities.len() == 1, "Tools do not share one build identity");
  Ok(())
}
fn inspect_installer(path: &Path, expected: &Inventory, markers: &BTreeSet<String>) -> Result<()> {
  let bytes = fs::read(path)?;
  ensure!(bytes.len() >= 40, "Short installer");
  let footer = bytes.len() - 40;
  let number = |offset| -> Result<usize> {
    Ok(u64::from_le_bytes(bytes[footer + offset..footer + offset + 8].try_into()?).try_into()?)
  };
  let start = number(0)?;
  let length = number(8)?;
  let manifest_length = number(16)?;
  ensure!(
    &bytes[footer + 24..] == b"DOINSTALL-SE-v1!"
      && start.checked_add(length).and_then(|v| v.checked_add(manifest_length)) == Some(footer),
    "Invalid installer bounds/footer"
  );
  scan(&bytes[..start], "installer launcher", markers)?;
  scan(&bytes[start + length..footer], "installer manifest", markers)?;
  let manifest: Value = serde_json::from_slice(&bytes[start + length..footer])?;
  let runtime = manifest["installer"].as_str().context("Missing installer")?;
  let mut allowed: BTreeSet<String> = [
    runtime.into(),
    format!("{}.yaml", runtime.trim_end_matches(".exe")),
  ]
  .into();
  allowed.extend(expected.keys().map(|n| format!("payload/{n}")));
  let mut archive = zip::ZipArchive::new(Cursor::new(&bytes[start..start + length]))?;
  let mut found = BTreeSet::new();
  for i in 0..archive.len() {
    let mut entry = archive.by_index(i)?;
    if entry.is_dir() {
      continue;
    }
    let name = entry.name().to_owned();
    ensure!(safe_name(&name) && found.insert(name.clone()), "Unsafe installer entry");
    let mut bytes = vec![];
    entry.read_to_end(&mut bytes)?;
    scan(&bytes, &format!("installer:{name}"), markers)?;
    if let Some(payload) = name.strip_prefix("payload/") {
      ensure!(
        expected.get(payload) == Some(&sha(&bytes)),
        "Installer payload differs from portable tools"
      );
    }
  }
  ensure!(found == allowed, "Unexpected self-installer inventory");
  Ok(())
}
fn accept(archive: &Path, installer: &Path, expected: &Inventory, markers: &BTreeSet<String>) -> Result<()> {
  let entries = archive_entries(archive)?;
  ensure!(
    entries.iter().map(|(n, b, _)| (n.clone(), sha(b))).collect::<Inventory>() == *expected,
    "Portable archive inventory mismatch"
  );
  let temporary = tempfile::tempdir()?;
  let temporary = fs::canonicalize(temporary.path())?;
  let portable = temporary.join("portable tools");
  fs::create_dir(&portable)?;
  for (name, bytes, mode) in entries {
    scan(&bytes, &name, markers)?;
    let path = portable.join(&name);
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, bytes)?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(&path, fs::Permissions::from_mode(mode))?;
    }
    #[cfg(not(unix))]
    let _ = mode;
  }
  query_tools(&portable)?;
  inspect_installer(installer, expected, markers)?;
  let root = temporary.join("installed tools");
  let commands = temporary.join("commands");
  run(
    Command::new(installer)
      .arg("--extract-to")
      .arg(temporary.join("retained source"))
      .args(["--", "--non-interactive", "--install-dir"])
      .arg(&root)
      .arg("--bin-dir")
      .arg(&commands)
      .args(["--set", "add-to-path=false"]),
  )?;
  let query: Value = serde_json::from_str(&capture(
    Command::new(commands.join(super::installer::executable("doinstall-builder"))).arg("--print-tool-paths"),
  )?)?;
  let installed = PathBuf::from(query["Installer"].as_str().context("Missing installed runtime")?)
    .parent()
    .unwrap()
    .to_path_buf();
  ensure!(
    fs::canonicalize(&installed)?.starts_with(fs::canonicalize(&root)?),
    "Installed tools escaped temporary root"
  );
  query_tools(&installed)?;
  for (name, digest) in expected {
    ensure!(sha(&fs::read(installed.join(name))?) == *digest, "Installed payload mismatch");
  }
  let payload = temporary.join("app payload");
  fs::create_dir(&payload)?;
  let example = super::installer::executable("example");
  fs::copy(portable.join(super::installer::executable("doinstall")), payload.join(&example))?;
  // JSON is a YAML subset consumed by the ordinary public builder. This second
  // application proves the extracted matching tools work as a consumer bundle.
  write_json(
    &temporary.join("app.yaml"),
    &json!({"schema":1,"application":{"id":"release-smoke","name":"Release smoke","version":"1.0.0"},"platform":std::env::consts::OS,"architecture":"x64","payload":payload,"runtime":{"interactive":false,"commands":[{"name":"release-smoke","executable":example}]}}),
  )?;
  run(
    Command::new(portable.join(super::installer::executable("doinstall-builder")))
      .arg("--config")
      .arg(temporary.join("app.yaml"))
      .arg("--installer")
      .arg(portable.join(super::installer::executable("doinstall")))
      .arg("--launcher-template")
      .arg(portable.join(super::installer::executable("doinstall-launcher")))
      .arg("--output")
      .arg(temporary.join("delivery"))
      .args(["--self-extracting", "--result-file"])
      .arg(temporary.join("app-result.json")),
  )?;
  let app: Value = serde_json::from_slice(&fs::read(temporary.join("app-result.json"))?)?;
  run(
    Command::new(app["FilePath"].as_str().context("Missing FilePath")?)
      .arg("--extract-to")
      .arg(temporary.join("app source"))
      .args(["--", "--non-interactive", "--install-dir"])
      .arg(temporary.join("app install"))
      .arg("--bin-dir")
      .arg(temporary.join("app commands"))
      .args(["--set", "add-to-path=false"]),
  )?;
  run(
    Command::new(
      temporary
        .join("app commands")
        .join(super::installer::executable("release-smoke")),
    )
    .arg("--help"),
  )?;
  for install in [temporary.join("app install"), root.clone()] {
    run(
      Command::new(install.join("installer").join(super::installer::executable("installer")))
        .args(["uninstall", "--non-interactive"]),
    )?;
  }
  ensure!(
    !root.exists() && !commands.join(super::installer::executable("doinstall-builder")).exists(),
    "Uninstall left active temporary tools"
  );
  Ok(())
}
fn destination(path: &Path) -> Result<PathBuf> {
  let path = std::path::absolute(path)?;
  ensure!(
    !path.components().any(|v| matches!(v, std::path::Component::ParentDir)),
    "Output path contains parent traversal"
  );
  let mut existing = path.as_path();
  let mut suffix = vec![];
  while !existing.exists() {
    suffix.push(existing.file_name().context("Output has no existing ancestor")?.to_owned());
    existing = existing.parent().context("Output has no existing ancestor")?;
  }
  let mut resolved = fs::canonicalize(existing)?;
  for part in suffix.into_iter().rev() {
    resolved.push(part);
  }
  Ok(resolved)
}
fn sums(output: &Path) -> Result<()> {
  let hashes = inventory(output)?;
  fs::write(
    output.join("SHA256SUMS"),
    hashes
      .into_iter()
      .filter(|(name, _)| name != "SHA256SUMS")
      .map(|(name, hash)| format!("{hash}  {name}\n"))
      .collect::<String>(),
  )?;
  Ok(())
}
pub fn build(output: &Path, tag: Option<&str>) -> Result<()> {
  let value = version(tag)?;
  ensure!(
    (cfg!(windows) || cfg!(target_os = "linux")) && cfg!(target_arch = "x86_64"),
    "Release build requires native Windows/Linux x64"
  );
  if cfg!(target_os = "linux") {
    let release = fs::read_to_string("/etc/os-release")?;
    ensure!(
      release.lines().any(|v| v == "ID=ubuntu")
        && release.lines().any(|v| v == "VERSION_ID=\"22.04\"")
        && capture(Command::new("getconf").arg("GNU_LIBC_VERSION"))? == "glibc 2.35",
      "Linux release requires Ubuntu 22.04 / glibc 2.35"
    );
  }
  let output = destination(output)?;
  let root = fs::canonicalize(super::root())?;
  ensure!(
    !output.starts_with(&root) && !root.starts_with(&output) && !output.exists(),
    "Release output must be fresh and outside source"
  );
  fs::create_dir_all(output.parent().context("Missing output parent")?)?;
  let rustc = capture(Command::new("rustc").arg("--verbose").arg("--version"))?;
  ensure!(rustc.lines().any(|v| v == "release: 1.95.0"), "Wrong Rust toolchain");
  for key in ["target_directory", "build_directory"] {
    if let Some(path) = super::metadata()?[key].as_str() {
      ensure!(!destination(Path::new(path))?.starts_with(&root), "Cargo output inside source");
    }
  }
  build_tools()?;
  fs::create_dir(&output)?;
  let work = tempfile::tempdir()?;
  let stage = work.path().join("tools");
  fs::create_dir(&stage)?;
  let markers = markers()?;
  #[cfg(unix)]
  let glibc_pattern = Regex::new(r"GLIBC_(\d+)\.(\d+)")?;
  for name in TOOLS {
    let name = super::installer::executable(name);
    let from = super::target()?.join("release").join(&name);
    fs::copy(&from, stage.join(&name))?;
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      ensure!(
        fs::metadata(&from)?.permissions().mode() & 0o111 != 0,
        "Missing executable permission"
      );
      let bytes = fs::read(&from)?;
      let text = String::from_utf8_lossy(&bytes);
      let versions: Vec<(u32, u32)> = glibc_pattern
        .captures_iter(&text)
        .map(|c| (c[1].parse().unwrap(), c[2].parse().unwrap()))
        .collect();
      ensure!(
        !versions.is_empty() && versions.iter().all(|v| *v <= (2, 35)),
        "Tool exceeds glibc 2.35 baseline"
      );
    }
  }
  fs::copy(super::root().join("LICENSE.md"), stage.join("LICENSE.md"))?;
  fs::copy(super::root().join("packaging/PORTABLE-README.md"), stage.join("README.md"))?;
  licenses(&stage)?;
  let identity = capture(Command::new(stage.join(super::installer::executable("doinstall"))).arg("--version"))?;
  let display_version = identity.split_whitespace().nth(1).context("Missing display version")?;
  let built_utc = identity
    .split("(built ")
    .nth(1)
    .and_then(|v| v.strip_suffix(')'))
    .context("Missing UTC build time")?;
  ensure!(display_version.starts_with(&format!("{value}+")), "Tool/Cargo version mismatch");
  let metadata = json!({"schema":1,"version":value,"tag":format!("v{value}"),"display_version":display_version,"built_utc":built_utc,"source_commit":capture(Command::new("git").current_dir(super::root()).args(["rev-parse","HEAD"]))?,"source_dirty":!capture(Command::new("git").current_dir(super::root()).args(["status","--porcelain","--untracked-files=no"]))?.is_empty(),"rustc":rustc,"cargo":capture(Command::new("cargo").arg("--version"))?,"platform":std::env::consts::OS,"architecture":"x64","target":if cfg!(windows){"x86_64-pc-windows-msvc"}else{"x86_64-unknown-linux-gnu"},"compatibility_baseline":if cfg!(windows){"Windows Server 2022 x64"}else{"Ubuntu 22.04 x64, glibc 2.35"},"files":inventory(&stage)?});
  write_json(&stage.join("RELEASE.json"), &metadata)?;
  let expected = inventory(&stage)?;
  let source = super::installer::assemble(&stage, &work.path().join("installers"), &value, Some(&stage))?;
  let stem = format!("doinstall-v{value}-{}-x64", std::env::consts::OS);
  let installer = output.join(format!("{stem}-setup{}", if cfg!(windows) { ".exe" } else { "" }));
  fs::copy(source, &installer)?;
  let archive = output.join(format!("{stem}-tools.{}", if cfg!(windows) { "zip" } else { "tar.gz" }));
  archive_tools(&stage, &archive, cfg!(windows))?;
  accept(&archive, &installer, &expected, &markers)?;
  write_json(&output.join(format!("{stem}-metadata.json")), &metadata)?;
  sums(&output)?;
  println!("Native release acceptance passed: {}", output.display());
  Ok(())
}
pub fn collect(source: &Path, output: &Path, tag: Option<&str>, commit: &str, publishing: bool) -> Result<()> {
  let value = version(tag)?;
  let output = destination(output)?;
  let root = fs::canonicalize(super::root())?;
  ensure!(
    !output.starts_with(&root) && !root.starts_with(&output),
    "Collection output must be outside source"
  );
  ensure!(!output.exists(), "Collection output must be fresh");
  let mut assets = BTreeMap::new();
  let mut metadata = vec![];
  for (platform, archive, suffix) in [("windows", "zip", ".exe"), ("linux", "tar.gz", "")] {
    let stem = format!("doinstall-v{value}-{platform}-x64");
    let directory = source.join(format!("native-{platform}"));
    let names = BTreeSet::from([
      format!("{stem}-tools.{archive}"),
      format!("{stem}-setup{suffix}"),
      format!("{stem}-metadata.json"),
    ]);
    ensure!(
      fs::read_dir(&directory)?
        .map(|v| Ok(v?.file_name().to_string_lossy().into_owned()))
        .collect::<Result<BTreeSet<_>>>()?
        == names.iter().cloned().chain(["SHA256SUMS".into()]).collect(),
      "Unexpected native artifact inventory"
    );
    let mut hashes = BTreeMap::new();
    for line in fs::read_to_string(directory.join("SHA256SUMS"))?.lines() {
      let (hash, name) = line.split_once("  ").context("Invalid checksum line")?;
      ensure!(
        hash.len() == 64
          && hash.bytes().all(|b| b.is_ascii_hexdigit())
          && hashes.insert(name.to_owned(), hash.to_owned()).is_none(),
        "Invalid checksum inventory"
      );
    }
    ensure!(
      hashes.keys().cloned().collect::<BTreeSet<_>>() == names,
      "Invalid native checksum inventory"
    );
    for name in &names {
      let path = directory.join(name);
      ensure!(
        fs::symlink_metadata(&path)?.file_type().is_file() && sha(&fs::read(&path)?) == hashes[name],
        "Native checksum mismatch"
      );
      assets.insert(name.clone(), path);
    }
    let info: Value = serde_json::from_slice(&fs::read(directory.join(format!("{stem}-metadata.json")))?)?;
    ensure!(
      info["version"] == value
        && info["tag"] == format!("v{value}")
        && info["source_commit"] == commit
        && info["platform"] == platform
        && info["architecture"] == "x64",
      "Native metadata mismatch"
    );
    if publishing {
      ensure!(info["source_dirty"] == false, "Cannot publish dirty-source artifacts");
    }
    metadata.push(info);
  }
  fs::create_dir_all(&output)?;
  for (name, path) in assets {
    fs::copy(path, output.join(name))?;
  }
  write_json(
    &output.join("RELEASE-METADATA.json"),
    &json!({"schema":1,"tag":format!("v{value}"),"source_commit":commit,"platforms":metadata}),
  )?;
  sums(&output)
}
pub fn cli(args: &[String]) -> Result<()> {
  let action = args.first().map(String::as_str).unwrap_or("help");
  let mut options = BTreeMap::new();
  let mut publishing = false;
  let mut iterator = args[1.min(args.len())..].iter();
  while let Some(key) = iterator.next() {
    if key == "--publishing" {
      ensure!(!publishing, "Duplicate publishing option");
      publishing = true;
    } else {
      ensure!(
        ["--output", "--source", "--tag", "--commit"].contains(&key.as_str()) && !options.contains_key(key),
        "Unexpected release argument: {key}"
      );
      options.insert(key.clone(), iterator.next().context("Missing option value")?.clone());
    }
  }
  let tag = options.get("--tag").map(String::as_str);
  match action {
    "version" => { println!("{}",version(tag)?); Ok(()) }
    "build" => {
      let default = super::target()?.join("release-deliveries").join(format!("{}-{}", std::env::consts::OS, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
      build(options.get("--output").map(PathBuf::from).unwrap_or(default).as_path(), tag)
    },
    "collect" => collect(Path::new(options.get("--source").context("--source is required")?),Path::new(options.get("--output").context("--output is required")?),tag,options.get("--commit").context("--commit is required")?,publishing),
    _ => bail!("Use release version [--tag TAG], release build --output DIR, or release collect --source DIR --output DIR --commit SHA [--tag TAG --publishing]"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn tags_require_exact_stable_cargo_version() {
    let version = version(None).unwrap();
    assert_eq!(super::version(Some(&format!("v{version}"))).unwrap(), version);
    for tag in [
      "0.1.0",
      "v9.9.9",
      "v01.0.0",
      "v0.1.0-preview",
      "v0.1.2610.2014",
    ] {
      assert!(super::version(Some(tag)).is_err());
    }
  }
  #[test]
  fn scan_rejects_private_content_without_echoing_it() {
    for text in [
      r"C:\Users\private\project",
      "/home/private/project",
      "-----BEGIN OPENSSH PRIVATE KEY-----",
      "GITHUB_TOKEN=private-value",
      "/custom/build/private.rs",
      "github_pat_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
    ] {
      for bytes in [
        text.as_bytes().to_vec(),
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
      ] {
        let error = scan(&bytes, "fixture", &BTreeSet::from(["/custom/build".into()]))
          .unwrap_err()
          .to_string();
        assert!(!error.contains(text));
      }
    }
    scan(
      b"/doinstall/src/main.rs /cargo/registry /rustc/public",
      "public",
      &BTreeSet::new(),
    )
    .unwrap();
  }
  #[test]
  fn archives_refuse_unsafe_paths_duplicates_and_links() {
    for name in [
      "../escape",
      "/escape",
      r"a\escape",
      "a/./escape",
      "C:/escape",
      "a//escape",
    ] {
      assert!(validate_entries(&[(name.into(), vec![], 0)]).is_err());
    }
    assert!(validate_entries(&[("same".into(), vec![], 0), ("same".into(), vec![], 0)]).is_err());
    let mut zip = zip::ZipWriter::new(Cursor::new(vec![]));
    zip
      .add_symlink("link", "target", zip::write::SimpleFileOptions::default())
      .unwrap();
    assert!(zip_entries(zip.finish().unwrap()).is_err());
  }
  #[test]
  fn portable_roundtrip_retains_bytes_and_linux_modes() {
    let temp = tempfile::tempdir().unwrap();
    let stage = temp.path().join("stage");
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("tool"), b"native fixture").unwrap();
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      fs::set_permissions(stage.join("tool"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    for (extension, windows) in [("zip", true), ("tar.gz", false)] {
      let archive = temp.path().join(format!("tools.{extension}"));
      archive_tools(&stage, &archive, windows).unwrap();
      let entries = archive_entries(&archive).unwrap();
      assert_eq!(entries.len(), 1);
      assert_eq!(entries[0].0, "tool");
      assert_eq!(entries[0].1, b"native fixture");
      if !windows {
        assert_eq!(entries[0].2 & 0o777, 0o755);
      }
    }
  }
  #[test]
  fn collection_refuses_wrong_commit_dirty_tampered_and_missing_assets() {
    let temp = tempfile::tempdir().unwrap();
    let value = version(None).unwrap();
    let commit = "a".repeat(40);
    for (system, archive, suffix) in [("windows", "zip", ".exe"), ("linux", "tar.gz", "")] {
      let directory = temp.path().join(format!("native-{system}"));
      fs::create_dir(&directory).unwrap();
      let stem = format!("doinstall-v{value}-{system}-x64");
      fs::write(directory.join(format!("{stem}-tools.{archive}")), b"archive fixture").unwrap();
      fs::write(directory.join(format!("{stem}-setup{suffix}")), b"installer fixture").unwrap();
      write_json(&directory.join(format!("{stem}-metadata.json")),&json!({"version":value,"tag":format!("v{value}"),"source_commit":commit,"source_dirty":false,"platform":system,"architecture":"x64"})).unwrap();
      sums(&directory).unwrap();
    }
    collect(temp.path(), &temp.path().join("good"), None, &commit, true).unwrap();
    assert_eq!(
      fs::read_to_string(temp.path().join("good/SHA256SUMS")).unwrap().lines().count(),
      7
    );
    assert!(collect(temp.path(), &temp.path().join("wrong"), None, &"b".repeat(40), true).is_err());
    let metadata = temp
      .path()
      .join(format!("native-linux/doinstall-v{value}-linux-x64-metadata.json"));
    let mut info: Value = serde_json::from_slice(&fs::read(&metadata).unwrap()).unwrap();
    info["source_dirty"] = true.into();
    write_json(&metadata, &info).unwrap();
    sums(metadata.parent().unwrap()).unwrap();
    assert!(collect(temp.path(), &temp.path().join("dirty"), None, &commit, true).is_err());
    fs::write(&metadata, b"tampered").unwrap();
    assert!(collect(temp.path(), &temp.path().join("tampered"), None, &commit, false).is_err());
    fs::remove_file(&metadata).unwrap();
    assert!(collect(temp.path(), &temp.path().join("missing"), None, &commit, false).is_err());
  }
  #[test]
  fn malformed_installer_is_rejected_before_execution() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("setup");
    fs::write(&path, [0; 40]).unwrap();
    assert!(inspect_installer(&path, &BTreeMap::new(), &BTreeSet::new()).is_err());
  }
}
