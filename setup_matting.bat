@echo off
cd /d "%~dp0"
where py >nul 2>nul
if %errorlevel%==0 (
  py setup_matting.py %*
) else (
  python setup_matting.py %*
)
pause
