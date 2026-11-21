@echo off
rem Launches the Protocol keyserver + client app. Lives at repo root.
cd /d "%~dp0server"
start "" target\release\protocol-keyserver.exe
start "" "%~dp0client\src-tauri\target\release\protocol-keysystem.exe"
