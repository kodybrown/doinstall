// dotask: 1
// description: Refresh external UTC build metadata; optionally increment a Cargo package version.
// options:
//   - {name: manifest, type: path, default: Cargo.toml}
//   - {name: package, default: ''}
//   - {name: increment, type: bool, default: false, description: Increment patch.}
//   - {name: increment-minor, type: bool, default: false}
//   - {name: increment-major, type: bool, default: false}
//   - {name: dry-run, type: bool, default: false}
// requires: [{kind: tool, value: cargo}, {kind: file, value: _/rust/_support/Rust.rs}]
// end-dotask
#[path = "_support/Rust.rs"]
mod rust;
fn main() {
  dotask_sdk::run(rust::update);
}
