@echo off
REM ═══════════════════════════════════════════════════════
REM Build script for Rust LLM API (Windows)
REM ═══════════════════════════════════════════════════════
REM Compiles the Rust API with release optimizations
REM
REM Usage:
REM   scripts\windows\build-api.bat

setlocal enabledelayedexpansion

REM Get project root
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR:~0,-21%"
set "API_DIR=%PROJECT_ROOT%\api"

echo.
echo   Building Rust API (Windows)...
echo   Project: %API_DIR%
echo.

REM Check if Rust is installed
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo   ERROR: Rust/Cargo not found.
    echo   Install from: https://rustup.rs/
    echo   Or: winget install Rustlang.Rustup
    pause
    exit /b 1
)

cd /d "%API_DIR%"

REM Build with release optimizations
echo   Compiling with --release...
cargo build --release

REM Show build results
set "BINARY=%API_DIR%\target\release\rust_llm_api.exe"
if exist "%BINARY%" (
    echo.
    echo   Build successful!
    echo   Binary: %BINARY%
    echo.
    echo   To run manually:
    echo     cd %API_DIR% ^&^& cargo run --release
    echo.
) else (
    echo   ERROR: Build failed. Binary not found at: %BINARY%
    pause
    exit /b 1
)

pause
