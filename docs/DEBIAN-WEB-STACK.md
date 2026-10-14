# Debian/Ubuntu web stack recipe

This reusable consumer recipe runs native dotask bundled by the application build
system. Doinstall remains independent of dotask. Copy `recipes/debian-web-stack`
into payload `workflows/` and supply `workflows/stack.yaml`. Bundle native
`tooling/dotask`, `tooling/doinstall-server`, and an extracted ASP.NET Core runtime
under `runtime/`. Declare their SHA-256 hashes as included tools; declaring
`runtime/dotnet` preserves its executable permission when building on Windows.

The entry points are `prepare`, `before-activate`, and `after-activate`. Server
orchestration is YAML `.task` files. Apt, systemctl, nginx and certbot run directly.
The optional native helper handles structured validation, credentials, PostgreSQL
ownership, private process input, restore-verified backups, application migrations
and HTTP readiness. It is separate from the installer engine.

## Package policy

The recipe admits Debian 12 and Ubuntu 24.04 with running systemd, root, and
matching x64/arm64 executables. Declare the same installer host requirements so
rejection precedes destination writes. Schema support does not prove native
acceptance; see the recorded verification.

Set each lifecycle environment to `DOINSTALL_SERVER_PLAN=/run/doinstall-ID/plan.json`
and `DEBIAN_FRONTEND=noninteractive`. Doinstall supplies `DOINSTALL_CONTEXT_FILE`.
Plans live in a root-only 0700 directory with 0600 files and may contain secrets;
they are never printed. Success removes the plan; failure retains it for inspection.

The build system selects an official ASP.NET Core runtime, verifies its published
SHA-512 and bundles it. Apt installs native libraries and services. The server
requires no SDK or global .NET installation. Package refresh/install is automatic;
a full OS upgrade, reboot, firewall policy and SSH hardening remain operator policy.

## Configuration

The [sample](../examples/debian-web-stack/stack.yaml) contains application values.
The helper accepts schema 1 YAML or JSON and rejects unknown fields.

| Field | Meaning |
| --- | --- |
| `id` | Account/services, install root `/opt/ID`, and persistent paths |
| `domain` | One nginx virtual host and optional certificate domain |
| `port`, `executable`, `arguments` | Loopback API and payload-relative `api/` apphost |
| `database.role` | Dedicated login without superuser/createdb privileges |
| `database.connections` | Application environment key -> database name |
| `redis` | Private port and host/port/password environment key names |
| `environment` | Literal app settings; generated credentials cannot be overridden |
| `migrations` | Optional payload executable, tokenized arguments and environment |
| `frontend` | Optional `web/NAME` directory containing a prebuilt SPA |
| `tls.enabled`, `tls.email` | Webroot certificate and dedicated renewal timer |
| `health-path`, `health-status` | Readiness, default `/health` and 200 |

A frontend uses SPA fallback at `/` and proxies `/api/` without stripping its
prefix. Otherwise nginx proxies all application paths. Forwarded headers use the
distribution's `proxy_params`.

For real certificate issuance, DNS must resolve to this host and port 80 must be
reachable. Certbot first uses HTTP webroot validation, then the recipe writes TLS
configuration, validates nginx and reloads it. A dedicated systemd timer invokes
native certbot renewal followed by native nginx reload, without a shell hook.
Invalid configuration fails before reload and remains in the recovery journal.

## Ownership and recovery

Declare every path from `doinstall-server describe --config stack.yaml` in
`managed-files`: credentials, app environment, Redis/nginx config and service/
renewal units. Doinstall refuses unowned or modified files before workflows run.

Passwords are generated once using OS randomness, stored root-only and reused.
Existing accounts, roles and databases require application ownership markers.
Password drift fails rather than resetting credentials. SQL travels through stdin;
password-bearing SQL and its stderr are not logged. Redis binds loopback with
authentication, using its own service and persistent state. The distro's ordinary
Redis instance is retained. PostgreSQL uses its ordinary local service.

The API runs under a dedicated unprivileged account, with a read-only system view
and writable state/log directories. Secrets, databases and certificates stay
outside immutable versions.

Updates stop API writes and back up every database owned by the application role,
including later tenant databases. Each custom-format dump is restored into a
unique temporary database, then that database is dropped before migrations run.
Dumps remain root-only under `/var/backups/ID/BUILD/`. Restore requires adequate
disk space. A backup failure stops installation with the API stopped. First
installation has no previous application database to back up.

Application migrations must return nonzero on failure. Migration environment
overrides can enable migration behavior disabled during ordinary service startup.
Success is committed only after readiness and activation pass.

Retry a failed installation with the original package and unchanged inputs. Do
not alter its included tools/tasks. Journals, snapshots and retained versions
support forward recovery; apt changes, migrations and certificates do not roll
back automatically. Lifecycle-managed uninstall is refused until an explicit
cleanup workflow exists. Deploy-time dumps are not scheduled off-machine backups.

## Verified acceptance

On 2026-10-14, Windows release verification passed 100 Rust tests. Native Ubuntu
release verification passed 88 tests, formatting, clippy, shim integrity/PE checks
and whitespace. Its temporary Linux clone included the exact staged source diff:
the distribution's older Git cannot read the Windows repository's relative
worktree extension. Source output paths remained external.

Disposable Ubuntu 24.04 and Debian 12 systemd installations ran the same static
x64 native tools and pinned .NET runtime. Both installed the consuming Spark API
and the standalone sample with nginx SPA fallback/API proxying. Ubuntu additionally
verified repeat installation, updates, credential/data preservation, actual
restore-verified dumps, modified-configuration refusal before workflows, migration
failure with retained pending state and successful original-package retry, and
strict-host-key SSH/SCP upload/hash/extract/remote execution from Windows.
Debian additionally verified dependency-aware health failure/recovery.

Prebuilt GNU binaries must target the oldest supported glibc. The tested portable
bundle instead uses Rust's `x86_64-unknown-linux-musl` target for native tools;
the ASP.NET Core runtime remains the official `linux-x64` archive. Supply separately
built matching native arm64 tools for that architecture.

Live ACME issuance/renewal against a real domain and native arm64 acceptance remain
pending. The TLS configuration/commands are implemented; HTTP-only test domains
do not prove certificate issuance. No production environment or user PATH was
changed. Both disposable distributions were stopped after acceptance.
