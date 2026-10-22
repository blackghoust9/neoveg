#ifndef NEOVEG_THIRD_PARTY_VOUKODER_BRIDGE_H
#define NEOVEG_THIRD_PARTY_VOUKODER_BRIDGE_H

/*
 * Voukoder Connector C/C++ Bridge Header (voukoder_bridge.h)
 * License: LGPL-2.1-or-later / GPL-2.0
 * Provides hardware/broadcast encoder descriptors for Neoveg Render Engine
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct neoveg_voukoder_encoder_info_t {
    const char* id;
    const char* name;
    const char* codec_family;
    const char* hw_accel;
    int32_t     supports_10bit;
    int32_t     supports_alpha;
} neoveg_voukoder_encoder_info_t;

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_THIRD_PARTY_VOUKODER_BRIDGE_H */
