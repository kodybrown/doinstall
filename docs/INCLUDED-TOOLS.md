# Included tools and server workflows

Doinstall can launch pinned executables bundled in its payload. The interface uses
ordinary argument tokens and environment variables. Doinstall does not parse
dotask settings, embed dotask's engine, or require a source checkout, Cargo, or
.NET SDK on the destination.

A consumer can bundle the native dotask executable and only YAML `.task` files.
The consuming build system owns publishing, environment selection, archive
creation, SSH/SCP, and the decision to execute the package.

## Runtime configuration

This is a Linux server package example. Replace the tool hash with the SHA-256 of
the exact bundled executable; the placeholder deliberately does not validate.

```yaml
schema: 1
application: {id: example, name: Example API, version: 1.0.0}
platform: linux
architecture: x64
payload: payload
runtime:
  interactive: false
  values:
    install-dir: /opt/example
    add-to-path: false
  commands:
    - {name: api, executable: api/Example.API}
  host:
    distributions:
      debian: ["12"]
      ubuntu: ["24.04"]
    require-root: true
    require-systemd: true
  tools:
    workflow:
      executable: tooling/dotask
      sha256: "<64-character SHA-256>"
  managed-files:
    - /etc/example/api.env
    - /etc/systemd/system/example.service
    - /etc/nginx/conf.d/example.conf
  lifecycle:
    prepare:
      - tool: workflow
        arguments: [--use-dir, workflows/.tasks, prepare]
        timeout-seconds: 3600
    before-activate:
      - tool: workflow
        arguments: [--use-dir, workflows/.tasks, before-activate]
        timeout-seconds: 600
    after-activate:
      - tool: workflow
        arguments: [--use-dir, workflows/.tasks, after-activate]
        timeout-seconds: 600
```

Included-tool workflows currently require non-interactive execution: set
`runtime.interactive: false` or pass `--non-interactive`. Child tools use ordinary
process streams; routing those streams through the Cursive dialog is deferred.

A lifecycle can be used without command launchers or shortcuts. Each invocation
requires a declared tool. Executable paths are relative to the immutable build,
must remain inside it, and are verified against their pinned hash before launch.
Tool working directory is the build directory. Arguments and environment values
can use existing input templates plus `${build-dir}` and `${context-file}`.
Timeouts default to 600 seconds and must be from 1 through 86400.

The installer always supplies `DOINSTALL_CONTEXT_FILE`, pointing to a temporary
JSON file for the duration of the workflow. It contains:

- `schema`, `application-id`, `install-dir`, and `management-dir`.
- `build-dir` and nullable `previous-build-dir`.
- Resolved runtime `values`.

A dotask YAML step can read it with
`file: {operation: read, path: {environment: DOINSTALL_CONTEXT_FILE}, format: json}`.
The installer never prints tool arguments, environment values, or context contents.
Tools own their output; avoid passing raw secrets as runtime inputs or CLI tokens.
On Unix, `DOINSTALL_PROCESS_GROUP_OWNER` identifies the installer's owning process.
Bundled runners can preserve that process group for descendant cleanup. Both
installer environment names are reserved and cannot be overridden by a tool call.
Runtime values are retained in the existing installation receipt. Use references
to persistent secret files instead.

## Destination validation and cross-platform assembly

The builder performs structural validation on the build machine. It validates
the supplied installer's native executable format and architecture without
executing it. Destination validation runs before installation writes:

1. Package platform and architecture must match the running engine.
2. A distribution-gated package must run on Linux with an exact allowed
   `/etc/os-release` `ID` and `VERSION_ID`.
3. Optional root and running-systemd requirements must pass.

Only explicit Debian and Ubuntu version allowlists are supported initially.
`ID_LIKE` does not admit derivatives. The example versions are an explicit
package policy, not a claim of native stack acceptance.

When assembling a Unix package on Windows, directory payloads become ZIPs with
explicit file permissions: declared command, shortcut, and tool executables use
0755 and other files use 0644. The destination engine filename follows the package
platform (`installer` for Linux), not the builder's OS. The outer transport must
make that Linux installer executable after extraction when its archive lacks
Unix permission metadata.

## Phase ordering and recovery

Doinstall validates ownership, stages the immutable build, and saves its pending
activation record before launching lifecycle actions. Execution order is:

1. `prepare`: provision prerequisites and configure the stack.
2. `before-activate`: database preparation/migrations and service stop.
3. Native command/shortcut activation.
4. `after-activate`: start services and verify readiness.
5. Commit the successful receipt and then prune eligible older builds.

A failed or timed-out tool stops installation and retains `pending.yaml` and
`workflow.yaml`. No successful installation is reported. Retry the original
package with the same inputs. Successful earlier phases run again, so workflows
must be idempotent. Missing references and changed retry inputs fail explicitly.

External configuration files must be listed in `managed-files`; paths must be
absolute and outside the installation root. First installation refuses to adopt
an existing unowned file. Updates verify owned file hashes and Unix permissions
before changing destination state. Removing a managed path from the manifest
requires an explicit cleanup operation.

Before a new build's workflow, doinstall snapshots existing owned configuration
under `installer/config/<build>/`. Unix snapshot directories/files use 0700/0600.
The journal records original and observed hashes/modes, including partial changes
after a failed tool. A hard crash between tool writes and journal updates refuses
to adopt unexplained changes; inspect the saved snapshots and restore/resolve
configuration before retrying.

These records support inspection and forward recovery. They do not automatically
undo package-manager changes, database migrations, certificates, or tool side
effects outside declared files. Previous application builds are retained, but
automatic application/database rollback is not implemented.

Lifecycle-managed uninstall is deliberately refused until a consuming cleanup
workflow can stop its services and handle owned configuration. Persistent
databases, secrets, and certificates must live outside immutable builds; they
must not be erased or regenerated during repeated installation.
