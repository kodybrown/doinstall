# doinstall Agent Notes

## IMPORTANT: Shared AGENTS.md migration pending

At the start of your next task in this project, tell the user prominently that
this project's `AGENTS.md` still needs a migration review. Pause the original
task before making project changes.

Compare this file and any copied instruction modules with the current global
`~/.codex/AGENTS.md` and applicable shared instructions. Use the primary
checkout. Exclude linked worktrees and submodules.

Show the user exactly what can be removed, which shared rules already cover
it, and what should stay local. Flag conflicting rules and suggest reusable
guidance for discussion. Wait for the user to approve the migration or
explicitly defer it before continuing the original task. Remove this reminder
only when the user approves completing the migration.


Reusable agent policy comes from the global instruction collection. Keep these
product-specific requirements and exceptions.

- Product: doinstall; standalone Rust installer and build-machine builder.
- Work as a single agent; do not delegate unless requested.
- Changelog: `docs/CHANGELOG.md`.
- Source verification: `build.cmd verify` (Windows) or `./build.sh verify` (Unix).
  Focused installation-root checks: `build.cmd verify --test workflow --filter installation_root`
  or `dotask test --test workflow --filter installation_root`. These retain formatting,
  all-target lint, final executable inspection, and shim integrity checks.
- Keep doinstall independent of dotask: no project references, source includes,
  installed dotask requirement, .NET SDK, or dotask-specific configuration reads.
  Optional maintainer tasks may use dotask; Cargo-only development/CI must remain
  available.
- `.tasks/_/rust` contains managed copies from dotask's `shared-tasks/rust` catalog.
  Do not edit or format them. Keep project policy/payload wrappers at the task
  root and do not hardcode doinstall behavior into shared tasks.
  Use Rust or declarative `.task` automation.
- `README.md` introduces the product; `docs/INSTALLATION.md` owns YAML and
  installation contracts. Preserve receipts, immutable builds, activation recovery,
  and refusal of unrecognized existing state. No compatibility loaders/migrations.
- For consumer packages, follow the self-extracting build-machine recipe in
  `docs/INSTALLATION.md`. Use prebuilt target runtime/launcher tools; do not compile
  them per application.
- The shipped installer accepts resolved application/runtime YAML, not build
  steps. Assembly belongs to doinstall-builder on the build machine.
- Current local creator: `create-doinstall-installer`; changing its task name is
  a separate migration. Its deliveries follow the shared release-directory
  convention and must occur only after native acceptance.
- Shim source/assets belong here. Verify hashes ordinarily; source changes
  require rebuilding both architectures with LLVM and native Windows acceptance.
- Installer acceptance uses temporary install/bin/shortcut roots; preserve the
  user's active installation and PATH.
