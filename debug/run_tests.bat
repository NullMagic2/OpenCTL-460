@echo off
rem Runs every hardware-free test from debug/ and propagates Cargo's exit code.
setlocal
pushd "%~dp0.."
set "CARGO_TARGET_DIR=%CD%\target"
cargo test --locked --tests
set "test_result=%ERRORLEVEL%"
popd
exit /b %test_result%
