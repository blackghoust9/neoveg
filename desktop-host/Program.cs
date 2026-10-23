using System;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading.Tasks;
using Avalonia;

namespace Neoveg;

internal static class Program
{
    private const long MaxLogFileBytes = 10L * 1024L * 1024L; // 10 MB per log file
    private const int MaxHistoricalLogFiles = 5;
    private static readonly object LogSyncLock = new();

    public static string GoogleDriveSyncRoot => @"G:\Meu Drive\vp-rev-eng\teste\teste2";
    public static string LogsDirectory => Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "logs");
    public static string ActiveLogPath => Path.Combine(LogsDirectory, "neoveg.log");
    public static string CrashLogPath => Path.Combine(LogsDirectory, "crash.log");
    public static string LegacyRootCrashLogPath => Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "crash.log");

    /// <summary>
    /// Platform-agnostic root cache directory resolved via `OperatingSystem.IsWindows()`, `OperatingSystem.IsLinux()`, and `OperatingSystem.IsMacOS()`:
    /// - Windows (`OperatingSystem.IsWindows()`): `%LOCALAPPDATA%\Neoveg\`
    /// - Linux   (`OperatingSystem.IsLinux()`):   `~/.cache/neoveg/`
    /// - macOS   (`OperatingSystem.IsMacOS()`):   `~/Library/Caches/neoveg/`
    /// </summary>
    public static string PlatformCacheRoot
    {
        get
        {
            if (OperatingSystem.IsWindows())
            {
                string localAppData = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
                if (string.IsNullOrWhiteSpace(localAppData))
                    localAppData = Environment.GetEnvironmentVariable("LOCALAPPDATA") ?? AppContext.BaseDirectory;
                return Path.Combine(localAppData, "Neoveg");
            }

            string homeDir = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            if (string.IsNullOrWhiteSpace(homeDir))
                homeDir = Environment.GetEnvironmentVariable("HOME") ?? AppContext.BaseDirectory;

            if (OperatingSystem.IsMacOS())
            {
                return Path.Combine(homeDir, "Library", "Caches", "neoveg");
            }

            if (OperatingSystem.IsLinux())
            {
                return Path.Combine(homeDir, ".cache", "neoveg");
            }

            return Path.Combine(homeDir, ".cache", "neoveg");
        }
    }

    public static string LocalAppCacheDirectory =>
        OperatingSystem.IsWindows()
            ? Path.Combine(PlatformCacheRoot, "AppCache")
            : Path.Combine(PlatformCacheRoot, "app_cache");

    public static string PlatformUserLogsDirectory =>
        Path.Combine(PlatformCacheRoot, "logs");

    /// <summary>
    /// Returns the native FFI shared library extension for the active Operating System:
    /// - `.dll` on Windows (`OperatingSystem.IsWindows()`)
    /// - `.so` on Linux (`OperatingSystem.IsLinux()`)
    /// - `.dylib` on macOS (`OperatingSystem.IsMacOS()`)
    /// </summary>
    public static string GetNativeSharedLibraryExtension()
    {
        if (OperatingSystem.IsWindows())
            return ".dll";
        if (OperatingSystem.IsLinux())
            return ".so";
        if (OperatingSystem.IsMacOS())
            return ".dylib";
        throw new PlatformNotSupportedException($"Native Neoveg libraries are not configured for {RuntimeInformation.OSDescription}.");
    }

    /// <summary>
    /// Resolves the expected Rust Core (`neoveg_core`) and C++ Engine (`neoveg_cpp_engine`) FFI library filenames
    /// for the active Operating System (`.dll` on Windows, `.so` on Linux, `.dylib` on macOS).
    /// </summary>
    public static string[] ResolvePlatformNativeLibraryNames()
    {
        if (OperatingSystem.IsWindows())
            return new[] { "neoveg_core.dll", "neoveg_cpp_engine.dll", "WebView2Loader.dll" };
        if (OperatingSystem.IsLinux())
            return new[] { "libneoveg_core.so", "libneoveg_cpp_engine.so" };
        if (OperatingSystem.IsMacOS())
            return new[] { "libneoveg_core.dylib", "libneoveg_cpp_engine.dylib" };
        throw new PlatformNotSupportedException($"Native Neoveg libraries are not configured for {RuntimeInformation.OSDescription}.");
    }

    public static string GetPlatformRid()
    {
        bool isArm64 = RuntimeInformation.ProcessArchitecture == Architecture.Arm64;
        if (OperatingSystem.IsWindows())
            return isArm64 ? "win-arm64" : "win-x64";
        if (OperatingSystem.IsLinux())
            return isArm64 ? "linux-arm64" : "linux-x64";
        if (OperatingSystem.IsMacOS())
            return isArm64 ? "osx-arm64" : "osx-x64";
        throw new PlatformNotSupportedException($"Neoveg native libraries are not configured for {RuntimeInformation.OSDescription}.");
    }

    [STAThread]
    public static void Main(string[] args)
    {
        InitializeLoggingSubsystem();
        RegisterNativeLibraryResolver();
        RegisterGlobalExceptionHandlers();
        LogOperatingSystemAndFfiEnvironment();
        VerifyPreFlightEnvironmentAndToolchains();

        if (TryRelocateFromCloudDriveToLocalAppCache(args))
        {
            Environment.Exit(0);
            return;
        }

        ValidateNativeLibraries();

        if (OperatingSystem.IsWindows())
        {
            if (!EnsureVisualCppRuntime())
                return;

            VerifyDirectXVisualAndAudioAcceleration();
        }

        try
        {
            LogEvent("INFO", "Program.Main", $"Starting Neoveg from '{AppDomain.CurrentDomain.BaseDirectory}' (PID={Environment.ProcessId})");
            BuildAvaloniaApp().StartWithClassicDesktopLifetime(args);
            LogEvent("INFO", "Program.Main", "Neoveg exited normally.");
        }
        catch (Exception ex)
        {
            RxApp.DefaultExceptionHandler.OnNext(ex);
            WriteCrashLog("Program.Main.FatalStartupException", ex, isTerminating: true);
            throw;
        }
    }

    private static void RegisterNativeLibraryResolver()
    {
        NativeLibrary.SetDllImportResolver(typeof(Program).Assembly, ResolveNativeLibrary);
        LogEvent("INFO", "NativeLibraryResolver", "Registered Neoveg native DLL resolver.");
    }

    private static IntPtr ResolveNativeLibrary(
        string libraryName,
        System.Reflection.Assembly assembly,
        DllImportSearchPath? searchPath)
    {
        string libraryStem = Path.GetFileNameWithoutExtension(libraryName);
        if (libraryStem.StartsWith("lib", StringComparison.OrdinalIgnoreCase))
            libraryStem = libraryStem.Substring(3);

        string? fileName = libraryStem.ToLowerInvariant() switch
        {
            "neoveg_core" => ResolvePlatformNativeLibraryNames()[0],
            "neoveg_cpp_engine" => ResolvePlatformNativeLibraryNames()[1],
            _ => null
        };

        if (fileName == null)
            return IntPtr.Zero;

        string applicationDirectory = AppDomain.CurrentDomain.BaseDirectory;
        string[] candidates =
        {
            Path.Combine(applicationDirectory, fileName),
            Path.Combine(LocalAppCacheDirectory, fileName),
            Path.Combine(applicationDirectory, "runtimes", GetPlatformRid(), "native", fileName)
        };

        foreach (string candidate in candidates.Distinct(StringComparer.OrdinalIgnoreCase))
        {
            if (!File.Exists(candidate))
                continue;

            try
            {
                if (NativeLibrary.TryLoad(candidate, out IntPtr handle))
                {
                    LogEvent("INFO", "NativeLibraryResolver", $"Resolved '{libraryName}' from '{candidate}'.");
                    return handle;
                }
            }
            catch (Exception ex) when (ex is DllNotFoundException or BadImageFormatException or FileLoadException)
            {
                LogEvent("WARN", "NativeLibraryResolver", $"Failed to load '{libraryName}' from '{candidate}'.", ex);
            }
        }

        LogEvent(
            "WARN",
            "NativeLibraryResolver",
            $"Could not resolve '{libraryName}' in the application directory or AppCache: {string.Join(" | ", candidates)}.");
        return IntPtr.Zero;
    }

    private static void ValidateNativeLibraries()
    {
        string[] nativeLibraries = ResolvePlatformNativeLibraryNames().Take(2).ToArray();
        string[] searchDirectories =
        {
            AppDomain.CurrentDomain.BaseDirectory,
            LocalAppCacheDirectory,
            Path.Combine(AppDomain.CurrentDomain.BaseDirectory, "runtimes", GetPlatformRid(), "native")
        };

        foreach (string library in nativeLibraries)
        {
            string? resolvedPath = searchDirectories
                .Select(directory => Path.Combine(directory, library))
                .FirstOrDefault(File.Exists);

            if (resolvedPath == null)
            {
                string message =
                    $"Required native library '{library}' was not found beside Neoveg.exe or in AppCache. " +
                    $"Searched: {string.Join(" | ", searchDirectories.Select(directory => Path.Combine(directory, library)))}.";
                LogEvent("ERROR", "NativeLibraryValidation", message);
                throw new FileNotFoundException(message, library);
            }

            LogEvent("INFO", "NativeLibraryValidation", $"Native library '{library}' is available at '{resolvedPath}'.");
        }
    }

    private static bool EnsureVisualCppRuntime()
    {
        if (!OperatingSystem.IsWindows())
            return true;

        string systemDirectory = Environment.GetFolderPath(Environment.SpecialFolder.System);
        string[] requiredRuntimeDlls =
        {
            "vcruntime140.dll",
            "vcruntime140_1.dll",
            "msvcp140.dll",
            "msvcp140_1.dll"
        };
        string[] missingDlls = requiredRuntimeDlls
            .Where(dll => !File.Exists(Path.Combine(systemDirectory, dll)))
            .ToArray();

        if (missingDlls.Length == 0)
        {
            LogEvent("INFO", "VisualCppPrerequisite", "Required Visual C++ runtime DLLs are present.");
            return true;
        }

        string applicationDirectory = AppContext.BaseDirectory;
        string installerPath = Path.Combine(applicationDirectory, "scripts", "install_vcredist.bat");
        string? packageInstallerPath = new[]
        {
            Path.Combine(applicationDirectory, "addon", "vcredist_all_in_one", "install_all.bat"),
            Path.Combine(applicationDirectory, "addon", "vcredist2022-2015_14.36.32532", "install_all.bat")
        }.FirstOrDefault(File.Exists);
        if (!File.Exists(installerPath) || packageInstallerPath == null)
        {
            string message = "Faltam componentes do Visual C++ necessários para iniciar o Neoveg: " +
                             string.Join(", ", missingDlls) +
                             Environment.NewLine + Environment.NewLine +
                             "Baixe o pacote Visual C++ Redistributable All-in-One da TechPowerUp e extraia-o em " +
                             "addon\\vcredist_all_in_one ou addon\\vcredist2022-2015_14.36.32532 ao lado do aplicativo.";
            LogEvent("ERROR", "VisualCppPrerequisite", message);
            MessageBox(IntPtr.Zero, message, "Neoveg — Pré-requisito ausente", 0x10);
            return false;
        }

        try
        {
            LogEvent("WARN", "VisualCppPrerequisite",
                $"Missing runtime DLL(s): {string.Join(", ", missingDlls)}. Requesting administrator approval to run the installer.");

            var startInfo = new ProcessStartInfo
            {
                FileName = installerPath,
                Arguments = "/elevated",
                WorkingDirectory = applicationDirectory,
                UseShellExecute = true,
                Verb = "runas"
            };
            using Process? installer = Process.Start(startInfo);
            if (installer == null)
            {
                LogEvent("ERROR", "VisualCppPrerequisite", "Windows did not start the Visual C++ prerequisite installer.");
                return false;
            }

            installer.WaitForExit();
            missingDlls = requiredRuntimeDlls
                .Where(dll => !File.Exists(Path.Combine(systemDirectory, dll)))
                .ToArray();
            if (missingDlls.Length == 0)
            {
                LogEvent("INFO", "VisualCppPrerequisite", "Visual C++ runtime installation verified.");
                return true;
            }

            string remaining = "A instalação não disponibilizou todos os componentes necessários: " +
                               string.Join(", ", missingDlls);
            LogEvent("ERROR", "VisualCppPrerequisite", $"{remaining} (installer exit code {installer.ExitCode}).");
            MessageBox(IntPtr.Zero, remaining, "Neoveg — Falha nos pré-requisitos", 0x10);
            return false;
        }
        catch (Win32Exception ex) when (ex.NativeErrorCode == 1223)
        {
            LogEvent("WARN", "VisualCppPrerequisite", "Administrator approval was declined; Neoveg startup was cancelled.", ex);
            return false;
        }
        catch (Exception ex)
        {
            LogEvent("ERROR", "VisualCppPrerequisite", "Failed to run the Visual C++ prerequisite installer.", ex);
            MessageBox(IntPtr.Zero, ex.Message, "Neoveg — Falha nos pré-requisitos", 0x10);
            return false;
        }
    }

    [DllImport("user32.dll", EntryPoint = "MessageBoxW", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern int MessageBox(IntPtr hWnd, string text, string caption, uint type);

    /// <summary>
    /// Verifies DirectX Visual & Audio Hardware Acceleration runtime DLLs (`d3d11.dll`, `dxgi.dll`, `XAudio2_7.dll`, `XAudio2_9.dll`)
    /// in both C# (`NativeLibrary.TryLoad` / `System32`) and C++ (`neoveg_cpp_verify_directx_runtime`),
    /// logging the diagnostic status to `logs/neoveg.log` during boot.
    /// </summary>
    public static void VerifyDirectXVisualAndAudioAcceleration()
    {
        try
        {
            if (!OperatingSystem.IsWindows())
            {
                LogEvent("INFO", "DirectXDiagnostic", "Non-Windows platform detected; DirectX check bypassed (Vulkan/Metal/OpenGL/ALSA/CoreAudio active).");
                return;
            }

            string systemDirectory = Environment.GetFolderPath(Environment.SpecialFolder.System);
            bool hasD3D11 = ProbeSystemLibrary(systemDirectory, "d3d11.dll");
            bool hasDxgi = ProbeSystemLibrary(systemDirectory, "dxgi.dll");
            bool hasXAudio29 = ProbeSystemLibrary(systemDirectory, "XAudio2_9.dll");
            bool hasXAudio27 = ProbeSystemLibrary(systemDirectory, "XAudio2_7.dll") ||
                               ProbeSystemLibrary(systemDirectory, "XAudio2_8.dll") ||
                               hasXAudio29;

            // If XAudio2_7.dll (DirectX June 2010 Redist) is missing and directx_Jun2010_redist.exe is staged in addon/,
            // attempt silent installation via scripts/install_prerequisites.bat
            if (!hasXAudio27)
            {
                string appDir = AppContext.BaseDirectory;
                string prereqBat = Path.Combine(appDir, "scripts", "install_prerequisites.bat");
                string dxRedistExe = Path.Combine(appDir, "addon", "directx_redist", "directx_Jun2010_redist.exe");
                if (File.Exists(prereqBat) && File.Exists(dxRedistExe))
                {
                    LogEvent("INFO", "DirectXDiagnostic", "XAudio2_7.dll missing and directx_Jun2010_redist.exe found; running scripts/install_prerequisites.bat.");
                    try
                    {
                        using var proc = Process.Start(new ProcessStartInfo
                        {
                            FileName = prereqBat,
                            WorkingDirectory = appDir,
                            UseShellExecute = true,
                            Verb = "runas"
                        });
                        proc?.WaitForExit(30000);
                        hasXAudio27 = ProbeSystemLibrary(systemDirectory, "XAudio2_7.dll");
                    }
                    catch (Exception ex)
                    {
                        LogEvent("WARN", "DirectXDiagnostic", $"DirectX June 2010 silent setup skipped: {ex.Message}");
                    }
                }
            }

            bool visualReady = hasD3D11 && hasDxgi;
            bool audioReady = hasXAudio27 || hasXAudio29;
            string level = (visualReady && audioReady) ? "INFO" : "WARN";

            LogEvent(
                level,
                "DirectXDiagnostic",
                $"DirectX Visual & Audio Acceleration Boot Check: " +
                $"d3d11.dll={(hasD3D11 ? "ACCESSIBLE" : "MISSING")}, " +
                $"dxgi.dll={(hasDxgi ? "ACCESSIBLE" : "MISSING")}, " +
                $"XAudio2_7.dll={(hasXAudio27 ? "ACCESSIBLE" : "MISSING (Install DirectX June 2010 Runtimes: https://www.techpowerup.com/download/directx-redistributable-runtime/)")}, " +
                $"XAudio2_9.dll={(hasXAudio29 ? "ACCESSIBLE" : "MISSING")} | " +
                $"VisualGPU={(visualReady ? "READY" : "FALLBACK")}, AudioDSP={(audioReady ? "READY" : "FALLBACK")}");

            // Trigger C++ Engine (`neoveg_cpp_engine`) DirectX diagnostic verification so C++ also logs its boot check to logs/neoveg.log
            try
            {
                var cppInterop = new Interop.CppCoreInterop(AppContext.BaseDirectory);
                string cppDxJson = cppInterop.VerifyDirectXRuntimeJson();
                LogEvent("INFO", "DirectXDiagnostic.CppBridge", $"C++ Engine DirectX verification report: {cppDxJson}");
            }
            catch (Exception cppEx)
            {
                LogEvent("WARN", "DirectXDiagnostic.CppBridge", $"C++ DirectX probe skipped: {cppEx.Message}");
            }
        }
        catch (Exception ex)
        {
            LogEvent("WARN", "DirectXDiagnostic", $"DirectX diagnostic check encountered an error: {ex.Message}", ex);
        }
    }

    private static bool ProbeSystemLibrary(string systemDirectory, string dllName)
    {
        try
        {
            string fullPath = Path.Combine(systemDirectory, dllName);
            if (File.Exists(fullPath))
            {
                if (NativeLibrary.TryLoad(fullPath, out IntPtr handle))
                {
                    NativeLibrary.Free(handle);
                }
                return true;
            }
            if (NativeLibrary.TryLoad(dllName, out IntPtr sysHandle))
            {
                NativeLibrary.Free(sysHandle);
                return true;
            }
        }
        catch
        {
        }
        return false;
    }

    /// <summary>
    /// Section 9 — Local App Cache & Cloud Drive Execution Protocol:
    /// Detects if `AppDomain.CurrentDomain.BaseDirectory` is on a cloud or virtual drive (`G:\`, `G:\Meu Drive`, OneDrive, UNC `\\`),
    /// synchronizes the application binaries (`Neoveg.exe`, native DLLs, `Assets/`, `scripts/`) into `%LOCALAPPDATA%\Neoveg\AppCache\`,
    /// launches the cached executable preserving command-line arguments, and exits the initial cloud-drive process cleanly.
    /// </summary>
    public static bool TryRelocateFromCloudDriveToLocalAppCache(string[] args)
    {
        try
        {
            if (args.Any(a => string.Equals(a, "--no-local-cache", StringComparison.OrdinalIgnoreCase)) ||
                string.Equals(Environment.GetEnvironmentVariable("NEOVEG_RELOCATED_FROM_CACHE"), "1", StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            string sourceDir = AppDomain.CurrentDomain.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar, Path.AltDirectorySeparatorChar);
            if (!IsCloudOrVirtualDrivePath(sourceDir))
            {
                return false;
            }

            string cacheDir = LocalAppCacheDirectory;
            if (string.Equals(
                    Path.GetFullPath(sourceDir).TrimEnd(Path.DirectorySeparatorChar),
                    Path.GetFullPath(cacheDir).TrimEnd(Path.DirectorySeparatorChar),
                    StringComparison.OrdinalIgnoreCase))
            {
                return false;
            }

            LogEvent(
                "INFO",
                "LocalAppCache",
                $"Cloud/Virtual Drive execution detected at '{sourceDir}'. Verifying Local App Cache at '{cacheDir}'.");

            int syncedCount = SynchronizeApplicationToLocalCache(sourceDir, cacheDir);

            string exeName = OperatingSystem.IsWindows() ? "Neoveg.exe" : "Neoveg";
            string cachedExePath = Path.Combine(cacheDir, exeName);
            if (!File.Exists(cachedExePath))
            {
                LogEvent("WARN", "LocalAppCache", $"Cached executable not found at '{cachedExePath}'. Continuing in-place execution.");
                return false;
            }

            LogEvent(
                "INFO",
                "LocalAppCache",
                $"Redirecting execution from Cloud Drive ('{sourceDir}') to Local App Cache ('{cachedExePath}', {syncedCount} file(s) updated).");

            var psi = new ProcessStartInfo
            {
                FileName = cachedExePath,
                WorkingDirectory = cacheDir,
                UseShellExecute = false
            };
            foreach (string arg in args)
            {
                psi.ArgumentList.Add(arg);
            }
            psi.Environment["NEOVEG_RELOCATED_FROM_CACHE"] = "1";
            psi.Environment["NEOVEG_SOURCE_DRIVE_DIR"] = sourceDir;

            Process.Start(psi);
            return true;
        }
        catch (Exception ex)
        {
            LogEvent("WARN", "LocalAppCache", $"Failed to relocate execution to Local App Cache (continuing in-place): {ex.Message}", ex);
            return false;
        }
    }

    public static bool IsCloudOrVirtualDrivePath(string path)
    {
        if (string.IsNullOrWhiteSpace(path))
            return false;

        string norm = path.Replace('/', '\\');
        if (norm.StartsWith(@"\\", StringComparison.OrdinalIgnoreCase))
            return true;
        if (norm.StartsWith(@"G:\", StringComparison.OrdinalIgnoreCase))
            return true;
        if (norm.Contains(@"\Meu Drive", StringComparison.OrdinalIgnoreCase) ||
            norm.Contains(@"\My Drive", StringComparison.OrdinalIgnoreCase) ||
            norm.Contains(@"\Google Drive", StringComparison.OrdinalIgnoreCase) ||
            norm.Contains(@"\OneDrive", StringComparison.OrdinalIgnoreCase))
        {
            return true;
        }
        return false;
    }

    private static int SynchronizeApplicationToLocalCache(string sourceDir, string cacheDir)
    {
        Directory.CreateDirectory(cacheDir);
        string exeName = OperatingSystem.IsWindows() ? "Neoveg.exe" : "Neoveg";
        string stampFile = Path.Combine(cacheDir, "appcache_sync.stamp");
        string currentFingerprint = ComputeFastCacheFingerprint(sourceDir, exeName);
        if (!string.IsNullOrEmpty(currentFingerprint) &&
            File.Exists(Path.Combine(cacheDir, exeName)) &&
            File.Exists(stampFile))
        {
            try
            {
                string cachedStamp = File.ReadAllText(stampFile, Encoding.UTF8).Trim();
                if (string.Equals(cachedStamp, currentFingerprint, StringComparison.Ordinal))
                {
                    return 0; // Instantaneous <2ms fast-path when Local AppCache is already up-to-date
                }
            }
            catch
            {
            }
        }

        int copiedFiles = 0;

        // 1. Copy root binaries and runtime configuration files (.exe, .dll, .so, .dylib, .json, .pdb, .ico)
        var allowedRootExtensions = new[] { ".exe", ".dll", ".so", ".dylib", ".json", ".pdb", ".ico", ".config" };
        foreach (string srcFile in Directory.GetFiles(sourceDir))
        {
            string ext = Path.GetExtension(srcFile);
            if (!allowedRootExtensions.Contains(ext, StringComparer.OrdinalIgnoreCase))
                continue;

            string dstFile = Path.Combine(cacheDir, Path.GetFileName(srcFile));
            if (CopyFileIfOutdated(srcFile, dstFile))
                copiedFiles++;
        }

        // 2. Copy subdirectories required for standalone execution (Assets, scripts, runtimes)
        foreach (string subFolder in new[] { "Assets", "scripts", "runtimes", "addon" })
        {
            string srcSub = Path.Combine(sourceDir, subFolder);
            if (!Directory.Exists(srcSub))
            {
                string parentCandidate = Path.Combine(sourceDir, "..", subFolder);
                if (Directory.Exists(parentCandidate))
                    srcSub = Path.GetFullPath(parentCandidate);
            }

            if (Directory.Exists(srcSub))
            {
                copiedFiles += CopyDirectoryIfOutdated(srcSub, Path.Combine(cacheDir, subFolder));
            }
        }

        // 3. Ensure native FFI libraries (.dll on Windows, .so on Linux, .dylib on macOS) are present in cacheDir root
        string platformRid = GetPlatformRid();
        foreach (string nativeLib in ResolvePlatformNativeLibraryNames())
        {
            string dstNative = Path.Combine(cacheDir, nativeLib);
            var candidates = new List<string>
            {
                Path.Combine(sourceDir, nativeLib),
                Path.Combine(sourceDir, "runtimes", platformRid, "native", nativeLib)
            };
            // The Google Drive mirror is a Windows drive-letter path; it is meaningless on Linux/macOS.
            if (OperatingSystem.IsWindows())
            {
                candidates.Add(Path.Combine(GoogleDriveSyncRoot, "runtimes", platformRid, "native", nativeLib));
            }
            foreach (string cand in candidates)
            {
                if (File.Exists(cand))
                {
                    if (CopyFileIfOutdated(cand, dstNative))
                        copiedFiles++;
                    break;
                }
            }
        }

        try
        {
            if (!string.IsNullOrEmpty(currentFingerprint))
                File.WriteAllText(stampFile, currentFingerprint, Encoding.UTF8);
        }
        catch
        {
        }

        return copiedFiles;
    }

    private static string ComputeFastCacheFingerprint(string sourceDir, string exeName)
    {
        try
        {
            var sb = new StringBuilder();
            foreach (string probe in new[] { exeName, "Neoveg.dll" }.Concat(ResolvePlatformNativeLibraryNames()))
            {
                var fi = new FileInfo(Path.Combine(sourceDir, probe));
                if (fi.Exists)
                {
                    sb.Append($"{probe}:{fi.Length}:{fi.LastWriteTimeUtc.Ticks};");
                }
            }
            return sb.ToString();
        }
        catch
        {
            return "";
        }
    }

    private static int CopyDirectoryIfOutdated(string srcDir, string dstDir)
    {
        Directory.CreateDirectory(dstDir);
        int count = 0;
        foreach (string file in Directory.GetFiles(srcDir))
        {
            string dstFile = Path.Combine(dstDir, Path.GetFileName(file));
            if (CopyFileIfOutdated(file, dstFile))
                count++;
        }
        foreach (string subDir in Directory.GetDirectories(srcDir))
        {
            string dirName = Path.GetFileName(subDir);
            if (string.Equals(dirName, "__pycache__", StringComparison.OrdinalIgnoreCase) ||
                string.Equals(dirName, ".git", StringComparison.OrdinalIgnoreCase))
            {
                continue;
            }
            count += CopyDirectoryIfOutdated(subDir, Path.Combine(dstDir, dirName));
        }
        return count;
    }

    private static bool CopyFileIfOutdated(string srcFile, string dstFile)
    {
        try
        {
            var srcInfo = new FileInfo(srcFile);
            if (!srcInfo.Exists)
                return false;

            var dstInfo = new FileInfo(dstFile);
            if (!dstInfo.Exists ||
                srcInfo.Length != dstInfo.Length ||
                Math.Abs((srcInfo.LastWriteTimeUtc - dstInfo.LastWriteTimeUtc).TotalSeconds) > 1.0)
            {
                string? parent = Path.GetDirectoryName(dstFile);
                if (!string.IsNullOrEmpty(parent))
                    Directory.CreateDirectory(parent);
                File.Copy(srcFile, dstFile, overwrite: true);
                return true;
            }
        }
        catch
        {
            // Ignore locked files already running in cache
        }
        return false;
    }

    private static void InitializeLoggingSubsystem()
    {
        try
        {
            Directory.CreateDirectory(LogsDirectory);
            Directory.CreateDirectory(PlatformUserLogsDirectory);
            RotateLogsIfNeeded();
            if (OperatingSystem.IsWindows() && Directory.Exists(GoogleDriveSyncRoot))
            {
                Directory.CreateDirectory(Path.Combine(GoogleDriveSyncRoot, "logs"));
            }
        }
        catch
        {
            // Non-fatal if directory is read-only
        }
    }

    /// <summary>
    /// Logs the detected Operating System (`OperatingSystem.IsWindows()`, `OperatingSystem.IsLinux()`, `OperatingSystem.IsMacOS()`),
    /// platform-agnostic cache/log directories, and expected FFI library extensions (`.dll` / `.so` / `.dylib`).
    /// </summary>
    public static void LogOperatingSystemAndFfiEnvironment()
    {
        string osName = OperatingSystem.IsWindows()
            ? "Windows"
            : OperatingSystem.IsLinux()
                ? "Linux"
                : OperatingSystem.IsMacOS()
                    ? "macOS"
                    : "POSIX";
        string ffiExt = GetNativeSharedLibraryExtension();
        string[] expectedLibs = ResolvePlatformNativeLibraryNames();
        LogEvent(
            "INFO",
            "OSPlatformBootstrap",
            $"OS={osName} (RID={GetPlatformRid()}) | CacheRoot='{PlatformCacheRoot}' | AppCache='{LocalAppCacheDirectory}' | " +
            $"LogsDir='{LogsDirectory}' | UserLogs='{PlatformUserLogsDirectory}' | FFIExtension='{ffiExt}' ({string.Join(", ", expectedLibs)})");
    }

    /// <summary>
    /// Section 13 — Developer Environment & Dependency Enforcement Protocol:
    /// Verifies presence of required runtimes (.NET 8 runtime/self-contained, VC++ Redistributable on Windows,
    /// system shared libraries on Linux/macOS) and developer toolchains prior to application launch,
    /// logging diagnostic reports with actionable resolution steps (`REQUIREMENTS.md`, `scripts/check_env.py`) to `logs/neoveg.log`.
    /// </summary>
    public static void VerifyPreFlightEnvironmentAndToolchains()
    {
        try
        {
            bool isNet8OrNewer = Environment.Version.Major >= 8;
            string frameworkDesc = RuntimeInformation.FrameworkDescription;

            bool platformLibsReady = true;
            string platformNote = "System libraries verified";
            if (OperatingSystem.IsWindows())
            {
                string sysDir = Environment.GetFolderPath(Environment.SpecialFolder.System);
                bool hasVcRuntime = ProbeSystemLibrary(sysDir, "vcruntime140.dll") && ProbeSystemLibrary(sysDir, "msvcp140.dll");
                platformLibsReady = hasVcRuntime;
                if (!hasVcRuntime)
                {
                    platformNote = "Missing Visual C++ Redistributable (vcruntime140.dll / msvcp140.dll). Run .\\scripts\\install_prerequisites.bat as Administrator or consult REQUIREMENTS.md.";
                }
            }
            else if (OperatingSystem.IsLinux())
            {
                bool hasLibX11 = NativeLibrary.TryLoad("libX11.so.6", out IntPtr x11Handle);
                if (hasLibX11) NativeLibrary.Free(x11Handle);
                bool hasLibGl = NativeLibrary.TryLoad("libGL.so.1", out IntPtr glHandle);
                if (hasLibGl) NativeLibrary.Free(glHandle);
                platformLibsReady = hasLibX11 && hasLibGl;
                if (!platformLibsReady)
                {
                    platformNote = "Missing Linux graphics/windowing libraries (libX11/libGL). Run: sudo apt install libgl1-mesa-dev libx11-dev libasound2-dev (see REQUIREMENTS.md).";
                }
            }
            else if (OperatingSystem.IsMacOS())
            {
                platformNote = "macOS CoreFoundation/Metal/CoreAudio runtime active (see REQUIREMENTS.md for Xcode CLI & Homebrew setup).";
            }

            string level = (isNet8OrNewer && platformLibsReady) ? "INFO" : "WARN";
            LogEvent(
                level,
                "PreFlightEnvironmentCheck",
                $"Runtime='{frameworkDesc}' (.NET8+={isNet8OrNewer}) | PlatformRuntimeLibs={(platformLibsReady ? "OK" : "MISSING")} ({platformNote}) | " +
                "Specs='REQUIREMENTS.md' & 'python-backend/requirements.txt' | DiagnosticTool='python scripts/check_env.py' / '.\\scripts\\check_env.ps1'");
        }
        catch (Exception ex)
        {
            LogEvent("WARN", "PreFlightEnvironmentCheck", $"Pre-flight environment verification warning: {ex.Message}", ex);
        }
    }

    private static void RotateLogsIfNeeded()
    {
        try
        {
            var activeInfo = new FileInfo(ActiveLogPath);
            if (activeInfo.Exists && activeInfo.Length >= MaxLogFileBytes)
            {
                string datedName = $"neoveg.{DateTime.Now:yyyy-MM-dd_HHmmss}.log";
                File.Move(ActiveLogPath, Path.Combine(LogsDirectory, datedName), overwrite: true);
            }

            var historicalLogs = new DirectoryInfo(LogsDirectory)
                .GetFiles("neoveg.*.log")
                .OrderByDescending(f => f.LastWriteTimeUtc)
                .ToList();

            for (int i = MaxHistoricalLogFiles; i < historicalLogs.Count; i++)
            {
                try { historicalLogs[i].Delete(); } catch { }
            }
        }
        catch
        {
        }
    }

    private static void RegisterGlobalExceptionHandlers()
    {
        AppDomain.CurrentDomain.UnhandledException += (_, e) =>
        {
            var ex = e.ExceptionObject as Exception
                     ?? new Exception($"Unhandled non-CLR exception object: {e.ExceptionObject}");
            LogEvent("FATAL", "AppDomain.CurrentDomain.UnhandledException", ex.Message, ex);
            WriteCrashLog("AppDomain.CurrentDomain.UnhandledException", ex, e.IsTerminating);
        };

        TaskScheduler.UnobservedTaskException += (_, e) =>
        {
            LogEvent("ERROR", "TaskScheduler.UnobservedTaskException", e.Exception.Message, e.Exception);
            WriteCrashLog("TaskScheduler.UnobservedTaskException", e.Exception, isTerminating: false);
            e.SetObserved();
        };
    }

    /// <summary>
    /// Writes a structured `[DEBUG]` / `[INFO]` / `[WARN]` / `[ERROR]` / `[FATAL]` entry to `logs/neoveg.log`,
    /// `logs/neoveg.YYYY-MM-DD.log`, Console output (`Serilog` / `Microsoft.Extensions.Logging` sink format),
    /// and synchronizes with `G:\Meu Drive\vp-rev-eng\teste\teste2\logs\neoveg.log`.
    /// </summary>
    public static void LogEvent(string level, string source, string message, Exception? ex = null)
    {
        try
        {
            string ts = DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss.fff");
            string normLevel = (level ?? "INFO").Trim().ToUpperInvariant();
            var sb = new StringBuilder();
            sb.Append($"[{ts}] [{normLevel}] [C#/{source}] {message}");
            if (ex != null)
            {
                sb.Append($" | Exception={ex.GetType().FullName}: {ex.Message}");
            }
            string line = sb.ToString();

            Console.WriteLine(line);

            lock (LogSyncLock)
            {
                Directory.CreateDirectory(LogsDirectory);
                RotateLogsIfNeeded();
                File.AppendAllText(ActiveLogPath, line + Environment.NewLine, Encoding.UTF8);

                string dailyLogPath = Path.Combine(LogsDirectory, $"neoveg.{DateTime.Now:yyyy-MM-dd}.log");
                File.AppendAllText(dailyLogPath, line + Environment.NewLine, Encoding.UTF8);

                SyncLogLineToGoogleDrive("neoveg.log", line + Environment.NewLine);
            }
        }
        catch
        {
            // Never throw inside the logger
        }
    }

    public static void WriteCrashLog(string source, Exception? ex, bool isTerminating = false)
    {
        try
        {
            var sb = new StringBuilder();
            sb.AppendLine("================================================================================");
            sb.AppendLine($"[{DateTime.Now:yyyy-MM-dd HH:mm:ss.fff}] [FATAL] NEOVEG CRASH / EXCEPTION REPORT");
            sb.AppendLine($"Source        : {source}");
            sb.AppendLine($"IsTerminating : {isTerminating}");
            sb.AppendLine($"BaseDirectory : {AppDomain.CurrentDomain.BaseDirectory}");
            sb.AppendLine($"CurrentDir    : {Environment.CurrentDirectory}");
            sb.AppendLine($"OS / Arch     : {RuntimeInformation.OSDescription} ({RuntimeInformation.ProcessArchitecture})");
            sb.AppendLine($"Framework     : {RuntimeInformation.FrameworkDescription}");
            if (ex != null)
            {
                sb.AppendLine($"ExceptionType : {ex.GetType().FullName}");
                sb.AppendLine($"Message       : {ex.Message}");
                sb.AppendLine("StackTrace    :");
                sb.AppendLine(ex.ToString());
            }
            sb.AppendLine("================================================================================");
            string payload = sb.ToString();

            lock (LogSyncLock)
            {
                Directory.CreateDirectory(LogsDirectory);
                File.AppendAllText(CrashLogPath, payload, Encoding.UTF8);
                File.AppendAllText(LegacyRootCrashLogPath, payload, Encoding.UTF8);
                SyncLogLineToGoogleDrive("crash.log", payload);
            }
        }
        catch
        {
            // Never throw inside the crash logger itself
        }
    }

    private static void SyncLogLineToGoogleDrive(string logFileName, string content)
    {
        try
        {
            string? targetRoot = (OperatingSystem.IsWindows() && Directory.Exists(GoogleDriveSyncRoot))
                ? GoogleDriveSyncRoot
                : Environment.GetEnvironmentVariable("NEOVEG_SOURCE_DRIVE_DIR");

            if (string.IsNullOrWhiteSpace(targetRoot) || !Directory.Exists(targetRoot))
                return;

            string gdriveLogsDir = Path.Combine(targetRoot, "logs");
            Directory.CreateDirectory(gdriveLogsDir);
            string gdriveLogFile = Path.Combine(gdriveLogsDir, logFileName);

            if (!string.Equals(
                    Path.GetFullPath(gdriveLogFile),
                    Path.GetFullPath(Path.Combine(LogsDirectory, logFileName)),
                    StringComparison.OrdinalIgnoreCase))
            {
                File.AppendAllText(gdriveLogFile, content, Encoding.UTF8);
            }
        }
        catch
        {
        }
    }

    public static AppBuilder BuildAvaloniaApp() =>
        AppBuilder.Configure<App>()
            .UsePlatformDetect()
            .WithInterFont()
            .LogToTrace();
}
