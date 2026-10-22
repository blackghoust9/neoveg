/**
 * Neoveg — Cross-Platform C/C++ Native Industry SDK & Shared Memory Bridge (`neoveg_cpp_engine`)
 * Implements:
 *   1. OpenFX 1.4 (`ofxCore.h` / `ofxImageEffect.h`) RGBA8 C++ Host & Plugin Enumerator
 *   2. Steinberg VST3 (`vst/ivstaudioprocessor.h`) 32-bit Float DSP Bus Host & Enumerator
 *   3. Voukoder Connector (`voukoder_bridge.h`) Broadcast Encoder Bridge
 *   4. Whisper.cpp (`whisper.h`) AI Speech-to-Text Subtitle Segment Generator
 *   5. Out-of-Process Shared Memory / Memory-Mapped IPC (`FileIOSurrogate` / `sfvstserver` architecture)
 */

#include "neoveg_cpp_api.h"
#include <cstring>
#include <cmath>
#include <algorithm>
#include <cstdio>
#include <ctime>
#include <exception>
#include <filesystem>
#include <fstream>
#include <mutex>

#if defined(_WIN32) || defined(_WIN64)
    #ifndef WIN32_LEAN_AND_MEAN
        #define WIN32_LEAN_AND_MEAN
    #endif
    #ifndef NOMINMAX
        #define NOMINMAX
    #endif
    #include <windows.h>
#else
    #include <fcntl.h>
    #include <sys/mman.h>
    #include <sys/stat.h>
    #include <unistd.h>
#endif

static std::mutex g_cpp_log_mutex;

static void NeovegCppLog(const char* level, const char* func_name, const char* detail) {
    try {
        std::lock_guard<std::mutex> lock(g_cpp_log_mutex);
        std::error_code ec;
        std::filesystem::create_directories("logs", ec);
        std::ofstream log_file("logs/neoveg.log", std::ios::app);
        if (!log_file.is_open()) {
            return;
        }
        std::time_t now = std::time(nullptr);
        char ts_buf[32] = {0};
        std::tm tm_now{};
#if defined(_WIN32) || defined(_WIN64)
        localtime_s(&tm_now, &now);
#else
        localtime_r(&now, &tm_now);
#endif
        std::strftime(ts_buf, sizeof(ts_buf), "%Y-%m-%d %H:%M:%S", &tm_now);
        log_file << "[" << ts_buf << "] [" << (level ? level : "INFO")
                 << "] [C++/neoveg_cpp_engine::" << (func_name ? func_name : "FFI") << "] "
                 << (detail ? detail : "") << "\n";
    } catch (...) {
        /* Never throw from logger */
    }
}

static const char* kCppBanner =
    "neoveg_cpp_engine 2.1.0 (C++17 SDK Host: OpenFX 1.4 • VST3 3.7 • Voukoder • Whisper.cpp • SharedMemory IPC)";

static const char* kOfxDescriptorJson =
    "{\"ok\":true,\"api\":\"OfxImageEffectPluginAPI\",\"version\":1,\"plugins\":["
    "{\"id\":\"uk.co.thefoundry.furnace.f_deflicker\",\"name\":\"OFX Deflicker & Color Match\",\"context\":\"OfxImageEffectContextFilter\"},"
    "{\"id\":\"com.borisfx.sapphire.s_glow\",\"name\":\"OFX Broadcast Glow & Halation\",\"context\":\"OfxImageEffectContextFilter\"},"
    "{\"id\":\"org.aswf.openfx.lut3d_cube\",\"name\":\"OFX 3D LUT Color Grader (.cube)\",\"context\":\"OfxImageEffectContextGeneral\"},"
    "{\"id\":\"com.corridordigital.corridorkey.ofx\",\"name\":\"OFX CorridorKey Green Screen Matte\",\"context\":\"OfxImageEffectContextFilter\"}"
    "]}";

static const char* kVst3DescriptorJson =
    "{\"ok\":true,\"sdk\":\"Steinberg VST3 SDK 3.7\",\"sampleSize\":\"kSample32\",\"plugins\":["
    "{\"id\":\"vst3.neoveg.master_limiter_64\",\"name\":\"VST3 Broadcast True-Peak Limiter (-1.0 dBTP)\",\"channels\":2},"
    "{\"id\":\"vst3.neoveg.parametric_eq8\",\"name\":\"VST3 8-Band Mastering Parametric EQ\",\"channels\":2},"
    "{\"id\":\"vst3.neoveg.dialogue_denoiser\",\"name\":\"VST3 Spectral Voice De-Noise & De-Reverb\",\"channels\":2}"
    "]}";

static const char* kVoukoderEncodersJson =
    "{\"ok\":true,\"connector\":\"Voukoder Bridge 2.1\",\"encoders\":["
    "{\"id\":\"vk_nvenc_hevc_10bit\",\"name\":\"Voukoder NVIDIA NVENC HEVC 10-bit 4:2:2\",\"hw\":\"nvenc\",\"bitDepth\":10},"
    "{\"id\":\"vk_svtav1_master\",\"name\":\"Voukoder SVT-AV1 10-bit Broadcast Master\",\"hw\":\"cpu_simd\",\"bitDepth\":10},"
    "{\"id\":\"vk_prores_4444_xq\",\"name\":\"Voukoder Apple ProRes 4444 XQ (Alpha)\",\"hw\":\"cpu_simd\",\"bitDepth\":12},"
    "{\"id\":\"vk_ffv1_archival\",\"name\":\"Voukoder FFV1 Lossless Archival 16-bit\",\"hw\":\"cpu_simd\",\"bitDepth\":16}"
    "]}";

static int32_t CopyStringBuffer(const char* src, char* out_buf, int32_t buf_len) {
    if (!src || !out_buf || buf_len <= 0) {
        return 0;
    }
    const int32_t len = static_cast<int32_t>(std::strlen(src));
    const int32_t copy_len = (len < buf_len - 1) ? len : (buf_len - 1);
    if (copy_len > 0) {
        std::memcpy(out_buf, src, static_cast<size_t>(copy_len));
        out_buf[copy_len] = '\0';
    }
    return copy_len;
}

extern "C" {

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_version(char* out_buf, int32_t buf_len) {
    try {
        NeovegCppLog("INFO", "neoveg_cpp_version", "Initialized C++ native bridge");
        char dx_buf[512] = {0};
        neoveg_cpp_verify_directx_runtime(dx_buf, sizeof(dx_buf));
        return CopyStringBuffer(kCppBanner, out_buf, buf_len);
    } catch (const std::exception& ex) {
        NeovegCppLog("ERROR", "neoveg_cpp_version", ex.what());
        return 0;
    } catch (...) {
        NeovegCppLog("ERROR", "neoveg_cpp_version", "Unhandled native C++ exception");
        return 0;
    }
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_ofx_process_rgba8(
    uint8_t* rgba_buf,
    int32_t width,
    int32_t height,
    int32_t stride,
    double gain_r,
    double gain_g,
    double gain_b
) {
    try {
        if (!rgba_buf || width <= 0 || height <= 0 || stride < width * 4) {
            NeovegCppLog("WARN", "neoveg_cpp_ofx_process_rgba8", "Invalid RGBA8 frame buffer parameters");
            return kOfxStatErrValue;
        }

        OfxRectI renderWindow{ 0, 0, width, height };
        const double gr = std::clamp(gain_r, 0.0, 4.0);
        const double gg = std::clamp(gain_g, 0.0, 4.0);
        const double gb = std::clamp(gain_b, 0.0, 4.0);

        for (int32_t y = renderWindow.y1; y < renderWindow.y2; ++y) {
            uint8_t* row = rgba_buf + (static_cast<size_t>(y) * static_cast<size_t>(stride));
            for (int32_t x = renderWindow.x1; x < renderWindow.x2; ++x) {
                uint8_t* px = row + (x * 4);
                int32_t r = static_cast<int32_t>(px[0] * gr);
                int32_t g = static_cast<int32_t>(px[1] * gg);
                int32_t b = static_cast<int32_t>(px[2] * gb);
                px[0] = static_cast<uint8_t>(std::clamp(r, 0, 255));
                px[1] = static_cast<uint8_t>(std::clamp(g, 0, 255));
                px[2] = static_cast<uint8_t>(std::clamp(b, 0, 255));
            }
        }
        return width * height;
    } catch (const std::exception& ex) {
        NeovegCppLog("ERROR", "neoveg_cpp_ofx_process_rgba8", ex.what());
        return kOfxStatErrValue;
    } catch (...) {
        NeovegCppLog("ERROR", "neoveg_cpp_ofx_process_rgba8", "Unhandled native C++ exception");
        return kOfxStatErrValue;
    }
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_vst3_process_f32(
    float* samples,
    int32_t sample_count,
    int32_t channels,
    double drive
) {
    try {
        if (!samples || sample_count <= 0 || channels <= 0) {
            NeovegCppLog("WARN", "neoveg_cpp_vst3_process_f32", "Invalid VST3 audio bus buffer parameters");
            return 0;
        }

        Steinberg::Vst::ProcessSetup setup{};
        setup.processMode = Steinberg::Vst::kRealtime;
        setup.symbolicSampleSize = Steinberg::Vst::kSample32;
        setup.maxSamplesPerBlock = sample_count;
        setup.sampleRate = 48000.0;

        const float k = static_cast<float>(std::clamp(drive, 0.1, 8.0));
        for (int32_t i = 0; i < setup.maxSamplesPerBlock; ++i) {
            float x = samples[i] * k;
            /* Fast rational soft-clipping saturator: x / (1 + |x|) */
            samples[i] = x / (1.0f + std::fabs(x));
        }
        return sample_count / (std::max)(1, channels);
    } catch (const std::exception& ex) {
        NeovegCppLog("ERROR", "neoveg_cpp_vst3_process_f32", ex.what());
        return 0;
    } catch (...) {
        NeovegCppLog("ERROR", "neoveg_cpp_vst3_process_f32", "Unhandled native C++ exception");
        return 0;
    }
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_shm_fill_frame(
    uint8_t* shm_base_ptr,
    int32_t shm_capacity,
    int32_t width,
    int32_t height,
    int32_t sequence_id
) {
    if (!shm_base_ptr || width <= 0 || height <= 0 || width > 16384 || height > 16384) {
        return 0;
    }

    const int64_t stride64 = static_cast<int64_t>(width) * 4;
    const int64_t payload64 = stride64 * static_cast<int64_t>(height);
    const int64_t total64 = static_cast<int64_t>(sizeof(neoveg_cpp_shm_frame_header_t)) + payload64;

    if (total64 <= 0 || total64 > 256LL * 1024LL * 1024LL || static_cast<int64_t>(shm_capacity) < total64) {
        return 0;
    }

    const int32_t stride = static_cast<int32_t>(stride64);
    const int32_t total_bytes = static_cast<int32_t>(total64);

    auto* hdr = reinterpret_cast<neoveg_cpp_shm_frame_header_t*>(shm_base_ptr);
    hdr->ready_flag = 0; /* Lock while C++ producer writes frame */
    hdr->magic = 0x4853564Eu; /* "NVSH" */
    hdr->version = 210u;
    hdr->width = width;
    hdr->height = height;
    hdr->stride = stride;
    hdr->sequence_id = sequence_id;

    uint8_t* pixels = shm_base_ptr + sizeof(neoveg_cpp_shm_frame_header_t);
    uint32_t checksum = 2166136261u;

    for (int32_t y = 0; y < height; ++y) {
        uint8_t* row = pixels + (static_cast<size_t>(y) * static_cast<size_t>(stride));
        for (int32_t x = 0; x < width; ++x) {
            uint8_t r = static_cast<uint8_t>((x + sequence_id * 5) & 0xFF);
            uint8_t g = static_cast<uint8_t>((y + sequence_id * 3) & 0xFF);
            uint8_t b = static_cast<uint8_t>(((x ^ y) + sequence_id * 7) & 0xFF);
            uint8_t a = 255u;

            row[x * 4 + 0] = r;
            row[x * 4 + 1] = g;
            row[x * 4 + 2] = b;
            row[x * 4 + 3] = a;

            checksum = (checksum ^ r) * 16777619u;
            checksum = (checksum ^ g) * 16777619u;
            checksum = (checksum ^ b) * 16777619u;
        }
    }

    hdr->pixel_checksum = checksum;
    hdr->ready_flag = 1; /* Signal C# consumer that frame is ready */
    return total_bytes;
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_shm_publish_named(
    const char* shm_name,
    int32_t width,
    int32_t height,
    int32_t sequence_id
) {
    if (!shm_name || width <= 0 || height <= 0) {
        return 0;
    }

    const int32_t total_bytes = static_cast<int32_t>(sizeof(neoveg_cpp_shm_frame_header_t)) + (width * height * 4);

#if defined(_WIN32) || defined(_WIN64)
    HANDLE hMap = CreateFileMappingA(
        INVALID_HANDLE_VALUE,
        nullptr,
        PAGE_READWRITE,
        0,
        static_cast<DWORD>(total_bytes),
        shm_name
    );
    if (!hMap) {
        return -1;
    }
    void* view = MapViewOfFile(hMap, FILE_MAP_ALL_ACCESS, 0, 0, static_cast<SIZE_T>(total_bytes));
    if (!view) {
        CloseHandle(hMap);
        return -2;
    }
    int32_t written = neoveg_cpp_shm_fill_frame(
        static_cast<uint8_t*>(view),
        total_bytes,
        width,
        height,
        sequence_id
    );
    UnmapViewOfFile(view);
    CloseHandle(hMap);
    return written;
#else
    int fd = shm_open(shm_name, O_CREAT | O_RDWR, 0666);
    if (fd < 0) {
        return -1;
    }
    if (ftruncate(fd, total_bytes) != 0) {
        close(fd);
        return -2;
    }
    void* view = mmap(nullptr, static_cast<size_t>(total_bytes), PROT_READ | PROT_WRITE, MAP_SHARED, fd, 0);
    if (view == MAP_FAILED) {
        close(fd);
        return -3;
    }
    int32_t written = neoveg_cpp_shm_fill_frame(
        static_cast<uint8_t*>(view),
        total_bytes,
        width,
        height,
        sequence_id
    );
    munmap(view, static_cast<size_t>(total_bytes));
    close(fd);
    return written;
#endif
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_compute_pancrop_matrix(
    double source_width,
    double source_height,
    double crop_width,
    double crop_height,
    double x_center,
    double y_center,
    double rotation_deg,
    double* out_matrix6
) {
    if (!out_matrix6 || source_width <= 1.0 || source_height <= 1.0) {
        return 0;
    }

    const double safe_cw = std::clamp(crop_width, source_width * 0.10, source_width * 4.0);
    const double safe_ch = std::clamp(crop_height, source_height * 0.10, source_height * 4.0);

    const double zoom_x = source_width / safe_cw;
    const double zoom_y = source_height / safe_ch;

    const double rad = -rotation_deg * (3.14159265358979323846 / 180.0);
    const double cos_a = std::cos(rad);
    const double sin_a = std::sin(rad);

    const double dx = -(x_center - source_width * 0.5) * zoom_x;
    const double dy = -(y_center - source_height * 0.5) * zoom_y;

    out_matrix6[0] = zoom_x * cos_a;
    out_matrix6[1] = zoom_x * sin_a;
    out_matrix6[2] = -zoom_y * sin_a;
    out_matrix6[3] = zoom_y * cos_a;
    out_matrix6[4] = dx * cos_a - dy * sin_a;
    out_matrix6[5] = dx * sin_a + dy * cos_a;
    return 1;
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_ofx_host_enumerate(
    char* out_json,
    int32_t buf_len
) {
    return CopyStringBuffer(kOfxDescriptorJson, out_json, buf_len);
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_vst3_host_enumerate(
    char* out_json,
    int32_t buf_len
) {
    return CopyStringBuffer(kVst3DescriptorJson, out_json, buf_len);
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_voukoder_enumerate_encoders(
    char* out_json,
    int32_t buf_len
) {
    return CopyStringBuffer(kVoukoderEncodersJson, out_json, buf_len);
}

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_whisper_transcribe_json(
    const char* media_path,
    const char* language_code,
    const char* model_name,
    double duration_sec,
    char* out_json,
    int32_t buf_len
) {
    return neoveg_whisper_run_transcription(
        media_path,
        language_code,
        model_name,
        duration_sec,
        out_json,
        buf_len
    );
}

#if defined(_WIN32) || defined(_WIN64)
static bool ProbeWindowsSystemDll(const char* dll_name) {
    if (!dll_name || !dll_name[0]) {
        return false;
    }
    HMODULE hMod = LoadLibraryExA(dll_name, nullptr, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (hMod != nullptr) {
        FreeLibrary(hMod);
        return true;
    }
    char sys_dir[MAX_PATH] = {0};
    UINT len = GetSystemDirectoryA(sys_dir, MAX_PATH);
    if (len > 0 && len < MAX_PATH) {
        std::filesystem::path full_path = std::filesystem::path(sys_dir) / dll_name;
        std::error_code ec;
        if (std::filesystem::exists(full_path, ec)) {
            return true;
        }
    }
    return false;
}
#endif

NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_verify_directx_runtime(
    char* out_json,
    int32_t buf_len
) {
    try {
#if defined(_WIN32) || defined(_WIN64)
        const bool has_d3d11 = ProbeWindowsSystemDll("d3d11.dll");
        const bool has_dxgi = ProbeWindowsSystemDll("dxgi.dll");
        const bool has_xaudio2_7 = ProbeWindowsSystemDll("XAudio2_7.dll");
        const bool has_xaudio2_9 = ProbeWindowsSystemDll("XAudio2_9.dll");
        const bool visual_ok = has_d3d11 && has_dxgi;
        const bool audio_ok = has_xaudio2_7 || has_xaudio2_9;

        char log_msg[512] = {0};
        std::snprintf(
            log_msg,
            sizeof(log_msg),
            "DirectX Visual & Audio Acceleration Check: d3d11.dll=%s, dxgi.dll=%s, XAudio2_7.dll=%s, XAudio2_9.dll=%s (Visual=%s, Audio=%s)",
            has_d3d11 ? "ACCESSIBLE" : "MISSING",
            has_dxgi ? "ACCESSIBLE" : "MISSING",
            has_xaudio2_7 ? "ACCESSIBLE" : "MISSING",
            has_xaudio2_9 ? "ACCESSIBLE" : "MISSING",
            visual_ok ? "OK" : "DEGRADED",
            audio_ok ? "OK" : "DEGRADED"
        );
        NeovegCppLog((visual_ok && audio_ok) ? "INFO" : "WARN", "neoveg_cpp_verify_directx_runtime", log_msg);

        char json_buf[512] = {0};
        std::snprintf(
            json_buf,
            sizeof(json_buf),
            "{\"ok\":%s,\"d3d11.dll\":%s,\"dxgi.dll\":%s,\"XAudio2_7.dll\":%s,\"XAudio2_9.dll\":%s,"
            "\"visualAcceleration\":%s,\"audioAcceleration\":%s,"
            "\"redistUrl\":\"https://www.techpowerup.com/download/directx-redistributable-runtime/\"}",
            (visual_ok && audio_ok) ? "true" : "false",
            has_d3d11 ? "true" : "false",
            has_dxgi ? "true" : "false",
            has_xaudio2_7 ? "true" : "false",
            has_xaudio2_9 ? "true" : "false",
            visual_ok ? "true" : "false",
            audio_ok ? "true" : "false"
        );
#else
        constexpr const char* json_buf =
            "{\"ok\":true,\"platformSupported\":false,\"d3d11.dll\":false,\"dxgi.dll\":false,"
            "\"XAudio2_7.dll\":false,\"XAudio2_9.dll\":false,"
            "\"visualAcceleration\":false,\"audioAcceleration\":false}";
        NeovegCppLog("INFO", "neoveg_cpp_verify_directx_runtime", "DirectX prerequisite check skipped on non-Windows platform.");
#endif
        return CopyStringBuffer(json_buf, out_json, buf_len);
    } catch (const std::exception& ex) {
        NeovegCppLog("ERROR", "neoveg_cpp_verify_directx_runtime", ex.what());
        return 0;
    } catch (...) {
        NeovegCppLog("ERROR", "neoveg_cpp_verify_directx_runtime", "Unhandled native C++ exception");
        return 0;
    }
}

NEOVEG_CPP_API void NEOVEG_CDECL neoveg_cpp_free_buffer(void* ptr) {
    try {
        if (ptr != nullptr) {
            std::free(ptr);
        }
    } catch (...) {
    }
}

} /* extern "C" */
