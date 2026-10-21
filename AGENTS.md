# doinstall Agent Notes

- Product: doinstall; standalone Rust installer and build-machine builder.
- Work directly in this checkout on develop, as explicitly requested by the user.
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
  Optional maintainer tasks may use dotask, but Cargo-only development and CI
  must remain available. .tasks/_/rust contains managed copies from dotask's
  shared-tasks/rust catalog; edit canonical sources in dotask, publish its catalog,
  then use dotask --sync here. Do not edit or format managed task files. Keep
  project policy/payload wrappers at the task root. Do not hardcode doinstall into
  shared tasks. Use Rust or declarative .task automation. Never add Python helpers
  or a Python requirement to tasks, builds, tests, packaging, or releases.
- README.md introduces the product; docs/INSTALLATION.md owns the YAML and
  installation contract. Preserve receipts, immutable builds, activation recovery,
  and refusal of unrecognized existing state. No compatibility loaders or migrations.
- For consumer package creation, follow the self-extracting build-machine recipe
  in docs/INSTALLATION.md. Use prebuilt target runtime/launcher tools and the
  builder's result FilePath; do not guess delivery names or compile per application.
- The shipped installer accepts resolved application/runtime YAML, not build
  steps. Assembly belongs to doinstall-builder on the build machine.
- Shim source/assets belong here. Verify hashes ordinarily; source changes require
  rebuilding both architectures with LLVM and native Windows acceptance.
- Preserve useful code comments and update them with behavioral changes.
- Record real platform acceptance separately; Windows tests do not prove Linux
  or macOS behavior. Use temporary install/bin/shortcut roots for acceptance.
