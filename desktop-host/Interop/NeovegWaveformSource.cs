using System;
using System.Collections.Concurrent;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Threading.Tasks;

namespace Neoveg.Interop;

/// <summary>
/// Sample-accurate waveform data for the timeline (Rust FFI `waveform_source.rs`, managed fallback otherwise).
/// - <see cref="QueryColumns"/>: exact min/max/RMS of every pixel column (no interpolation).
/// - <see cref="ReadSamples"/>: raw samples for sample-level zoom.
/// Non-WAV media (MP3, MP4, MOV...) is first decoded to a cached PCM WAV with the bundled FFmpeg.
/// </summary>
public abstract class NeovegWaveformSource : IDisposable
{
    public const int ColumnStride = 6; // minL, maxL, rmsL, minR, maxR, rmsR
    public const int SummaryBlockFrames = 256;

    private static readonly ConcurrentDictionary<string, Task<NeovegWaveformSource?>> Cache = new(StringComparer.OrdinalIgnoreCase);

    public abstract int SampleRate { get; }
    public abstract long FrameCount { get; }

    /// <summary>Column c covers frames [start + c*framesPerColumn, start + (c+1)*framesPerColumn).</summary>
    public abstract void QueryColumns(double startFrame, double framesPerColumn, float[] output, int columns);

    /// <summary>Copies raw samples starting at <paramref name="startFrame"/>; out-of-range frames are 0.</summary>
    public abstract int ReadSamples(long startFrame, float[] left, float[] right, int count);

    public abstract void Dispose();

    /// <summary>
    /// Returns the source when ready, or null while it loads in the background (then <paramref name="onReady"/> is invoked).
    /// </summary>
    public static NeovegWaveformSource? GetOrBeginOpen(string mediaPath, RustCoreInterop? rustInterop, Action? onReady)
    {
        if (string.IsNullOrWhiteSpace(mediaPath) || !File.Exists(mediaPath))
            return null;

        bool created = false;
        var task = Cache.GetOrAdd(mediaPath, path =>
        {
            created = true;
            return Task.Run(() => OpenSafe(path, rustInterop));
        });

        if (task.IsCompletedSuccessfully)
            return task.Result;

        if (created && onReady != null)
        {
            task.ContinueWith(t =>
            {
                if (t.IsCompletedSuccessfully && t.Result != null)
                    Avalonia.Threading.Dispatcher.UIThread.Post(onReady, Avalonia.Threading.DispatcherPriority.Background);
            }, TaskScheduler.Default);
        }
        return null;
    }

    private static NeovegWaveformSource? OpenSafe(string mediaPath, RustCoreInterop? rustInterop)
    {
        try
        {
            string? wavPath = string.Equals(Path.GetExtension(mediaPath), ".wav", StringComparison.OrdinalIgnoreCase)
                ? mediaPath
                : TranscodeToCachedWav(mediaPath);
            if (wavPath == null)
                return null;

            var native = NativeWaveformSource.TryOpen(wavPath, rustInterop);
            if (native != null)
            {
                Program.LogEvent("INFO", "NeovegWaveformSource", $"Rust FFI waveform source opened: {wavPath} ({native.FrameCount} frames @ {native.SampleRate} Hz)");
                return native;
            }
            var managed = ManagedWaveformSource.TryOpen(wavPath);
            if (managed != null)
                Program.LogEvent("INFO", "NeovegWaveformSource", $"Managed fallback waveform source opened: {wavPath}");
            return managed;
        }
        catch (Exception ex)
        {
            Program.LogEvent("WARN", "NeovegWaveformSource.Open", $"{mediaPath}: {ex.Message}", ex);
            return null;
        }
    }

    /// <summary>Decodes any FFmpeg-readable media to a 16-bit PCM WAV in the waveform cache (reused while the source is unchanged).</summary>
    private static string? TranscodeToCachedWav(string mediaPath)
    {
        string? ffmpeg = ResolveFfmpegExecutable();
        if (ffmpeg == null)
        {
            Program.LogEvent("WARN", "NeovegWaveformSource", $"FFmpeg not found; precise waveform unavailable for {mediaPath}");
            return null;
        }

        var info = new FileInfo(mediaPath);
        string key = Convert.ToHexString(SHA1.HashData(Encoding.UTF8.GetBytes($"{info.FullName}|{info.Length}|{info.LastWriteTimeUtc.Ticks}")))[..16];
        string cacheDir = Path.Combine(ResolveCacheRoot(), "waveforms");
        Directory.CreateDirectory(cacheDir);
        string wavPath = Path.Combine(cacheDir, $"{key}.wav");
        if (File.Exists(wavPath) && new FileInfo(wavPath).Length > 44)
            return wavPath;

        string tmpPath = wavPath + ".tmp.wav";
        var psi = new ProcessStartInfo(ffmpeg)
        {
            UseShellExecute = false,
            CreateNoWindow = true,
            RedirectStandardError = true,
            RedirectStandardOutput = true
        };
        foreach (string arg in new[] { "-y", "-v", "error", "-i", mediaPath, "-vn", "-ac", "2", "-c:a", "pcm_s16le", "-f", "wav", tmpPath })
            psi.ArgumentList.Add(arg);

        using var proc = Process.Start(psi);
        if (proc == null) return null;
        string stderr = proc.StandardError.ReadToEnd();
        proc.WaitForExit();
        if (proc.ExitCode != 0 || !File.Exists(tmpPath))
        {
            Program.LogEvent("WARN", "NeovegWaveformSource", $"FFmpeg audio decode failed ({proc.ExitCode}) for {mediaPath}: {stderr.Trim()}");
            try { File.Delete(tmpPath); } catch { }
            return null;
        }
        File.Move(tmpPath, wavPath, overwrite: true);
        return wavPath;
    }

    private static string ResolveCacheRoot()
    {
        if (OperatingSystem.IsWindows())
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Neoveg");
        if (OperatingSystem.IsMacOS())
            return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), "Library", "Caches", "neoveg");
        return Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.UserProfile), ".cache", "neoveg");
    }

    private static string? ResolveFfmpegExecutable()
    {
        string exe = OperatingSystem.IsWindows() ? "ffmpeg.exe" : "ffmpeg";
        string rid = RustCoreInterop.DetectPlatformRid();
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null)
        {
            foreach (string candidate in new[]
                     {
                         Path.Combine(dir.FullName, exe),
                         Path.Combine(dir.FullName, "runtimes", rid, exe),
                         Path.Combine(dir.FullName, "runtimes", rid, "ffmpeg", "bin", exe),
                         Path.Combine(dir.FullName, "runtimes", rid, "ffmpeg", exe)
                     })
            {
                if (File.Exists(candidate)) return candidate;
            }
            dir = dir.Parent;
        }

        foreach (string p in (Environment.GetEnvironmentVariable("PATH") ?? "").Split(Path.PathSeparator, StringSplitOptions.RemoveEmptyEntries))
        {
            string candidate = Path.Combine(p.Trim(), exe);
            if (File.Exists(candidate)) return candidate;
        }
        return null;
    }
}

/// <summary>Rust FFI implementation (`neoveg_waveform_*` exports of neoveg_core).</summary>
internal sealed unsafe class NativeWaveformSource : NeovegWaveformSource
{
    private delegate IntPtr OpenFn(byte* path);
    private delegate int InfoFn(IntPtr handle, uint* sampleRate, ushort* channels, long* frames);
    private delegate int QueryFn(IntPtr handle, double startFrame, double framesPerColumn, int columns, float* output);
    private delegate int ReadFn(IntPtr handle, long startFrame, int count, float* left, float* right);
    private delegate void CloseFn(IntPtr handle);

    private static readonly object BindLock = new();
    private static bool _bindAttempted;
    private static OpenFn? _open;
    private static InfoFn? _info;
    private static QueryFn? _query;
    private static ReadFn? _read;
    private static CloseFn? _close;

    private IntPtr _handle;
    private readonly int _sampleRate;
    private readonly long _frames;

    private NativeWaveformSource(IntPtr handle, int sampleRate, long frames)
    {
        _handle = handle;
        _sampleRate = sampleRate;
        _frames = frames;
    }

    public override int SampleRate => _sampleRate;
    public override long FrameCount => _frames;

    public static NativeWaveformSource? TryOpen(string wavPath, RustCoreInterop? rustInterop)
    {
        if (!Bind(rustInterop) || _open == null || _info == null)
            return null;

        byte[] utf8 = Encoding.UTF8.GetBytes(wavPath + "\0");
        IntPtr handle;
        fixed (byte* p = utf8)
            handle = _open(p);
        if (handle == IntPtr.Zero)
            return null;

        uint rate = 0;
        ushort channels = 0;
        long frames = 0;
        if (_info(handle, &rate, &channels, &frames) != 1 || rate == 0)
        {
            _close?.Invoke(handle);
            return null;
        }
        return new NativeWaveformSource(handle, (int)rate, frames);
    }

    private static bool Bind(RustCoreInterop? rustInterop)
    {
        lock (BindLock)
        {
            if (_bindAttempted)
                return _open != null;
            _bindAttempted = true;

            if (rustInterop == null || !rustInterop.IsNativeLoaded || string.IsNullOrEmpty(rustInterop.LoadedLibraryPath))
                return false;
            if (!NativeLibrary.TryLoad(rustInterop.LoadedLibraryPath, out IntPtr lib))
                return false;

            if (NativeLibrary.TryGetExport(lib, "neoveg_waveform_open", out IntPtr pOpen) &&
                NativeLibrary.TryGetExport(lib, "neoveg_waveform_info", out IntPtr pInfo) &&
                NativeLibrary.TryGetExport(lib, "neoveg_waveform_query_columns", out IntPtr pQuery) &&
                NativeLibrary.TryGetExport(lib, "neoveg_waveform_read_samples", out IntPtr pRead) &&
                NativeLibrary.TryGetExport(lib, "neoveg_waveform_close", out IntPtr pClose))
            {
                _open = Marshal.GetDelegateForFunctionPointer<OpenFn>(pOpen);
                _info = Marshal.GetDelegateForFunctionPointer<InfoFn>(pInfo);
                _query = Marshal.GetDelegateForFunctionPointer<QueryFn>(pQuery);
                _read = Marshal.GetDelegateForFunctionPointer<ReadFn>(pRead);
                _close = Marshal.GetDelegateForFunctionPointer<CloseFn>(pClose);
                return true;
            }

            Program.LogEvent("WARN", "NativeWaveformSource", $"neoveg_waveform_* exports missing in {rustInterop.LoadedLibraryPath}; using managed fallback.");
            return false;
        }
    }

    public override void QueryColumns(double startFrame, double framesPerColumn, float[] output, int columns)
    {
        if (_handle == IntPtr.Zero || _query == null || columns <= 0 || output.Length < columns * ColumnStride)
            return;
        fixed (float* p = output)
            _query(_handle, startFrame, framesPerColumn, columns, p);
    }

    public override int ReadSamples(long startFrame, float[] left, float[] right, int count)
    {
        count = Math.Min(count, Math.Min(left.Length, right.Length));
        if (_handle == IntPtr.Zero || _read == null || count <= 0)
            return 0;
        fixed (float* pl = left)
        fixed (float* pr = right)
            return Math.Max(0, _read(_handle, startFrame, count, pl, pr));
    }

    public override void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            _close?.Invoke(_handle);
            _handle = IntPtr.Zero;
        }
    }
}

/// <summary>Managed fallback with the same algorithm as `waveform_source.rs` (raw PCM bytes + 256-frame summaries).</summary>
internal sealed class ManagedWaveformSource : NeovegWaveformSource
{
    private readonly byte[] _data;
    private readonly int _dataOffset;
    private readonly int _frameSize;
    private readonly int _bytesPerSample;
    private readonly int _rightOffset;
    private readonly bool _isFloat;
    private readonly int _sampleRate;
    private readonly long _frames;
    private readonly float[] _summary; // per block: minL, maxL, sumSqL, minR, maxR, sumSqR

    private ManagedWaveformSource(byte[] data, int dataOffset, long frames, int frameSize, int bytesPerSample, int rightOffset, bool isFloat, int sampleRate)
    {
        _data = data;
        _dataOffset = dataOffset;
        _frames = frames;
        _frameSize = frameSize;
        _bytesPerSample = bytesPerSample;
        _rightOffset = rightOffset;
        _isFloat = isFloat;
        _sampleRate = sampleRate;

        long blocks = (frames + SummaryBlockFrames - 1) / SummaryBlockFrames;
        _summary = new float[blocks * 6];
        for (long b = 0; b < blocks; b++)
        {
            long start = b * SummaryBlockFrames;
            long end = Math.Min(start + SummaryBlockFrames, frames);
            for (int ch = 0; ch < 2; ch++)
            {
                float min = float.MaxValue, max = float.MinValue;
                double sumSq = 0;
                for (long f = start; f < end; f++)
                {
                    float v = Sample(ch, f);
                    if (v < min) min = v;
                    if (v > max) max = v;
                    sumSq += v * (double)v;
                }
                _summary[b * 6 + ch * 3] = min;
                _summary[b * 6 + ch * 3 + 1] = max;
                _summary[b * 6 + ch * 3 + 2] = (float)sumSq;
            }
        }
    }

    public override int SampleRate => _sampleRate;
    public override long FrameCount => _frames;

    public static ManagedWaveformSource? TryOpen(string wavPath)
    {
        byte[] bytes = File.ReadAllBytes(wavPath);
        if (bytes.Length < 12 || Encoding.ASCII.GetString(bytes, 0, 4) != "RIFF" || Encoding.ASCII.GetString(bytes, 8, 4) != "WAVE")
            return null;

        int formatTag = 0, channels = 0, sampleRate = 0, bits = 0, blockAlign = 0;
        int dataStart = -1, dataEnd = -1;
        int pos = 12;
        while (pos + 8 <= bytes.Length)
        {
            string id = Encoding.ASCII.GetString(bytes, pos, 4);
            long size = BitConverter.ToUInt32(bytes, pos + 4);
            int bodyStart = pos + 8;
            int bodyEnd = size == 0 || size == uint.MaxValue ? bytes.Length : (int)Math.Min(bytes.Length, bodyStart + size);
            if (id == "fmt " && bodyEnd - bodyStart >= 16)
            {
                formatTag = BitConverter.ToUInt16(bytes, bodyStart);
                channels = BitConverter.ToUInt16(bytes, bodyStart + 2);
                sampleRate = BitConverter.ToInt32(bytes, bodyStart + 4);
                blockAlign = BitConverter.ToUInt16(bytes, bodyStart + 12);
                bits = BitConverter.ToUInt16(bytes, bodyStart + 14);
                if (formatTag == 0xFFFE && bodyEnd - bodyStart >= 26)
                    formatTag = BitConverter.ToUInt16(bytes, bodyStart + 24);
            }
            else if (id == "data")
            {
                dataStart = bodyStart;
                dataEnd = bodyEnd;
                if (channels != 0) break;
            }
            if (size == 0 || size == uint.MaxValue) break;
            pos = (int)Math.Min(int.MaxValue, bodyStart + size + (size & 1));
        }

        if (dataStart < 0 || channels == 0 || sampleRate == 0 || bits == 0 || (formatTag != 1 && formatTag != 3))
            return null;
        int bytesPerSample = (bits + 7) / 8;
        int frameSize = Math.Max(blockAlign, bytesPerSample * channels);
        long frames = (dataEnd - dataStart) / frameSize;
        return new ManagedWaveformSource(bytes, dataStart, frames, frameSize, bytesPerSample, channels > 1 ? bytesPerSample : 0, formatTag == 3, sampleRate);
    }

    private float Sample(int channel, long frame)
    {
        int o = (int)(_dataOffset + frame * _frameSize + (channel == 0 ? 0 : _rightOffset));
        float v = (_bytesPerSample, _isFloat) switch
        {
            (4, true) => BitConverter.ToSingle(_data, o),
            (8, true) => (float)BitConverter.ToDouble(_data, o),
            (1, false) => (_data[o] - 128f) / 128f,
            (2, false) => BitConverter.ToInt16(_data, o) / 32768f,
            (3, false) => (((_data[o] | (_data[o + 1] << 8) | (_data[o + 2] << 16)) << 8) >> 8) / 8388608f,
            (4, false) => BitConverter.ToInt32(_data, o) / 2147483648f,
            _ => 0f
        };
        return float.IsFinite(v) ? Math.Clamp(v, -1f, 1f) : 0f;
    }

    public override void QueryColumns(double startFrame, double framesPerColumn, float[] output, int columns)
    {
        double fpc = Math.Max(1e-9, framesPerColumn);
        for (int c = 0; c < columns && (c + 1) * ColumnStride <= output.Length; c++)
        {
            double a = startFrame + c * fpc;
            double b = a + fpc;
            long first = (long)Math.Max(0, Math.Floor(a));
            long last = (long)Math.Max(0, Math.Max(Math.Ceiling(b), Math.Floor(a) + 1));
            for (int ch = 0; ch < 2; ch++)
            {
                float min = 0, max = 0, rms = 0;
                if (first < _frames && b > 0)
                    (min, max, rms) = Scan(ch, first, Math.Min(last, _frames));
                output[c * ColumnStride + ch * 3] = min;
                output[c * ColumnStride + ch * 3 + 1] = max;
                output[c * ColumnStride + ch * 3 + 2] = rms;
            }
        }
    }

    private (float Min, float Max, float Rms) Scan(int ch, long first, long last)
    {
        float min = float.MaxValue, max = float.MinValue;
        double sumSq = 0;
        long count = 0;
        long f = first;
        while (f < last)
        {
            long block = f / SummaryBlockFrames;
            long blockStart = block * SummaryBlockFrames;
            long blockEnd = Math.Min(blockStart + SummaryBlockFrames, _frames);
            if (f == blockStart && blockEnd <= last)
            {
                long s = block * 6 + ch * 3;
                min = Math.Min(min, _summary[s]);
                max = Math.Max(max, _summary[s + 1]);
                sumSq += _summary[s + 2];
                count += blockEnd - blockStart;
                f = blockEnd;
            }
            else
            {
                long end = Math.Min(blockEnd, last);
                for (; f < end; f++)
                {
                    float v = Sample(ch, f);
                    if (v < min) min = v;
                    if (v > max) max = v;
                    sumSq += v * (double)v;
                    count++;
                }
            }
        }
        return count == 0 ? (0, 0, 0) : (min, max, (float)Math.Sqrt(sumSq / count));
    }

    public override int ReadSamples(long startFrame, float[] left, float[] right, int count)
    {
        count = Math.Min(count, Math.Min(left.Length, right.Length));
        int copied = 0;
        for (int i = 0; i < count; i++)
        {
            long f = startFrame + i;
            bool inside = f >= 0 && f < _frames;
            left[i] = inside ? Sample(0, f) : 0f;
            right[i] = inside ? Sample(1, f) : 0f;
            if (inside) copied++;
        }
        return copied;
    }

    public override void Dispose()
    {
    }
}
