@echo off
setlocal EnableExtensions
rem Launches the IEEE 1815.2 Test Tool: seeds per-user data under
rem %LOCALAPPDATA%, points the backend at it via env vars set only when
rem the user has not already set them, then runs web_server.exe in the
rem foreground so Ctrl+C and the console window are the stop mechanism.

set "APP_DIR=%~dp0"
set "DATA_ROOT=%LOCALAPPDATA%\ieee-1815-2-test-tool"

if not exist "%APP_DIR%bin\web_server.exe" (
  call :fail "web_server.exe not found under %APP_DIR%bin. Reinstall the tool."
  exit /b 1
)

if not exist "%DATA_ROOT%" mkdir "%DATA_ROOT%" >nul 2>&1
if not exist "%DATA_ROOT%" (call :fail "could not create %DATA_ROOT%." & exit /b 1)

if not exist "%DATA_ROOT%\logs" mkdir "%DATA_ROOT%\logs" >nul 2>&1
if not exist "%DATA_ROOT%\logs" (call :fail "could not create %DATA_ROOT%\logs." & exit /b 1)

rem robocopy refreshes the read-only seed data on every launch, so an
rem upgrade's new profiles reach the user's data root.
rem The redirect only silences robocopy's per-file listing; its exit
rem code is still checked below (0-7 success, 8+ failure).
robocopy "%APP_DIR%data" "%DATA_ROOT%\data" /E /XD working >nul
if %ERRORLEVEL% GEQ 8 (call :fail "seeding data into %DATA_ROOT%\data failed, robocopy exit code %ERRORLEVEL%." & exit /b 1)

if not exist "%DATA_ROOT%\data\working" mkdir "%DATA_ROOT%\data\working" >nul 2>&1
if not exist "%DATA_ROOT%\data\working" (call :fail "could not create %DATA_ROOT%\data\working." & exit /b 1)

rem Each var is set only if the user has not already set it, so a user
rem override always wins.
if not defined DATA_DIR set "DATA_DIR=%DATA_ROOT%\data"
if not defined FRONTEND_DIR set "FRONTEND_DIR=%APP_DIR%frontend"
if not defined OPENAPI_OUTPUT_PATH set "OPENAPI_OUTPUT_PATH=%DATA_ROOT%\openapi.json"
if not defined LOG_DIR set "LOG_DIR=%DATA_ROOT%\logs"
if not defined HOST set "HOST=127.0.0.1"
if not defined PORT set "PORT=8000"
if not defined REFERENCE_OUTSTATION_BIN set "REFERENCE_OUTSTATION_BIN=%APP_DIR%bin\reference-outstation.exe"
if not defined REFERENCE_CONTROL_STATION_BIN set "REFERENCE_CONTROL_STATION_BIN=%APP_DIR%bin\reference-control-station.exe"

rem cd to the data root so the backend's relative ./target/... binary
rem probes cannot pick up a stray build.
cd /d "%DATA_ROOT%"

if not defined TESTTOOL_NO_BROWSER (
  start "" /B cmd /c "timeout /t 3 /nobreak >nul & start http://127.0.0.1:%PORT%/"
)

"%APP_DIR%bin\web_server.exe"
set "SERVER_EXIT=%ERRORLEVEL%"
if not "%SERVER_EXIT%"=="0" (
  echo web_server exited with code %SERVER_EXIT%. See the log under "%LOG_DIR%".
  pause
)
exit /b %SERVER_EXIT%

:fail
rem Quoted so a path containing &, ^, ( or ) in %~1 is not re-parsed as a
rem command separator or block token; call :fail never returns to its
rem caller, since every call site exits right after it (exit /b in a
rem CALLed label only pops that call frame, it does not stop the script).
echo ERROR: "%~1"
pause
exit /b 1
