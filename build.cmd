@echo off
setlocal DisableDelayedExpansion
pushd "%~dp0" || exit /b 1
cargo run --quiet --locked --profile bootstrap --manifest-path Cargo.toml --package doinstall-dev -- %*
set "doinstall_exit=%errorlevel%"
popd
exit /b %doinstall_exit%
