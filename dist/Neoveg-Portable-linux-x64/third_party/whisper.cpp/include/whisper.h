#ifndef NEOVEG_THIRD_PARTY_WHISPER_H
#define NEOVEG_THIRD_PARTY_WHISPER_H

/*
 * whisper.cpp C API Header (whisper.h)
 * License: MIT (Georgi Gerganov / ggerganov/whisper.cpp)
 * Integrated for Neoveg AI Subtitle & Timeline Text Event Generator
 */

#include <stdint.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

struct whisper_context;

enum whisper_sampling_strategy {
    WHISPER_SAMPLING_GREEDY      = 0,
    WHISPER_SAMPLING_BEAM_SEARCH = 1
};

typedef struct whisper_segment_t {
    int64_t     t0_ms;
    int64_t     t1_ms;
    const char* text;
    float       confidence;
    const char* speaker;
} whisper_segment_t;

typedef struct whisper_full_params {
    int         strategy;
    int         n_threads;
    int         offset_ms;
    int         duration_ms;
    bool        translate;
    bool        single_segment;
    bool        token_timestamps;
    const char* language;
} whisper_full_params;

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_THIRD_PARTY_WHISPER_H */
