//! Mach-O signing preparation is checked on every host, without invoking Apple tools.
#[allow(dead_code)]
#[path = "../src/binary.rs"]
mod binary;
mod support;
use std::fs;

fn put32(bytes: &mut [u8], offset: usize, value: u32) {
  bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}
fn put64(bytes: &mut [u8], offset: usize, value: u64) {
  bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}
fn get32(bytes: &[u8], offset: usize) -> u32 {
  u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}
fn get64(bytes: &[u8], offset: usize) -> u64 {
  u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
fn signed_template() -> Vec<u8> {
  let mut bytes = vec![0; 528];
  bytes[..4].copy_from_slice(b"\xcf\xfa\xed\xfe");
  put32(&mut bytes, 4, 0x01000007);
  put32(&mut bytes, 16, 4);
  put32(&mut bytes, 20, 184);
  // Keep LC_CODE_SIGNATURE in the middle to catch accidental removal of later
  // commands or shifts of code/data when freeing its header slot for codesign.
  put32(&mut bytes, 32, 0x19);
  put32(&mut bytes, 36, 72);
  bytes[40..46].copy_from_slice(b"__TEXT");
  put64(&mut bytes, 80, 384);
  put32(&mut bytes, 104, 0x1d);
  put32(&mut bytes, 108, 16);
  put32(&mut bytes, 112, 512);
  put32(&mut bytes, 116, 16);
  put32(&mut bytes, 120, 0x19);
  put32(&mut bytes, 124, 72);
  bytes[128..138].copy_from_slice(b"__LINKEDIT");
  put64(&mut bytes, 152, 4096);
  put64(&mut bytes, 160, 384);
  put64(&mut bytes, 168, 144);
  put32(&mut bytes, 192, 2);
  put32(&mut bytes, 196, 24);
  put32(&mut bytes, 200, 400);
  put32(&mut bytes, 204, 2);
  put32(&mut bytes, 208, 432);
  put32(&mut bytes, 212, 72);
  bytes[240..432].fill(0x5a);
  bytes[433..447].copy_from_slice(b"stable-symbol\0");
  bytes[512..].fill(0xcc);
  bytes
}

#[test]
fn removes_signature_command_without_moving_native_data_or_later_commands() {
  let temporary = support::temporary();
  let path = temporary.path().join("template");
  let original = signed_template();
  fs::write(&path, &original).unwrap();
  assert_eq!(binary::signature(&path).unwrap(), (512, Some(112)));
  let mut bytes = original[..512].to_vec();
  binary::remove_macho_signature_command(&mut bytes).unwrap();
  assert_eq!(bytes.len(), 512);
  assert_eq!(get32(&bytes, 16), 3);
  assert_eq!(get32(&bytes, 20), 168);
  assert_eq!(&bytes[104..176], &original[120..192]);
  assert_eq!(&bytes[176..200], &original[192..216]);
  assert_eq!(&bytes[200..216], &[0; 16]);
  assert_eq!(&bytes[240..], &original[240..512]);
  let unsigned = bytes.clone();
  binary::remove_macho_signature_command(&mut bytes).unwrap();
  assert_eq!(bytes, unsigned); // Already unsigned templates remain unchanged.
  fs::write(&path, bytes).unwrap();
  assert_eq!(binary::signature(&path).unwrap(), (512, None));
}

#[test]
fn container_fills_linkedit_and_string_table_before_a_new_signature_trailer() {
  for (architecture, page) in [("x64", 4096), ("arm64", 16384)] {
    let temporary = support::temporary();
    let path = temporary.path().join("bundle");
    let mut bytes = signed_template()[..512].to_vec();
    put32(
      &mut bytes,
      4,
      if architecture == "arm64" {
        0x0100000c
      } else {
        0x01000007
      },
    );
    binary::remove_macho_signature_command(&mut bytes).unwrap();
    let native_data = bytes[240..].to_vec();
    let container = vec![0xa5; 448];
    bytes.extend_from_slice(&container);
    fs::write(&path, &bytes).unwrap();
    binary::extend_macho_linkedit(&path, architecture, 512).unwrap();
    let mut bytes = fs::read(&path).unwrap();
    assert_eq!(get64(&bytes, 136), page);
    assert_eq!(get64(&bytes, 144), 384); // Existing segment file offset.
    assert_eq!(get64(&bytes, 152), 960 - 384);
    assert_eq!(get32(&bytes, 192), 432); // Existing string offset.
    assert_eq!(get32(&bytes, 196), 960 - 432);
    assert_eq!(&bytes[240..512], &native_data);
    assert_eq!(&bytes[512..], &container);
    // Model codesign adding its command in the vacated header space. The
    // unchanged footer boundary must still be found before the new signature.
    put32(&mut bytes, 16, 4);
    put32(&mut bytes, 20, 184);
    put32(&mut bytes, 200, 0x1d);
    put32(&mut bytes, 204, 16);
    put32(&mut bytes, 208, 960);
    put32(&mut bytes, 212, 16);
    bytes.extend_from_slice(&[0xcc; 16]);
    fs::write(&path, bytes).unwrap();
    assert_eq!(binary::signature(&path).unwrap(), (960, Some(208)));
  }
}

#[test]
fn malformed_or_nontrailing_tables_fail_before_changing_the_file() {
  for (offset, value) in [(16, 5), (20, u32::MAX), (108, 0), (192, 0x1d)] {
    let mut bytes = signed_template()[..512].to_vec();
    put32(&mut bytes, offset, value);
    let original = bytes.clone();
    assert!(binary::remove_macho_signature_command(&mut bytes).is_err());
    assert_eq!(bytes, original);
  }
  for (offset, value) in [(176, 0), (192, 510), (196, 32)] {
    let temporary = support::temporary();
    let path = temporary.path().join("invalid");
    let mut bytes = signed_template()[..512].to_vec();
    binary::remove_macho_signature_command(&mut bytes).unwrap();
    put32(&mut bytes, offset, value);
    bytes.extend_from_slice(&[0xa5; 448]);
    fs::write(&path, &bytes).unwrap();
    assert!(binary::extend_macho_linkedit(&path, "arm64", 512).is_err());
    assert_eq!(fs::read(&path).unwrap(), bytes);
  }
  let temporary = support::temporary();
  let path = temporary.path().join("stale-signature");
  let bytes = signed_template();
  fs::write(&path, &bytes).unwrap();
  assert!(binary::extend_macho_linkedit(&path, "arm64", 512).is_err());
  assert_eq!(fs::read(&path).unwrap(), bytes);
}
