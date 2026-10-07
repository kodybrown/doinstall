# doinstall Agent Notes

- Product: doinstall; standalone Rust installer and build-machine builder.
- Work directly in this checkout on main, as explicitly requested by the user.
  Do not create worktrees or delegate agents unless requested. Preserve unrelated
  changes and never change the user's active installation or PATH in tests.
- Commit scoped, verified changes before handoff. Include a dated entry at the
  end of docs/CHANGELOG.md Unreleased for every agent-created commit. Pushing and
  publishing require separate authorization.
- Verify source changes with build.cmd verify (Windows) or ./build.sh verify
  (Unix). Also run git diff --check. Keep Cargo outputs external and query
  cargo metadata for the evaluated target directory; preserve user tool settings.
- Keep doinstall independent of dotask: no project references, source includes,
  installed dotask requirement, .NET SDK, or dotask-specific configuration reads.
- README.md introduces the product; docs/INSTALLATION.md owns the YAML and
  installation contract. Preserve receipts, immutable builds, activation recovery,
  and refusal of unrecognized existing state. No compatibility loaders or migrations.
- The shipped installer accepts resolved application/runtime YAML, not build
  steps. Assembly belongs to doinstall-builder on the build machine.
- Shim source/assets belong here. Verify hashes ordinarily; source changes require
  rebuilding both architectures with LLVM and native Windows acceptance.
- Preserve useful code comments and update them with behavioral changes.
- Record real platform acceptance separately; Windows tests do not prove Linux
  or macOS behavior. Use temporary install/bin/shortcut roots for acceptance.
