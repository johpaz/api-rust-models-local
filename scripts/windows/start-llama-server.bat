@echo off
REM ═══════════════════════════════════════════════════════
REM Start llama-server on Windows (Vulkan backend)
REM ═══════════════════════════════════════════════════════
REM
REM LLM API Server - Developed by @johpaz
REM Windows version - Vulkan GPU
REM
REM Usage:
REM   scripts\windows\start-llama-server.bat
REM
REM Configuration: edit .env in the project root

setlocal enabledelayedexpansion

REM Get project root
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR:~0,-21%"
set "ENV_FILE=%PROJECT_ROOT%\.env"

REM ═══════════════════════════════════════════════════════
REM Banner
REM ═══════════════════════════════════════════════════════
echo.
echo ^+---------------------------------------------------------^+
echo ^|                                                         ^|
echo ^|    LLM  API  SERVER                                     ^|
echo ^|         (Windows Vulkan)                                ^|
echo ^|                                                         ^|
echo ^|         Version:  1.0.0                                 ^|
echo ^|         Author:   @johpaz                               ^|
echo ^|                                                         ^|
echo ^+---------------------------------------------------------^+
echo.

REM ═══════════════════════════════════════════════════════
REM Load .env (simple parser for KEY=VALUE)
REM ═══════════════════════════════════════════════════════
if not exist "%ENV_FILE%" (
    echo   ERROR: .env not found at %ENV_FILE%
    echo   Copy .env.example to .env and edit configuration:
    echo   copy .env.example .env
    pause
    exit /b 1
)

echo   Loading configuration from .env...

REM Set defaults
set "PORT=8080"
set "HOST=0.0.0.0"
set "MODEL_NAME="
set "CONTEXT_SIZE=4096"
set "GPU_LAYERS=35"
set "LLAMA_ARG_CACHE_TYPE_K=q4_0"
set "LLAMA_ARG_CACHE_TYPE_V=q4_0"
set "VK_ICD_FILENAMES="
set "MESA_VK_WSI=1"

REM Parse .env file (skip comments and empty lines)
for /f "usebackq tokens=1,* delims==" %%A in ("%ENV_FILE%") do (
    set "line=%%A"
    REM Skip comments
    if not "!line:~0,1!"=="#" (
        if not "%%A"=="" (
            REM Remove surrounding quotes from value
            set "val=%%~B"
            if "%%A"=="PORT" set "PORT=!val!"
            if "%%A"=="HOST" set "HOST=!val!"
            if "%%A"=="MODEL_NAME" set "MODEL_NAME=!val!"
            if "%%A"=="CONTEXT_SIZE" set "CONTEXT_SIZE=!val!"
            if "%%A"=="GPU_LAYERS" set "GPU_LAYERS=!val!"
            if "%%A"=="LLAMA_ARG_CACHE_TYPE_K" set "CACHE_TYPE_K=!val!"
            if "%%A"=="LLAMA_ARG_CACHE_TYPE_V" set "CACHE_TYPE_V=!val!"
            if "%%A"=="VK_ICD_FILENAMES" set "VK_ICD_FILENAMES=!val!"
            if "%%A"=="MESA_VK_WSI" set "MESA_VK_WSI=!val!"
        )
    )
)

REM Fallback: if .env doesn't set Vulkan ICD, use default Windows paths
if "!VK_ICD_FILENAMES!"=="" (
    set "VK_ICD_FILENAMES=C:\VulkanSDK\etc\vulkan\icd.d\radeon_icd.x86_64.json;C:\VulkanSDK\etc\vulkan\icd.d\intel_icd.x86_64.json"
)

REM ═══════════════════════════════════════════════════════
REM Paths
REM ═══════════════════════════════════════════════════════
set "LLAMA_BINARY=%PROJECT_ROOT%\llama-server\build-windows\llama.cpp\build\bin\llama-server.exe"
set "MODEL_PATH=%PROJECT_ROOT%\models\%MODEL_NAME%"
set "LOG_FILE=%TEMP%\llama.log"

REM ═══════════════════════════════════════════════════════
REM Validations
REM ═══════════════════════════════════════════════════════
echo.
echo   Validations...

if not exist "%LLAMA_BINARY%" (
    echo   ERROR: Binary not found: %LLAMA_BINARY%
    echo   Build it first with:
    echo     scripts\windows\build-llama-server.bat
    pause
    exit /b 1
)

if "%MODEL_NAME%"=="" (
    echo   ERROR: MODEL_NAME not set in .env
    echo   Add: MODEL_NAME=your_model.gguf
    pause
    exit /b 1
)

if not exist "%MODEL_PATH%" (
    echo   ERROR: Model not found: %MODEL_PATH%
    echo.
    echo   Available models:
    dir /b "%PROJECT_ROOT%\models\*.gguf" 2>nul || echo     (none found)
    echo.
    echo   Download one with:
    echo     scripts\windows\download-model.bat ^<repo^> ^<filename^>
    pause
    exit /b 1
)

REM ═══════════════════════════════════════════════════════
REM Kill existing process
REM ═══════════════════════════════════════════════════════
taskkill /IM llama-server.exe /F >nul 2>nul
if not errorlevel 1 (
    echo   Stopped existing llama-server process
    echo.
)

REM ═══════════════════════════════════════════════════════
REM Show configuration
REM ═══════════════════════════════════════════════════════
echo.
echo   Configuration:
echo   Model:        %MODEL_NAME%
echo   Host:         %HOST%
echo   Port:         %PORT%
echo   Context:      %CONTEXT_SIZE% tokens
echo   GPU Layers:   %GPU_LAYERS%
echo   Cache K:      %CACHE_TYPE_K%
echo   Cache V:      %CACHE_TYPE_V%
echo.

REM ═══════════════════════════════════════════════════════
REM Build command
REM ═══════════════════════════════════════════════════════
set "CMD_ARGS=--model %MODEL_PATH% --host %HOST% --port %PORT% --ctx-size %CONTEXT_SIZE% --n-gpu-layers %GPU_LAYERS% --cache-type-k %CACHE_TYPE_K% --cache-type-v %CACHE_TYPE_V%"

REM Check for TurboQuant
echo %CACHE_TYPE_K% | findstr /b turbo >nul
if not errorlevel 1 (
    echo   TurboQuant detected: adding --flash-attn on
    set "CMD_ARGS=!CMD_ARGS! --flash-attn on"
)
echo %CACHE_TYPE_V% | findstr /b turbo >nul
if not errorlevel 1 (
    echo   TurboQuant detected: adding --flash-attn on
    set "CMD_ARGS=!CMD_ARGS! --flash-attn on"
)

REM ═══════════════════════════════════════════════════════
REM Start server
REM ═══════════════════════════════════════════════════════
echo.
echo   Starting llama-server with Vulkan GPU...
echo   Logs: type %LOG_FILE%
echo   Stop: taskkill /IM llama-server.exe /F
echo   Health: curl http://localhost:%PORT%/health
echo.

cd /d "%PROJECT_ROOT%"

REM Set Vulkan environment variables and start in background
set VK_ICD_FILENAMES=%VK_ICD_FILENAMES%
set MESA_VK_WSI=%MESA_VK_WSI%

start "llama-server" /B cmd /c "%LLAMA_BINARY% %CMD_ARGS% > %LOG_FILE% 2>&1"

echo.
echo   llama-server started
echo.
echo   Loading model... wait ~30-60s
echo   Verify with: curl http://localhost:%PORT%/health
echo.

pause
