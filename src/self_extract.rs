//! A versioned ZIP container attached to a precompiled native console launcher.
//! No application source, compiler, compatibility parser, or migration is involved.
use crate::{binary, files, model::FileRecord};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
  collections::{BTreeMap, BTreeSet},
  fs::{self, File, OpenOptions},
  io::{Read, Seek, SeekFrom, Write},
  path::Path,
};

pub const TEMPLATE_MARKER: &[u8; 32] = b"DOINSTALL-LAUNCHER-TEMPLATE-v1\0\0";
pub const TEMPLATE_ROLE_A: u64 = 0x4d6c_708f_0321_9a55;
pub const TEMPLATE_ROLE_B: u64 = 0x927a_f438_a381_c60e;
const MAGIC: &[u8; 16] = b"DOINSTALL-SE-v1!";
const FOOTER_SIZE: u64 = 40;
const MAX_MANIFEST: u64 = 16 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Manifest {
  pub schema: u32,
  pub platform: String,
  pub architecture: String,
  pub installer: String,
  pub fingerprint: String,
  pub template_hash: String,
  pub archive_hash: String,
  pub files: BTreeMap<String, FileRecord>,
  pub directories: BTreeMap<String, u32>,
}
pub struct Bundle {
  pub manifest: Manifest,
  archive: Section,
}

/// ZIP offsets are relative to the archive, not the native executable before it.
/// Keep reads/seeks inside this section, including on corrupt central directories.
struct Section {
  file: File,
  start: u64,
  length: u64,
  position: u64,
}
impl Read for Section {
  fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
    let remaining = self.length - self.position;
    let limit = buffer.len().min(usize::try_from(remaining).unwrap_or(usize::MAX));
    self.file.seek(SeekFrom::Start(self.start + self.position))?;
    let count = self.file.read(&mut buffer[..limit])?;
    self.position += count as u64;
    Ok(count)
  }
}
impl Seek for Section {
  fn seek(&mut self, from: SeekFrom) -> std::io::Result<u64> {
    let position = match from {
      SeekFrom::Start(value) => value as i128,
      SeekFrom::Current(value) => self.position as i128 + value as i128,
      SeekFrom::End(value) => self.length as i128 + value as i128,
    };
    if position < 0 || position > self.length as i128 {
      return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Seek outside embedded ZIP",
      ));
    }
    self.position = position as u64;
    Ok(self.position)
  }
}
fn hash_reader(reader: &mut impl Read) -> Result<String> {
  let mut digest = Sha256::new();
  let mut buffer = [0; 65536];
  loop {
    let count = reader.read(&mut buffer)?;
    if count == 0 {
      break;
    }
    digest.update(&buffer[..count]);
  }
  Ok(format!("{:x}", digest.finalize()))
}
pub fn template_hash(path: &Path) -> Result<String> {
  Ok(files::hash(&template_bytes(path)?))
}
fn template_bytes(path: &Path) -> Result<Vec<u8>> {
  let (length, signature) = binary::signature(path)?;
  let mut bytes = Vec::new();
  File::open(path)?.take(length).read_to_end(&mut bytes)?;
  ensure!(
    bytes.windows(48).any(|record| &record[..32] == TEMPLATE_MARKER
      && u64::from_le_bytes(record[32..40].try_into().unwrap()) == TEMPLATE_ROLE_A
      && u64::from_le_bytes(record[40..48].try_into().unwrap()) == TEMPLATE_ROLE_B),
    "Not a doinstall launcher template"
  );
  if let Some(field) = signature {
    bytes[field as usize..field as usize + 8].fill(0);
  }
  Ok(bytes)
}
fn directories(root: &Path, directory: &Path, result: &mut BTreeMap<String, u32>, platform: &str) -> Result<()> {
  files::safe_path(directory)?;
  for item in fs::read_dir(directory)? {
    let path = item?.path();
    files::safe_path(&path)?;
    if path.is_dir() {
      let name = path
        .strip_prefix(root)?
        .to_str()
        .context("Non UTF-8 directory")?
        .replace('\\', "/");
      #[cfg(unix)]
      let mode = {
        use std::os::unix::fs::PermissionsExt;
        if platform == "windows" {
          0
        } else {
          fs::metadata(&path)?.permissions().mode() & 0o777
        }
      };
      #[cfg(windows)]
      let mode = if platform == "windows" { 0 } else { 0o755 };
      result.insert(name, mode);
      directories(root, &path, result, platform)?;
    }
  }
  Ok(())
}
pub fn assemble(
  template: &Path,
  directory: &Path,
  installer: &str,
  platform: &str,
  architecture: &str,
  fingerprint: &str,
  output: &Path,
) -> Result<()> {
  binary::validate_launcher(template, platform, architecture)?;
  // Snapshot once. An input signature covers the template only; its pointer
  // and blob are removed before the completed deliverable is signed.
  let native = template_bytes(template)?;
  let template_hash = files::hash(&native);
  let mut inventory = files::delivery_inventory(directory)?;
  if platform != "windows" {
    // A Windows producer cannot store Unix permissions on the staged installer.
    inventory.get_mut(installer).context("Missing staged installer")?.mode = 0o755;
    for (name, record) in &mut inventory {
      if record.mode == 0 {
        record.mode = if name == installer { 0o755 } else { 0o644 };
      }
    }
  }
  files::safe_path(output)?;
  let mut file = OpenOptions::new().write(true).read(true).create_new(true).open(output)?;
  file.write_all(&native)?;
  let archive_start = file.stream_position()?;
  // Build the ZIP separately so all its central-directory offsets start at zero.
  let mut archive_file = tempfile::tempfile()?;
  let mut zip = zip::ZipWriter::new(&mut archive_file);
  let mut directory_modes = BTreeMap::new();
  directories(directory, directory, &mut directory_modes, platform)?;
  for (name, mode) in &directory_modes {
    zip.add_directory(
      format!("{name}/"),
      zip::write::SimpleFileOptions::default().unix_permissions(if *mode == 0 { 0o755 } else { *mode }),
    )?;
  }
  for (name, record) in &inventory {
    zip.start_file(
      name,
      zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(if record.mode == 0 { 0o644 } else { record.mode }),
    )?;
    std::io::copy(&mut File::open(directory.join(name))?, &mut zip)?;
  }
  zip.finish()?;
  archive_file.rewind()?;
  let archive_hash = hash_reader(&mut archive_file)?;
  archive_file.rewind()?;
  let archive_length = std::io::copy(&mut archive_file, &mut file)?;
  let manifest = Manifest {
    schema: 1,
    platform: platform.into(),
    architecture: architecture.into(),
    installer: installer.into(),
    fingerprint: fingerprint.into(),
    template_hash,
    archive_hash,
    files: inventory,
    directories: directory_modes,
  };
  let mut json = serde_json::to_vec(&manifest)?;
  ensure!(json.len() as u64 <= MAX_MANIFEST, "Self-extracting manifest is too large");
  // Authenticode and Mach-O signers align their trailer. Align the footer now
  // so signing does not introduce a gap between the footer and signature.
  while !(archive_start + archive_length + json.len() as u64 + FOOTER_SIZE).is_multiple_of(16) {
    json.push(b' ');
  }
  file.write_all(&json)?;
  for value in [archive_start, archive_length, json.len() as u64] {
    file.write_all(&value.to_le_bytes())?;
  }
  file.write_all(MAGIC)?;
  file.sync_all()?;
  drop(file);
  if platform == "macos" {
    binary::extend_macho_linkedit(output, architecture)?;
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(output, fs::Permissions::from_mode(0o755))?;
  }
  #[cfg(target_os = "macos")]
  if platform == "macos" {
    // Apple Silicon requires a valid code signature after changing Mach-O bytes.
    // Ad-hoc signing is supplied by macOS and requires no Rust SDK or identity.
    let status = std::process::Command::new("/usr/bin/codesign")
      .args(["--force", "--sign", "-"])
      .arg(output)
      .status()?;
    ensure!(status.success(), "Cannot ad-hoc sign the completed macOS launcher");
  }
  // Verify the exact deliverable, including any platform signing edits.
  Bundle::open(output)?;
  Ok(())
}

impl Bundle {
  pub fn open(path: &Path) -> Result<Self> {
    let (end, _) = binary::signature(path)?;
    ensure!(end >= FOOTER_SIZE, "Missing self-extracting footer");
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(end - FOOTER_SIZE))?;
    let mut footer = [0; FOOTER_SIZE as usize];
    file.read_exact(&mut footer)?;
    ensure!(&footer[24..] == MAGIC, "Unsupported or missing self-extracting format");
    let start = u64::from_le_bytes(footer[..8].try_into().unwrap());
    let length = u64::from_le_bytes(footer[8..16].try_into().unwrap());
    let json_length = u64::from_le_bytes(footer[16..24].try_into().unwrap());
    ensure!(
      start >= 64
        && length > 0
        && json_length > 0
        && json_length <= MAX_MANIFEST
        && start.checked_add(length).and_then(|v| v.checked_add(json_length)) == Some(end - FOOTER_SIZE),
      "Invalid self-extracting bounds"
    );
    file.seek(SeekFrom::Start(start + length))?;
    let mut json = vec![0; json_length as usize];
    file.read_exact(&mut json)?;
    let manifest: Manifest = serde_json::from_slice(&json).context("Invalid self-extracting manifest")?;
    ensure!(manifest.schema == 1, "Unsupported self-extracting schema");
    ensure!(
      ["windows", "linux", "macos"].contains(&manifest.platform.as_str())
        && ["x64", "arm64"].contains(&manifest.architecture.as_str()),
      "Unsupported self-extracting target"
    );
    files::name(&manifest.installer)?;
    ensure!(
      manifest.files.contains_key(&manifest.installer)
        && manifest.files.contains_key(&files::installer_yaml_name(&manifest.installer)),
      "Embedded installer or matching YAML is missing"
    );
    let mut names = BTreeSet::new();
    for (name, record) in &manifest.files {
      files::relative(name)?;
      ensure!(names.insert(name.to_ascii_lowercase()), "Duplicate embedded path: {name}");
      ensure!(
        record.sha256.len() == 64 && record.sha256.chars().all(|c| c.is_ascii_hexdigit()) && record.mode & !0o777 == 0,
        "Invalid embedded file record: {name}"
      );
    }
    for (name, mode) in &manifest.directories {
      files::relative(name)?;
      ensure!(
        names.insert(name.to_ascii_lowercase()) && mode & !0o777 == 0,
        "Invalid or duplicate embedded directory: {name}"
      );
    }
    binary::validate_launcher(path, &manifest.platform, &manifest.architecture)?;
    let mut archive = Section {
      file,
      start,
      length,
      position: 0,
    };
    ensure!(
      hash_reader(&mut archive)? == manifest.archive_hash,
      "Embedded ZIP hash mismatch"
    );
    archive.rewind()?;
    Ok(Self { manifest, archive })
  }
  pub fn extract(mut self, target: &Path) -> Result<()> {
    files::safe_path(target)?;
    ensure!(
      target.is_dir() && fs::read_dir(target)?.next().is_none(),
      "Extraction directory must be empty"
    );
    let mut archive = zip::ZipArchive::new(&mut self.archive)?;
    ensure!(
      archive.len() == self.manifest.files.len() + self.manifest.directories.len(),
      "Embedded ZIP inventory mismatch"
    );
    let mut seen = BTreeSet::new();
    let file_names: BTreeSet<_> = self.manifest.files.keys().map(|name| name.to_ascii_lowercase()).collect();
    // Validate the whole central directory before creating files. No links,
    // aliases, duplicate entries, or unrecorded files are permitted.
    for index in 0..archive.len() {
      let entry = archive.by_index(index)?;
      let name = if entry.is_dir() {
        entry.name().strip_suffix('/').unwrap_or(entry.name())
      } else {
        entry.name()
      };
      files::relative(name)?;
      ensure!(seen.insert(name.to_ascii_lowercase()), "Duplicate embedded ZIP path");
      ensure!(
        if entry.is_dir() {
          self.manifest.directories.contains_key(name)
        } else {
          self.manifest.files.contains_key(name)
        },
        "Unrecorded embedded ZIP path"
      );
      ensure!(!entry.is_symlink(), "Embedded ZIP links are unsupported");
      let mut parent = Path::new(name).parent();
      while let Some(path) = parent.filter(|p| !p.as_os_str().is_empty()) {
        ensure!(
          !file_names.contains(&path.to_string_lossy().to_ascii_lowercase()),
          "Embedded file/directory collision"
        );
        parent = path.parent();
      }
    }
    for index in 0..archive.len() {
      let mut entry = archive.by_index(index)?;
      if entry.is_dir() {
        let path = target.join(files::relative(entry.name().strip_suffix('/').unwrap_or(entry.name()))?);
        files::safe_path(&path)?;
        fs::create_dir_all(path)?;
        continue;
      }
      let record = &self.manifest.files[entry.name()];
      let path = target.join(files::relative(entry.name())?);
      files::safe_path(&path)?;
      fs::create_dir_all(path.parent().context("Embedded file needs a parent")?)?;
      files::safe_path(&path)?;
      let mut file = OpenOptions::new().write(true).create_new(true).open(&path)?;
      std::io::copy(&mut entry, &mut file)?;
      file.sync_all()?;
      drop(file);
      ensure!(
        files::file_hash(&path)? == record.sha256,
        "Embedded file hash mismatch: {}",
        path.display()
      );
      #[cfg(unix)]
      {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(record.mode))?;
      }
    }
    #[cfg(unix)]
    for (name, mode) in self.manifest.directories.iter().rev() {
      use std::os::unix::fs::PermissionsExt;
      let path = target.join(name);
      files::safe_path(&path)?;
      fs::set_permissions(path, fs::Permissions::from_mode(*mode))?;
    }
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  fn fixture(platform: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let temporary = tempfile::tempdir().unwrap();
    let template = temporary.path().join("template");
    let mut bytes = vec![0; 512];
    match platform {
      "windows" => {
        bytes[..2].copy_from_slice(b"MZ");
        bytes[60..64].copy_from_slice(&64u32.to_le_bytes());
        bytes[64..68].copy_from_slice(b"PE\0\0");
        bytes[68..70].copy_from_slice(&0x8664u16.to_le_bytes());
        bytes[84..86].copy_from_slice(&240u16.to_le_bytes());
        bytes[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
        bytes[156..158].copy_from_slice(&3u16.to_le_bytes());
      }
      "macos" => {
        bytes[..4].copy_from_slice(b"\xcf\xfa\xed\xfe");
        bytes[4..8].copy_from_slice(&0x01000007u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&16u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x1du32.to_le_bytes());
        bytes[36..40].copy_from_slice(&16u32.to_le_bytes());
      }
      _ => {
        bytes[..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18] = 62;
      }
    }
    bytes[320..320 + TEMPLATE_MARKER.len()].copy_from_slice(TEMPLATE_MARKER);
    bytes[352..360].copy_from_slice(&TEMPLATE_ROLE_A.to_le_bytes());
    bytes[360..368].copy_from_slice(&TEMPLATE_ROLE_B.to_le_bytes());
    fs::write(&template, bytes).unwrap();
    let directory = temporary.path().join("package");
    fs::create_dir(&directory).unwrap();
    let installer = if platform == "windows" {
      "setup.exe"
    } else {
      "setup-v1.2.3"
    };
    fs::copy(&template, directory.join(installer)).unwrap();
    fs::write(directory.join(files::installer_yaml_name(installer)), "schema: 1\n").unwrap();
    fs::create_dir(directory.join("payload")).unwrap();
    fs::write(directory.join("payload/data"), "original").unwrap();
    let output = temporary.path().join("bundle");
    assemble(&template, &directory, installer, platform, "x64", "fingerprint", &output).unwrap();
    (temporary, output)
  }
  fn rewrite(path: &Path, change: impl FnOnce(&mut Manifest)) {
    let mut bytes = fs::read(path).unwrap();
    let end = bytes.len();
    let start = u64::from_le_bytes(bytes[end - 40..end - 32].try_into().unwrap());
    let length = u64::from_le_bytes(bytes[end - 32..end - 24].try_into().unwrap());
    let mut manifest = Bundle::open(path).unwrap().manifest;
    change(&mut manifest);
    let json = serde_json::to_vec(&manifest).unwrap();
    bytes.truncate((start + length) as usize);
    bytes.extend_from_slice(&json);
    for value in [start, length, json.len() as u64] {
      bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(MAGIC);
    fs::write(path, bytes).unwrap();
  }
  #[test]
  fn unsupported_schema_paths_modes_and_missing_yaml_are_refused() {
    for kind in 0..5 {
      let (_temporary, output) = fixture("linux");
      rewrite(&output, |manifest| match kind {
        0 => manifest.schema = 2,
        1 => {
          let record = manifest.files.values().next().unwrap().clone();
          manifest.files.insert("../outside".into(), record);
        }
        2 => manifest.files.values_mut().next().unwrap().mode = 0o4755,
        3 => {
          manifest.files.remove("setup-v1.2.3.yaml");
        }
        _ => manifest.installer = "missing".into(),
      });
      assert!(Bundle::open(&output).is_err());
    }
  }
  #[test]
  fn corrupt_footer_bounds_and_file_hashes_do_not_succeed() {
    for kind in 0..3 {
      let (temporary, output) = fixture("linux");
      let mut bytes = fs::read(&output).unwrap();
      let end = bytes.len();
      match kind {
        0 => bytes[end - 40..end - 32].copy_from_slice(&u64::MAX.to_le_bytes()),
        1 => bytes[end - 24..end - 16].copy_from_slice(&(MAX_MANIFEST + 1).to_le_bytes()),
        _ => bytes[end - 1] ^= 1,
      }
      fs::write(&output, bytes).unwrap();
      assert!(Bundle::open(&output).is_err());
      assert!(!temporary.path().join("outside").exists());
    }
    let (temporary, output) = fixture("linux");
    rewrite(&output, |manifest| {
      manifest.files.get_mut("payload/data").unwrap().sha256 = "0".repeat(64)
    });
    let target = temporary.path().join("extract");
    fs::create_dir(&target).unwrap();
    assert!(Bundle::open(&output).unwrap().extract(&target).is_err());
  }
  #[test]
  fn signature_trailers_leave_the_versioned_footer_readable() {
    for platform in ["windows", "macos"] {
      let (_temporary, output) = fixture(if platform == "macos" {
        "linux"
      } else {
        platform
      });
      if platform == "macos" {
        // Exercise signature layout without signing a synthetic Mach-O.
        // Native macOS execution remains separate acceptance.
        rewrite(&output, |manifest| manifest.platform = "macos".into());
        let mut bytes = fs::read(&output).unwrap();
        bytes[..64].fill(0);
        bytes[..4].copy_from_slice(b"\xcf\xfa\xed\xfe");
        bytes[4..8].copy_from_slice(&0x01000007u32.to_le_bytes());
        bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
        bytes[20..24].copy_from_slice(&16u32.to_le_bytes());
        bytes[32..36].copy_from_slice(&0x1du32.to_le_bytes());
        bytes[36..40].copy_from_slice(&16u32.to_le_bytes());
        fs::write(&output, bytes).unwrap();
      }
      let mut bytes = fs::read(&output).unwrap();
      let length = bytes.len() as u32;
      let field = if platform == "windows" { 232 } else { 40 };
      bytes[field..field + 4].copy_from_slice(&length.to_le_bytes());
      bytes[field + 4..field + 8].copy_from_slice(&16u32.to_le_bytes());
      bytes.extend_from_slice(&[0; 16]);
      fs::write(&output, &bytes).unwrap();
      assert!(Bundle::open(&output).is_ok());
      bytes[field..field + 4].copy_from_slice(&u32::MAX.to_le_bytes());
      fs::write(&output, bytes).unwrap();
      assert!(Bundle::open(&output).is_err());
    }
  }
  #[test]
  fn hash_checked_zip_still_rejects_links_duplicates_and_traversal() {
    for kind in 0..3 {
      let (temporary, output) = fixture("linux");
      let mut bytes = fs::read(&output).unwrap();
      let end = bytes.len();
      let start = u64::from_le_bytes(bytes[end - 40..end - 32].try_into().unwrap());
      let mut manifest = Bundle::open(&output).unwrap().manifest;
      let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
      for name in manifest.directories.keys() {
        zip
          .add_directory(format!("{name}/"), zip::write::SimpleFileOptions::default())
          .unwrap();
      }
      for name in manifest.files.keys() {
        if kind == 0 && name == "payload/data" {
          zip
            .add_symlink(name, "../../outside", zip::write::SimpleFileOptions::default())
            .unwrap();
        } else {
          let entry = if kind == 2 && name == "payload/data" {
            "../outside"
          } else {
            name
          };
          zip.start_file(entry, zip::write::SimpleFileOptions::default()).unwrap();
          zip.write_all(b"test").unwrap();
        }
      }
      let mut zip = zip.finish().unwrap().into_inner();
      if kind == 1 {
        // The ZIP writer correctly refuses duplicate names. Construct the
        // malformed local/central names directly, preserving their lengths.
        let positions: Vec<_> = zip
          .windows(12)
          .enumerate()
          .filter_map(|(index, name)| (name == b"payload/data").then_some(index))
          .collect();
        for position in positions {
          zip[position..position + 12].copy_from_slice(b"setup-v1.2.3");
        }
      }
      manifest.archive_hash = files::hash(&zip);
      let json = serde_json::to_vec(&manifest).unwrap();
      bytes.truncate(start as usize);
      bytes.extend_from_slice(&zip);
      bytes.extend_from_slice(&json);
      for value in [start, zip.len() as u64, json.len() as u64] {
        bytes.extend_from_slice(&value.to_le_bytes());
      }
      bytes.extend_from_slice(MAGIC);
      fs::write(&output, bytes).unwrap();
      let target = temporary.path().join("extract");
      fs::create_dir(&target).unwrap();
      assert!(Bundle::open(&output).unwrap().extract(&target).is_err());
      assert!(fs::read_dir(target).unwrap().next().is_none());
      assert!(!temporary.path().join("outside").exists());
    }
  }
}
