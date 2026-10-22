@echo off
SET SEE_MASK_NOZONECHECKS=1
start "" "%~dp0dist\Neoveg-Portable-win-x64\Neoveg.exe" %*
