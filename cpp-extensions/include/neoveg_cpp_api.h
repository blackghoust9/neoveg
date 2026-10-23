#ifndef NEOVEG_CPP_API_H
#define NEOVEG_CPP_API_H

#include <stdint.h>

/* Third-Party Industry SDK Headers from `third_party/` */
#include "ofxCore.h"
#include "ofxImageEffect.h"
#include "vst/ivstaudioprocessor.h"
#include "voukoder_bridge.h"
#include "whisper.h"
#include "neoveg_whisper_bridge.h"

#if defined(_WIN32) || defined(_WIN64)
    #if defined(NEOVEG_CPP_EXPORTS)
        #define NEOVEG_CPP_API __declspec(dllexport)
    #else
        #define NEOVEG_CPP_API __declspec(dllimport)
    #endif
    #define NEOVEG_CDECL __cdecl
#else
    #define NEOVEG_CPP_API __attribute__((visibility("default")))
    #define NEOVEG_CDECL
#endif

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Binary header placed at offset 0 of the Out-of-Process Shared Memory (Memory-Mapped File) region.
 * Mirrors Neoveg's `FileIOSurrogate.exe` / `sfvstserver.exe` IPC ring-buffer frame descriptor.
 * Total header size: 32 bytes (64-bit aligned).
 */
#pragma pack(push, 1)
typedef struct neoveg_cpp_shm_frame_header_t {
    uint32_t magic;          /* 0x4853564E ("NVSH" in little-endian) */
    uint32_t version;        /* Protocol version (210 = Neoveg 2.1) */
    int32_t  width;          /* Frame width in pixels */
    int32_t  height;         /* Frame height in pixels */
    int32_t  stride;         /* Aligned row stride in bytes (width * 4) */
    int32_t  sequence_id;    /* Monotonically increasing frame sequence number */
    int32_t  ready_flag;     /* 1 = Frame ready for C# consumer, 0 = Writing in progress */
    uint32_t pixel_checksum; /* Fast FNV-1a / additive checksum of RGBA8 payload */
} neoveg_cpp_shm_frame_header_t;
#pragma pack(pop)

/**
 * 1. Writes the C/C++ Extension SDK Host version banner (OpenFX 1.4 / VST3 3.7 / Voukoder / Whisper.cpp / SharedMemory IPC) into `out_buf`.
 * Returns number of bytes written (excluding null terminator).
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_version(char* out_buf, int32_t buf_len);

/**
 * 2. Executes an OpenFX (`OfxImageEffectSuiteV1`) C++ host render pass on an RGBA8 image buffer.
 * Returns total pixels processed (`width * height`), or <= 0 on invalid arguments.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_ofx_process_rgba8(
    uint8_t* rgba_buf,
    int32_t width,
    int32_t height,
    int32_t stride,
    double gain_r,
    double gain_g,
    double gain_b
);

/**
 * 3. Executes a Steinberg VST3 (`IAudioProcessor::process`) 32-bit float audio bus host callback.
 * Returns number of audio frames processed, or <= 0 on invalid arguments.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_vst3_process_f32(
    float* samples,
    int32_t sample_count,
    int32_t channels,
    double drive
);

/**
 * 4. Populates a Shared Memory (Memory-Mapped) buffer (`shm_base_ptr`) with a 32-byte
 * `neoveg_cpp_shm_frame_header_t` followed by a procedural RGBA8 video frame (`width * height * 4` bytes),
 * simulating Out-of-Process `FileIOSurrogate` frame decoder transfer to C#.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_shm_fill_frame(
    uint8_t* shm_base_ptr,
    int32_t shm_capacity,
    int32_t width,
    int32_t height,
    int32_t sequence_id
);

/**
 * 5. Opens or creates a named OS Shared Memory segment (`CreateFileMappingA` on Windows, `shm_open`/`mmap` on Linux/macOS),
 * writes a complete `neoveg_cpp_shm_frame_header_t` + RGBA8 video frame out-of-process, and returns the payload checksum.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_shm_publish_named(
    const char* shm_name,
    int32_t width,
    int32_t height,
    int32_t sequence_id
);

/**
 * 6. Computes the Neoveg Inverted Camera 2D Affine Transformation Matrix (`out_matrix6`: [M11, M12, M21, M22, OffsetX, OffsetY])
 * from the Event Pan/Crop 'F' Gizmo parameters (`crop_width`, `crop_height`, `x_center`, `y_center`, `rotation_deg`).
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_compute_pancrop_matrix(
    double source_width,
    double source_height,
    double crop_width,
    double crop_height,
    double x_center,
    double y_center,
    double rotation_deg,
    double* out_matrix6
);

/**
 * 7. Enumerates OpenFX (`ofxCore.h` / `OfxImageEffectSuiteV1`) video effect descriptors as UTF-8 JSON for C# Avalonia & Rust FFI.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_ofx_host_enumerate(
    char* out_json,
    int32_t buf_len
);

/**
 * 8. Enumerates Steinberg VST3 (`ivstaudioprocessor.h` / `Steinberg::Vst::ProcessSetup`) audio DSP descriptors as UTF-8 JSON.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_vst3_host_enumerate(
    char* out_json,
    int32_t buf_len
);

/**
 * 9. Enumerates Voukoder Connector (`voukoder_bridge.h`) hardware & broadcast encoders as UTF-8 JSON.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_voukoder_enumerate_encoders(
    char* out_json,
    int32_t buf_len
);

/**
 * 10. Invokes the whisper.cpp (`whisper.h`) transcription engine and outputs timestamped subtitle segments as UTF-8 JSON
 * for `SubtitleGeneratorService.cs` to create timeline text events automatically.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_whisper_transcribe_json(
    const char* media_path,
    const char* language_code,
    const char* model_name,
    double duration_sec,
    char* out_json,
    int32_t buf_len
);

/**
 * 11. Verifies DirectX Visual & Audio Hardware Acceleration runtime libraries (`d3d11.dll`, `dxgi.dll`, `XAudio2_7.dll`, `XAudio2_9.dll`),
 * logs the boot status to `logs/neoveg.log`, and writes a UTF-8 JSON diagnostic report into `out_json`.
 */
NEOVEG_CPP_API int32_t NEOVEG_CDECL neoveg_cpp_verify_directx_runtime(
    char* out_json,
    int32_t buf_len
);

/**
 * 12. Explicitly frees a C++ heap buffer allocated for FFI consumers, guaranteeing zero memory leaks across FFI boundaries.
 */
NEOVEG_CPP_API void NEOVEG_CDECL neoveg_cpp_free_buffer(void* ptr);

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_CPP_API_H */
