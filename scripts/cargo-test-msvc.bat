@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"
set PATH=%USERPROFILE%\.cargo\bin;%PATH%
cd /d "E:\我的git项目\Github\cc-switch-doctor"
cargo test --manifest-path src-tauri\Cargo.toml --lib
