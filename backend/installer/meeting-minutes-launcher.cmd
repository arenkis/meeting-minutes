@echo off
setlocal enabledelayedexpansion

REM Meeting Minutes Backend Launcher
REM Starts both whisper-server and Python backend

title Meeting Minutes Backend

REM Change to application directory
cd /d "%~dp0"

REM Load configuration from .env if exists
if exist ".env" (
    for /f "tokens=1,2 delims==" %%a in (.env) do (
        set "%%a=%%b"
    )
)

REM Set defaults if not configured
if not defined WHISPER_PORT set WHISPER_PORT=8178
if not defined BACKEND_PORT set BACKEND_PORT=8000
if not defined WHISPER_MODEL set WHISPER_MODEL=small
if not defined WHISPER_HOST set WHISPER_HOST=127.0.0.1
if not defined BACKEND_HOST set BACKEND_HOST=127.0.0.1

REM Parse command line arguments
:parse_args
if "%~1"=="" goto check_startup
if "%~1"=="--startup" (
    set STARTUP_MODE=1
    shift
    goto parse_args
)
if "%~1"=="--stop" goto stop_services
if "%~1"=="--restart" goto restart_services
if "%~1"=="--status" goto check_status
if "%~1"=="--help" goto show_help
shift
goto parse_args

:check_startup
if defined STARTUP_MODE (
    REM Running in startup mode - minimize window
    if not "%MINIMIZED%"=="1" (
        set MINIMIZED=1
        start /min cmd /c "%~dpnx0 %*"
        exit
    )
)

:start_services
echo ===============================================
echo     Meeting Minutes Backend Launcher
echo ===============================================
echo.

REM Check if services are already running
tasklist /FI "IMAGENAME eq whisper-server.exe" 2>NUL | find /I /N "whisper-server.exe">NUL
if "%ERRORLEVEL%"=="0" (
    echo [!] Whisper server is already running
) else (
    echo [*] Starting Whisper server on port %WHISPER_PORT%...
    start /B "" whisper-server.exe ^
        --model "models\ggml-%WHISPER_MODEL%.bin" ^
        --host "%WHISPER_HOST%" ^
        --port "%WHISPER_PORT%" ^
        --print-progress > logs\whisper-server.log 2>&1
    
    timeout /t 2 /nobreak >nul
    
    tasklist /FI "IMAGENAME eq whisper-server.exe" 2>NUL | find /I /N "whisper-server.exe">NUL
    if "%ERRORLEVEL%"=="0" (
        echo [+] Whisper server started successfully
    ) else (
        echo [!] Failed to start Whisper server
        echo     Check logs\whisper-server.log for details
    )
)

echo.

tasklist /FI "IMAGENAME eq meeting-minutes-backend.exe" 2>NUL | find /I /N "meeting-minutes-backend.exe">NUL
if "%ERRORLEVEL%"=="0" (
    echo [!] Backend API is already running
) else (
    echo [*] Starting Backend API on port %BACKEND_PORT%...
    
    REM Set environment variables for backend
    set WHISPER_SERVER_URL=http://%WHISPER_HOST%:%WHISPER_PORT%
    
    start /B "" meeting-minutes-backend.exe > logs\backend.log 2>&1
    
    timeout /t 3 /nobreak >nul
    
    tasklist /FI "IMAGENAME eq meeting-minutes-backend.exe" 2>NUL | find /I /N "meeting-minutes-backend.exe">NUL
    if "%ERRORLEVEL%"=="0" (
        echo [+] Backend API started successfully
    ) else (
        echo [!] Failed to start Backend API
        echo     Check logs\backend.log for details
    )
)

echo.
echo ===============================================
echo Services are running:
echo   - Whisper Server: http://%WHISPER_HOST%:%WHISPER_PORT%
echo   - Backend API: http://%BACKEND_HOST%:%BACKEND_PORT%
echo   - Web Interface: http://%BACKEND_HOST%:%BACKEND_PORT%/docs
echo.
echo Press Ctrl+C to stop all services
echo ===============================================
echo.

if not defined STARTUP_MODE (
    REM Keep window open if not in startup mode
    pause >nul
)

REM Monitor services
:monitor_loop
timeout /t 5 /nobreak >nul

REM Check if services are still running
tasklist /FI "IMAGENAME eq whisper-server.exe" 2>NUL | find /I /N "whisper-server.exe">NUL
if not "%ERRORLEVEL%"=="0" (
    echo [!] Whisper server stopped unexpectedly
    if defined STARTUP_MODE goto restart_whisper
)

tasklist /FI "IMAGENAME eq meeting-minutes-backend.exe" 2>NUL | find /I /N "meeting-minutes-backend.exe">NUL
if not "%ERRORLEVEL%"=="0" (
    echo [!] Backend API stopped unexpectedly
    if defined STARTUP_MODE goto restart_backend
)

goto monitor_loop

:restart_whisper
echo [*] Attempting to restart Whisper server...
start /B "" whisper-server.exe ^
    --model "models\ggml-%WHISPER_MODEL%.bin" ^
    --host "%WHISPER_HOST%" ^
    --port "%WHISPER_PORT%" ^
    --print-progress >> logs\whisper-server.log 2>&1
timeout /t 3 /nobreak >nul
goto monitor_loop

:restart_backend
echo [*] Attempting to restart Backend API...
start /B "" meeting-minutes-backend.exe >> logs\backend.log 2>&1
timeout /t 3 /nobreak >nul
goto monitor_loop

:stop_services
echo [*] Stopping services...
taskkill /F /IM whisper-server.exe 2>nul
taskkill /F /IM meeting-minutes-backend.exe 2>nul
echo [+] Services stopped
exit /b 0

:restart_services
call :stop_services
timeout /t 2 /nobreak >nul
goto start_services

:check_status
echo Checking service status...
echo.

tasklist /FI "IMAGENAME eq whisper-server.exe" 2>NUL | find /I /N "whisper-server.exe">NUL
if "%ERRORLEVEL%"=="0" (
    echo [+] Whisper server: RUNNING
) else (
    echo [-] Whisper server: STOPPED
)

tasklist /FI "IMAGENAME eq meeting-minutes-backend.exe" 2>NUL | find /I /N "meeting-minutes-backend.exe">NUL
if "%ERRORLEVEL%"=="0" (
    echo [+] Backend API: RUNNING
) else (
    echo [-] Backend API: STOPPED
)

exit /b 0

:show_help
echo Meeting Minutes Backend Launcher
echo.
echo Usage: %~nx0 [options]
echo.
echo Options:
echo   --startup    Run in startup mode (minimized)
echo   --stop       Stop all services
echo   --restart    Restart all services
echo   --status     Check service status
echo   --help       Show this help message
echo.
echo Without options, starts services in interactive mode.
exit /b 0

:error_exit
echo.
echo [!] An error occurred. Press any key to exit...
pause >nul
exit /b 1