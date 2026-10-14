# Debian web stack sample

This .NET 9 API supplies `/health` and a response at `/`. Its [configuration](stack.yaml)
contains app values; the [reusable recipe](../../docs/DEBIAN-WEB-STACK.md) owns setup.

Publish the API on the build machine for Linux x64/arm64 using its evaluated
external MSBuild PublishDir. Put output under payload `api/`, bundle the matching
ASP.NET Core runtime and native tools, and copy `recipes/debian-web-stack` into
`workflows/` with this configuration. No SDK or source compilation runs on the server.

Use a disposable Debian 12 or Ubuntu 24.04 systemd host. The sample creates
`/opt/doinstall-sample`, dedicated services/configuration, private Redis on 6381
and an application database. The API listens on loopback 5180. Nginx uses
`sample.example.test`. TLS is disabled because that example domain cannot receive
a public certificate.

Run `installer --non-interactive --validate`, then `installer --non-interactive`.
Check `curl -H 'Host: sample.example.test' http://127.0.0.1/health`. Repeat the same
package, then upgrade to exercise restore-verified backups. Verify credentials and
fixture rows remain and receipts reference the expected active/previous builds.

An intentional readiness failure must leave pending state. Repair the dependency
and retry the original package without removing its journal. Tests must not use an
existing application installation or change user PATH.
