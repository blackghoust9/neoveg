#ifndef NEOVEG_THIRD_PARTY_OFX_CORE_H
#define NEOVEG_THIRD_PARTY_OFX_CORE_H

/*
 * OpenFX 1.4 Core Specification Header (ofxCore.h)
 * License: BSD-3-Clause / Apache-2.0 (Academy Software Foundation - ASWF)
 * Integrated for Neoveg C++ Host (neoveg_cpp_engine)
 */

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef int OfxStatus;
typedef struct OfxPropertySetStruct* OfxPropertySetHandle;

#define kOfxStatOK                  0
#define kOfxStatFailed              1
#define kOfxStatErrFatal            2
#define kOfxStatErrUnknown          3
#define kOfxStatErrMissingHostFeature 4
#define kOfxStatErrUnsupported      5
#define kOfxStatErrExists           6
#define kOfxStatErrFormat           7
#define kOfxStatErrMemory           8
#define kOfxStatErrBadHandle        9
#define kOfxStatErrBadIndex         10
#define kOfxStatErrValue            11
#define kOfxStatReplyYes            12
#define kOfxStatReplyNo             13
#define kOfxStatReplyDefault        14

/* Standard OpenFX Actions */
#define kOfxActionLoad                  "OfxActionLoad"
#define kOfxActionUnload                "OfxActionUnload"
#define kOfxActionDescribe              "OfxActionDescribe"
#define kOfxActionCreateInstance        "OfxActionCreateInstance"
#define kOfxActionDestroyInstance       "OfxActionDestroyInstance"
#define kOfxActionBeginInstanceChanged  "OfxActionBeginInstanceChanged"
#define kOfxActionInstanceChanged       "OfxActionInstanceChanged"
#define kOfxActionEndInstanceChanged    "OfxActionEndInstanceChanged"
#define kOfxActionPurgeCaches           "OfxActionPurgeCaches"
#define kOfxActionSyncPrivateData       "OfxActionSyncPrivateData"

typedef struct OfxHost {
    OfxPropertySetHandle host;
    const void* (*fetchSuite)(OfxPropertySetHandle host, const char* suiteName, int suiteVersion);
} OfxHost;

typedef OfxStatus (OfxPluginEntryPoint)(
    const char* action,
    const void* handle,
    OfxPropertySetHandle inArgs,
    OfxPropertySetHandle outArgs
);

typedef struct OfxPlugin {
    const char*          pluginApi;
    int                  apiVersion;
    const char*          pluginIdentifier;
    unsigned int         pluginVersionMajor;
    unsigned int         pluginVersionMinor;
    void                 (*setHost)(OfxHost* host);
    OfxPluginEntryPoint* mainEntry;
} OfxPlugin;

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_THIRD_PARTY_OFX_CORE_H */
