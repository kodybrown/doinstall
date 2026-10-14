//! The builder validates its supplied destination engine without executing it.
use anyhow::{ensure, Context, Result};
use std::{
  fs::{File, OpenOptions},
  io::{Read, Seek, SeekFrom, Write},
  path::Path,
};

pub(crate) fn validate(path: &Path, platform: &str, architecture: &str) -> Result<()> {
  let mut file = File::open(path)?;
  let mut header = [0u8; 64];
  file.read_exact(&mut header)?;
  match platform {
    "linux" => {
      ensure!(
        &header[..4] == b"\x7fELF" && header[4] == 2 && header[5] == 1,
        "Linux installer must be a little-endian ELF64 executable."
      );
      let machine = u16::from_le_bytes([header[18], header[19]]);
      ensure!(
        machine == if architecture == "arm64" { 183 } else { 62 },
        "Linux installer architecture does not match package."
      );
    }
    "windows" => {
      ensure!(&header[..2] == b"MZ", "Windows installer must be a PE executable.");
      file.seek(SeekFrom::Start(u32::from_le_bytes(header[60..64].try_into().unwrap()) as u64))?;
      let mut pe = [0u8; 6];
      file.read_exact(&mut pe)?;
      ensure!(&pe[..4] == b"PE\0\0", "Invalid PE installer.");
      let machine = u16::from_le_bytes([pe[4], pe[5]]);
      ensure!(
        machine
          == if architecture == "arm64" {
            0xaa64
          } else {
            0x8664
          },
        "Windows installer architecture does not match package."
      );
    }
    "macos" => {
      let cpu = u32::from_le_bytes(header[4..8].try_into().unwrap());
      ensure!(
        cpu
          == if architecture == "arm64" {
            0x0100000c
          } else {
            0x01000007
          },
        "macOS installer architecture does not match package."
      );
      ensure!(
        &header[..4] == b"\xcf\xfa\xed\xfe",
        "macOS installer must be a Mach-O64 executable."
      );
    }
    _ => anyhow::bail!("Unsupported destination platform."),
  }
  Ok(())
}

/// Locate a signature trailer without searching arbitrary executable bytes.
/// Signing a completed bundle may append a certificate/blob after its footer.
pub(crate) fn signature(path: &Path) -> Result<(u64, Option<u64>)> {
  let mut file = File::open(path)?;
  let length = file.metadata()?.len();
  let mut header = [0u8; 64];
  file.read_exact(&mut header)?;
  let field = if &header[..2] == b"MZ" {
    let pe = u32::from_le_bytes(header[60..64].try_into().unwrap()) as u64;
    file.seek(SeekFrom::Start(pe))?;
    let mut coff = [0; 26];
    file.read_exact(&mut coff)?;
    ensure!(
      &coff[..4] == b"PE\0\0" && &coff[24..26] == b"\x0b\x02",
      "Invalid PE64 executable"
    );
    ensure!(
      u16::from_le_bytes(coff[20..22].try_into().unwrap()) >= 152,
      "Missing PE directories"
    );
    Some(pe + 24 + 144)
  } else if &header[..4] == b"\xcf\xfa\xed\xfe" {
    let count = u32::from_le_bytes(header[16..20].try_into().unwrap());
    let end = 32 + u32::from_le_bytes(header[20..24].try_into().unwrap()) as u64;
    ensure!(end <= length && count as u64 <= (end - 32) / 8, "Invalid Mach-O load commands");
    let mut position = 32;
    let mut field = None;
    for _ in 0..count {
      ensure!(position + 8 <= end, "Truncated Mach-O load command");
      file.seek(SeekFrom::Start(position))?;
      let mut command = [0; 8];
      file.read_exact(&mut command)?;
      let kind = u32::from_le_bytes(command[..4].try_into().unwrap());
      let size = u32::from_le_bytes(command[4..].try_into().unwrap()) as u64;
      ensure!(size >= 8 && size <= end - position, "Invalid Mach-O load command size");
      if kind == 0x1d {
        ensure!(size >= 16 && field.is_none(), "Invalid Mach-O code signature command");
        field = Some(position + 8);
      }
      position += size;
    }
    field
  } else {
    None
  };
  if let Some(field) = field {
    file.seek(SeekFrom::Start(field))?;
    let mut data = [0; 8];
    file.read_exact(&mut data)?;
    let start = u32::from_le_bytes(data[..4].try_into().unwrap()) as u64;
    let size = u32::from_le_bytes(data[4..].try_into().unwrap()) as u64;
    if start != 0 || size != 0 {
      ensure!(
        start >= field + 8 && size != 0 && start.checked_add(size) == Some(length),
        "Invalid signature trailer"
      );
      return Ok((start, Some(field)));
    }
  }
  Ok((length, None))
}

pub(crate) fn validate_launcher(path: &Path, platform: &str, architecture: &str) -> Result<()> {
  validate(path, platform, architecture)?;
  if platform == "windows" {
    let mut file = File::open(path)?;
    file.seek(SeekFrom::Start(60))?;
    let mut offset = [0; 4];
    file.read_exact(&mut offset)?;
    file.seek(SeekFrom::Start(u32::from_le_bytes(offset) as u64 + 24 + 68))?;
    let mut subsystem = [0; 2];
    file.read_exact(&mut subsystem)?;
    ensure!(u16::from_le_bytes(subsystem) == 3, "Launcher must be a console executable");
  }
  Ok(())
}

/// Make the appended container part of the last Mach-O segment before signing.
/// codesign expects the signature at the end of __LINKEDIT, not an overlay
/// outside its declared file range. This also works when packaging on Windows.
#[allow(dead_code)] // Only builder/launcher container code consumes this operation.
pub(crate) fn extend_macho_linkedit(path: &Path, architecture: &str) -> Result<()> {
  let mut file = OpenOptions::new().read(true).write(true).open(path)?;
  let length = file.metadata()?.len();
  let mut header = [0; 32];
  file.read_exact(&mut header)?;
  ensure!(&header[..4] == b"\xcf\xfa\xed\xfe", "Expected Mach-O64 launcher");
  let end = 32 + u32::from_le_bytes(header[20..24].try_into().unwrap()) as u64;
  ensure!(end <= length, "Invalid Mach-O commands");
  let mut position = 32;
  let mut linkedit = None;
  let mut last_segment = 0;
  while position < end {
    file.seek(SeekFrom::Start(position))?;
    let mut command = [0; 8];
    file.read_exact(&mut command)?;
    let kind = u32::from_le_bytes(command[..4].try_into().unwrap());
    let size = u32::from_le_bytes(command[4..].try_into().unwrap()) as u64;
    ensure!(size >= 8 && size <= end - position, "Invalid Mach-O command size");
    if kind == 0x19 {
      ensure!(size >= 72, "Invalid Mach-O segment");
      let mut segment = [0; 64];
      file.read_exact(&mut segment)?;
      let offset = u64::from_le_bytes(segment[32..40].try_into().unwrap());
      last_segment = last_segment.max(offset);
      if &segment[..16] == b"__LINKEDIT\0\0\0\0\0\0" {
        ensure!(linkedit.is_none(), "Duplicate __LINKEDIT segment");
        linkedit = Some((position, offset));
      }
    }
    position += size;
  }
  let (command, offset) = linkedit.context("Missing __LINKEDIT segment")?;
  ensure!(
    offset == last_segment && offset < length,
    "__LINKEDIT must be the last file segment"
  );
  let size = length - offset;
  let page = if architecture == "arm64" { 16384 } else { 4096 };
  let virtual_size = size.div_ceil(page) * page;
  file.seek(SeekFrom::Start(command + 32))?;
  file.write_all(&virtual_size.to_le_bytes())?;
  file.seek(SeekFrom::Start(command + 48))?;
  file.write_all(&size.to_le_bytes())?;
  file.sync_all()?;
  Ok(())
}
