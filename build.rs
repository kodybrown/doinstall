// A single package build supplies the same identity to all four executables.
#[path = "src/tool_version.rs"]
mod tool_version;
fn main() {
  println!("cargo:rerun-if-changed=src/tool_version.rs");
  println!("cargo:rerun-if-env-changed=RUST_BUILD_TIMESTAMP");
  let seconds = std::env::var("RUST_BUILD_TIMESTAMP")
    .map(|v| v.parse::<u64>().expect("RUST_BUILD_TIMESTAMP must be Unix seconds"))
    .unwrap_or_else(|_| tool_version::now());
  let (metadata, utc) = tool_version::timestamp(seconds);
  println!(
    "cargo:rustc-env=DOINSTALL_DISPLAY_VERSION={}+{metadata}",
    std::env::var("CARGO_PKG_VERSION").unwrap()
  );
  println!("cargo:rustc-env=DOINSTALL_BUILD_UTC={utc}");
}
