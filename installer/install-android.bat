@echo off
rem maid-cafe-se Android installer launcher (double-click to run). Arguments are passed to install-android.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install-android.ps1" %*
if errorlevel 1 (
  echo.
  echo Installation failed. See the message above.
)
pause
