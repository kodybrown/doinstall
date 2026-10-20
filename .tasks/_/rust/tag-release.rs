// dotask: 1
// description: Tag a clean commit after matching branch CI; --push sends only that new tag.
// options:
//   - {name: manifest, type: path, default: Cargo.toml}
//   - {name: package, default: ''}
//   - {name: branch, default: main}
//   - {name: workflow, default: release.yml}
//   - {name: repo, default: '', description: Optional GitHub owner/repository; otherwise infer from checkout.}
//   - {name: create, type: bool, default: true}
//   - {name: push, type: bool, default: false}
// requires: [{kind: tool, value: cargo}, {kind: tool, value: git}, {kind: tool, value: gh}, {kind: file, value: _/rust/_support/Rust.rs}]
// end-dotask
#[path = "_support/Rust.rs"]
mod rust;
fn main() {
  dotask_sdk::run(rust::tag);
}
