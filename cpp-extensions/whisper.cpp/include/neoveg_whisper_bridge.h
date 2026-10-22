#ifndef NEOVEG_WHISPER_BRIDGE_H
#define NEOVEG_WHISPER_BRIDGE_H

#include <stdint.h>
#include "whisper.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Generates timestamped subtitle segments using the whisper.cpp C/C++ transcription pipeline
 * and writes a UTF-8 JSON payload into `out_json`.
 * Returns number of bytes written (excluding null terminator), or <= 0 on error.
 */
int32_t neoveg_whisper_run_transcription(
    const char* media_path,
    const char* language_code,
    const char* model_name,
    double duration_sec,
    char* out_json,
    int32_t buf_len
);

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_WHISPER_BRIDGE_H */
