use sha2::{Digest, Sha256};
#[test]
fn bundled_shims_match_hashes_architectures_and_are_native_console_programs() {
  let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
  let hashes: serde_json::Value = serde_json::from_str(include_str!("../src/shim/assets/hashes.json")).unwrap();
  for (name, hash) in hashes.as_object().unwrap() {
    let path = if name == "build-tool.rs" {
      root.join("xtask/src/main.rs")
    } else {
      root.join("src/shim").join(name)
    };
    assert_eq!(
      format!("{:x}", Sha256::digest(std::fs::read(path).unwrap())),
      hash.as_str().unwrap()
    );
  }
  for (name, machine) in [("x64", 0x8664u16), ("arm64", 0xaa64)] {
    let bytes = std::fs::read(root.join(format!("src/shim/assets/win-{name}.exe"))).unwrap();
    assert_eq!(&bytes[..2], b"MZ");
    let pe = u32::from_le_bytes(bytes[0x3c..0x40].try_into().unwrap()) as usize;
    assert_eq!(&bytes[pe..pe + 4], b"PE\0\0");
    assert_eq!(u16::from_le_bytes(bytes[pe + 4..pe + 6].try_into().unwrap()), machine);
    let optional = pe + 24;
    assert_eq!(u16::from_le_bytes(bytes[optional..optional + 2].try_into().unwrap()), 0x20b);
    assert_eq!(u16::from_le_bytes(bytes[optional + 68..optional + 70].try_into().unwrap()), 3);
    // PE32+'s CLR directory must be absent: these launchers need no .NET host.
    assert_eq!(&bytes[optional + 112 + 14 * 8..optional + 112 + 15 * 8], &[0u8; 8]);
  }
}
