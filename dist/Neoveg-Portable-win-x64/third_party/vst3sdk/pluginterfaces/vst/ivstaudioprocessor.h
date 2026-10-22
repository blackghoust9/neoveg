#ifndef NEOVEG_THIRD_PARTY_VST3_AUDIO_PROCESSOR_H
#define NEOVEG_THIRD_PARTY_VST3_AUDIO_PROCESSOR_H

/*
 * Steinberg VST3 Audio Processor Interface Header (ivstaudioprocessor.h)
 * License: GPL-3.0-or-later (Steinberg Media Technologies GmbH)
 * C/C++ Host Definitions for Neoveg DSP Audio Engine (neoveg_cpp_engine)
 */

#include <stdint.h>

#ifdef __cplusplus
namespace Steinberg {
namespace Vst {
extern "C" {
#endif

typedef float  Sample32;
typedef double Sample64;

enum SymbolicSampleSizes {
    kSample32 = 0,
    kSample64 = 1
};

enum ProcessModes {
    kRealtime = 0,
    kPrefetch = 1,
    kOffline  = 2
};

typedef struct ProcessSetup {
    int32_t processMode;
    int32_t symbolicSampleSize;
    int32_t maxSamplesPerBlock;
    double  sampleRate;
} ProcessSetup;

typedef struct AudioBusBuffers {
    int32_t  numChannels;
    uint64_t silenceFlags;
    union {
        float**  channelBuffers32;
        double** channelBuffers64;
    };
} AudioBusBuffers;

typedef struct ProcessData {
    int32_t          processMode;
    int32_t          symbolicSampleSize;
    int32_t          numSamples;
    int32_t          numInputs;
    int32_t          numOutputs;
    AudioBusBuffers* inputs;
    AudioBusBuffers* outputs;
} ProcessData;

#ifdef __cplusplus
} /* extern "C" */
} /* namespace Vst */
} /* namespace Steinberg */
#endif

#endif /* NEOVEG_THIRD_PARTY_VST3_AUDIO_PROCESSOR_H */
