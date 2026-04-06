@echo off
REM ═══════════════════════════════════════════════════════
REM Download a GGUF model from HuggingFace (Windows)
REM ═══════════════════════════════════════════════════════
REM
REM Usage:
REM   scripts\windows\download-model.bat ^<repo_id^> ^<filename^>
REM
REM Example:
REM   scripts\windows\download-model.bat bartowski/google_gemma-4-E4B-it-GGUF google_gemma-4-E4B-it-Q4_K_M.gguf

setlocal enabledelayedexpansion

set "REPO_ID=%~1"
set "FILENAME=%~2"

if "%REPO_ID%"=="" (
    echo Usage: %0 ^<repo_id^> ^<filename^>
    echo.
    echo Example:
    echo   %0 bartowski/google_gemma-4-E4B-it-GGUF google_gemma-4-E4B-it-Q4_K_M.gguf
    pause
    exit /b 1
)

if "%FILENAME%"=="" (
    echo Usage: %0 ^<repo_id^> ^<filename^>
    echo.
    echo Example:
    echo   %0 bartowski/google_gemma-4-E4B-it-GGUF google_gemma-4-E4B-it-Q4_K_M.gguf
    pause
    exit /b 1
)

REM Check if huggingface-cli is installed
where huggingface-cli >nul 2>nul
if %errorlevel% neq 0 (
    echo   ERROR: huggingface-cli not installed.
    echo   Install with: pip install huggingface_hub
    echo   Or: pipx install huggingface_hub
    echo.
    echo   Python required: https://www.python.org/downloads/
    pause
    exit /b 1
)

REM Get project root
set "SCRIPT_DIR=%~dp0"
set "PROJECT_ROOT=%SCRIPT_DIR:~0,-21%"
set "MODELS_DIR=%PROJECT_ROOT%\models"

if not exist "%MODELS_DIR%" mkdir "%MODELS_DIR%"

echo.
echo   Downloading model...
echo   Repository: %REPO_ID%
echo   File: %FILENAME%
echo   Destination: %MODELS_DIR%\
echo.

huggingface-cli download "%REPO_ID%" "%FILENAME%" --local-dir "%MODELS_DIR%"

echo.
echo   Model downloaded: %MODELS_DIR%\%FILENAME%
echo.
echo   To use it, add to your .env:
echo     MODEL_NAME=%FILENAME%
echo.
echo   Then run:
echo     scripts\windows\start-llama-server.bat
echo.

pause
