# DirectX End-User Runtimes (June 2010) — Neoveg Prerequisite

- **Official Download Source:** [TechPowerUp DirectX Redistributable Runtime](https://www.techpowerup.com/download/directx-redistributable-runtime/)
- **Expected Installer File:** `directx_Jun2010_redist.exe`
- **Automated Silent Installation Command (`scripts/install_prerequisites.bat`):**
  ```bat
  directx_Jun2010_redist.exe /q /t:%TEMP%\dxredist && %TEMP%\dxredist\DXSETUP.exe /silent
  ```
- **Verified Runtime Libraries on Boot (`logs/neoveg.log`):**
  - `d3d11.dll` / `dxgi.dll` (Direct3D 11 Hardware Visual Acceleration)
  - `XAudio2_7.dll` / `XAudio2_9.dll` (DirectX Low-Latency Audio DSP Acceleration)
