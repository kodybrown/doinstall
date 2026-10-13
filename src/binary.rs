//! The builder validates its supplied destination engine without executing it.
use anyhow::{ensure, Result};
use std::{
  fs::File,
  io::{Read, Seek, SeekFrom},
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
