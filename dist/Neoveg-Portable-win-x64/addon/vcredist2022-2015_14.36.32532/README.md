# Visual C++ Redistributable prerequisite

The supported all-in-one prerequisite source is [TechPowerUp's Visual C++ Redistributable Runtime package](https://www.techpowerup.com/download/visual-c-redistributable-runtime-package-all-in-one/).

Download and extract the package so its `install_all.bat` and accompanying installers are in:

```text
addon\vcredist_all_in_one\
```

The installer also recognizes `addon\vcredist2022-2015_14.36.32532\`.

The Neoveg Windows bootstrap checks for the Visual C++ runtime DLLs used by its native components before startup. If one is missing, it requests administrator approval and runs `scripts\install_vcredist.bat`; that script invokes the package's `install_all.bat`, which silently installs the bundled runtimes (Visual C++ 2005–2022). The package itself is not included in this repository; do not rename or remove its accompanying files.

The legacy `.NET Framework 4.8` installer is not required for Neoveg's self-contained .NET application.
