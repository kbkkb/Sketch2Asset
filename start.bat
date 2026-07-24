@echo off
setlocal
cd /d "%~dp0"

echo ===================================================
echo   Sketch2Asset Server Launcher
echo ===================================================
echo.

set PY_CMD=
where py >nul 2>nul
if %errorlevel%==0 (
    set PY_CMD=py
) else (
    where python >nul 2>nul
    if %errorlevel%==0 (
        set PY_CMD=python
    )
)

if "%PY_CMD%"=="" (
    echo [ERROR] Python is not installed or not in PATH!
    echo Please install Python 3.8+ from https://www.python.org/
    echo.
    pause
    exit /b
)

echo [1/2] Checking Python runtime dependencies...
"%PY_CMD%" -c "import requests, PIL, numpy, onnxruntime" >nul 2>nul

if %errorlevel% neq 0 (
    echo [!] Missing dependencies, installing automatically via Aliyun mirror...
    echo.
    "%PY_CMD%" -m pip install -r requirements.txt -i https://mirrors.aliyun.com/pypi/simple/ --trusted-host mirrors.aliyun.com
    if %errorlevel% neq 0 (
        echo [!] Aliyun mirror failed, trying Tsinghua mirror...
        "%PY_CMD%" -m pip install -r requirements.txt -i https://pypi.tuna.tsinghua.edu.cn/simple
    )
    echo.
    echo [OK] Dependencies installation completed!
) else (
    echo [OK] Dependencies are ready.
)

echo.
echo [2/2] Starting Sketch2Asset local Web server...
echo Browser will automatically open: http://127.0.0.1:8000
echo ===================================================
echo.

"%PY_CMD%" server.py %*

if %errorlevel% neq 0 (
    echo.
    echo [ERROR] Server exited with error.
)
pause
