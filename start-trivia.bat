@echo off
REM Trivia App Complete Startup Script for Windows
REM Starts both servers and exposes them for multiplayer access

setlocal enabledelayedexpansion

REM Configuration
set BACKEND_PORT=3001
set FRONTEND_PORT=3000
set USE_NGROK=false

REM Parse command line arguments
:parse_args
if "%~1"=="" goto start_app
if "%~1"=="--help" goto show_help
if "%~1"=="--ngrok" (
    set USE_NGROK=true
    shift
    goto parse_args
)
echo Unknown option: %~1
goto show_help

:show_help
echo Usage: %0 [OPTIONS]
echo.
echo Starts trivia game servers and exposes them for multiplayer access.
echo.
echo Options:
echo   --ngrok       Also create public internet tunnels
echo   --help        Show this help message
echo.
echo Examples:
echo   %0            # Start servers with local network access
echo   %0 --ngrok    # Start servers + create public tunnels
goto end

:start_app
echo Starting Trivia Game Server...
echo.

REM Check prerequisites
where cargo >nul 2>nul
if errorlevel 1 (
    echo ERROR: Rust/Cargo is not installed. Please install from https://rustup.rs/
    goto end
)

where npm >nul 2>nul
if errorlevel 1 (
    echo ERROR: Node.js/npm is not installed. Please install from https://nodejs.org/
    goto end
)

REM Kill existing processes on our ports
echo Cleaning up existing processes...
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :3000') do taskkill /PID %%a /F >nul 2>nul
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :3001') do taskkill /PID %%a /F >nul 2>nul

REM Start backend server
echo Starting backend server...
cd backend
if not exist .env (
    echo Creating .env file with dummy token...
    echo SHOPIFY_API_TOKEN=dummy_token_for_development > .env
)

echo Backend will be accessible on all network interfaces (0.0.0.0:%BACKEND_PORT%)
start /b cmd /c "set HOST=0.0.0.0 && set SHOPIFY_API_TOKEN=dummy && cargo run"
cd ..

echo Waiting for backend to start...
timeout /t 5 >nul

REM Start frontend server
echo Starting frontend server...
cd frontend

echo Frontend will be accessible on all network interfaces (0.0.0.0:%FRONTEND_PORT%)
start /b cmd /c "set HOST=0.0.0.0 && set PORT=%FRONTEND_PORT% && npm start"
cd ..

echo Waiting for frontend to start...
timeout /t 10 >nul

echo.
echo Trivia Game Server is running!
echo.

echo Local Access:
echo   Game URL: http://localhost:%FRONTEND_PORT%
echo.

echo Network Access (share with others on your WiFi):
echo   Find your IP with: ipconfig
echo   Then share: http://[YOUR-IP]:%FRONTEND_PORT%
echo   Backend: http://[YOUR-IP]:%BACKEND_PORT% (auto-detected by frontend)
echo.

if "%USE_NGROK%"=="true" (
    where ngrok >nul 2>nul
    if errorlevel 1 (
        echo ERROR: ngrok is not installed. Please install from https://ngrok.com/
    ) else (
        echo Setting up ngrok tunnels for both frontend and backend...
        start /b ngrok http %FRONTEND_PORT%
        start /b ngrok http %BACKEND_PORT%
        timeout /t 5 >nul
        echo Public Internet Access:
        echo   Frontend dashboard: http://localhost:4040
        echo   Backend dashboard: http://localhost:4041
        echo   Share the HTTPS frontend URL with anyone on the internet!
        echo.
    )
) else (
    echo Want public internet access? Run: %0 --ngrok
    echo.
)

echo Your computer is now the game server!
echo Players can join by visiting the game URL
echo Press any key to stop the server...
pause >nul

REM Cleanup
echo Shutting down servers...
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :3000') do taskkill /PID %%a /F >nul 2>nul
for /f "tokens=5" %%a in ('netstat -aon ^| findstr :3001') do taskkill /PID %%a /F >nul 2>nul
taskkill /IM ngrok.exe /F >nul 2>nul
echo Cleanup completed

:end
endlocal 