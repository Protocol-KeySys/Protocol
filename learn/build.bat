@echo off
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
if errorlevel 1 (
    echo Could not find vcvars64.bat - is Build Tools installed?
    exit /b 1
)
cl /nologo /O1 /W3 /Fe:keygen.exe keygen.c
cl /nologo /O1 /W3 /Fe:app.exe app.c
