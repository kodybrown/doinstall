// dotask: 1
// description: Reusable Cargo operations with external outputs and scanned build executables.
// options:
//   - {name: operation, required: true, choices: [build, test, fmt, clippy]}
//   - {name: manifest, type: path, default: Cargo.toml}
//   - {name: package, default: ''}
//   - {name: profile, default: release}
//   - {name: check, type: bool, default: false}
//   - {name: format-tasks, type: bool, default: false}
// requires: [{kind: tool, value: cargo}, {kind: file, value: _/rust/_support/Rust.rs}]
// end-dotask
#[path = "_support/Rust.rs"]
mod rust;
fn main() {
  dotask_sdk::run(rust::operation);
}
