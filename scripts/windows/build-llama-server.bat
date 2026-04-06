@echo off
REM ═══════════════════════════════════════════════════════
REM Build script for llama-server with Vulkan GPU support (Windows)
REM ═══════════════════════════════════════════════════════
REM Clones llama.cpp and compiles with Vulkan GPU acceleration
REM
REM Usage:
REM   scripts\windows\build-llama-server.bat [install_dir]

setlocal enabledelayedexpansion

REM Get project root (parent of scripts\windows\)
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR:~0,-21%"

if "%1"=="" (
    set "INSTALL_DIR=%PROJECT_ROOT%\llama-server\build-windows"
) else (
    set "INSTALL_DIR=%~1"
)
set "LLAMA_DIR=%INSTALL_DIR%\llama.cpp"

echo.
echo   Building llama-server with Vulkan GPU support (Windows)...
echo   Install directory: %INSTALL_DIR%
echo.

REM Check dependencies
echo   Checking dependencies...

where cmake >nul 2>nul
if %errorlevel% neq 0 (
    echo   ERROR: cmake not found. Install from https://cmake.org/download/
    echo   Or: winget install Kitware.CMake
    pause
    exit /b 1
)

where cl >nul 2>nul
if %errorlevel% neq 0 (
    echo   WARNING: MSVC compiler not found.
    echo   Install Visual Studio Build Tools with C++ workload.
    echo   See: https://visualstudio.microsoft.com/downloads/
    echo.
)

where git >nul 2>nul
if %errorlevel% neq 0 (
    echo   ERROR: git not found. Install from https://git-scm.com/download/win
    echo   Or: winget install Git.Git
    pause
    exit /b 1
)

where glslc >nul 2>nul
if %errorlevel% neq 0 (
    echo   WARNING: glslc (Vulkan shader compiler) not found.
    echo   Install Vulkan SDK from https://vulkan.lunarg.com/sdk/home#windows
    echo   Build may fail without it.
    echo.
)

echo   Dependencies check complete.
echo.

REM Create build directory
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

REM Clone llama.cpp if not exists
if not exist "%LLAMA_DIR%" (
    echo   Cloning llama.cpp...
    git clone --depth 1 --branch b5336 https://github.com/ggerganov/llama.cpp "%LLAMA_DIR%"
    echo   llama.cpp cloned
) else (
    echo   llama.cpp already exists at %LLAMA_DIR%
)

cd /d "%LLAMA_DIR%"

REM Build with Vulkan GPU support
echo.
echo   Compiling llama-server with Vulkan...
echo   Configuration:
echo   - GPU Backend: Vulkan
echo   - Build Type: Release
echo   - Server: ON
echo   - Tests: OFF
echo   - Examples: OFF
echo.

cmake -B build ^
    -DCMAKE_BUILD_TYPE=Release ^
    -DLLAMA_BUILD_TESTS=OFF ^
    -DLLAMA_BUILD_EXAMPLES=OFF ^
    -DLLAMA_BUILD_SERVER=ON ^
    -DGGML_VULKAN=ON ^
    -DBUILD_SHARED_LIBS=OFF

cmake --build build --target llama-server --config Release

REM Verify build
set "BINARY=%LLAMA_DIR%\build\bin\llama-server.exe"
if exist "%BINARY%" (
    for %%A in ("%BINARY%") do set "BINARY_SIZE=%%~zA"
    echo.
    echo   Build successful!
    echo   Binary: %BINARY%
    echo.
    echo   To test:
    echo     %BINARY% --help
    echo.
    echo   To start server:
    echo     scripts\windows\start-llama-server.bat
) else (
    echo   ERROR: Build failed. Binary not found at: %BINARY%
    pause
    exit /b 1
)

pause
