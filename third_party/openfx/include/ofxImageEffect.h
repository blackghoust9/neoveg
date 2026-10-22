#ifndef NEOVEG_THIRD_PARTY_OFX_IMAGE_EFFECT_H
#define NEOVEG_THIRD_PARTY_OFX_IMAGE_EFFECT_H

/*
 * OpenFX 1.4 Image Effect Suite Header (ofxImageEffect.h)
 * License: BSD-3-Clause / Apache-2.0 (Academy Software Foundation - ASWF)
 */

#include "ofxCore.h"

#ifdef __cplusplus
extern "C" {
#endif

#define kOfxImageEffectPluginApi          "OfxImageEffectPluginAPI"
#define kOfxImageEffectPluginApiVersion   1
#define kOfxImageEffectSuite              "OfxImageEffectSuite"

#define kOfxImageEffectActionDescribeInContext "OfxImageEffectActionDescribeInContext"
#define kOfxImageEffectActionGetRegionOfDefinition "OfxImageEffectActionGetRegionOfDefinition"
#define kOfxImageEffectActionRender       "OfxImageEffectActionRender"

#define kOfxImageEffectContextFilter      "OfxImageEffectContextFilter"
#define kOfxImageEffectContextGenerator   "OfxImageEffectContextGenerator"
#define kOfxImageEffectContextTransition  "OfxImageEffectContextTransition"
#define kOfxImageEffectContextGeneral     "OfxImageEffectContextGeneral"

typedef struct OfxImageEffectStruct* OfxImageEffectHandle;
typedef struct OfxImageClipStruct*   OfxImageClipHandle;

typedef struct OfxRectI {
    int x1, y1, x2, y2;
} OfxRectI;

typedef struct OfxRectD {
    double x1, y1, x2, y2;
} OfxRectD;

typedef struct OfxImageEffectSuiteV1 {
    OfxStatus (*getPropertySet)(OfxImageEffectHandle imageEffect, OfxPropertySetHandle* propHandle);
    OfxStatus (*clipDefine)(OfxImageEffectHandle imageEffect, const char* name, OfxPropertySetHandle* propertySet);
    OfxStatus (*clipGetHandle)(OfxImageEffectHandle imageEffect, const char* name, OfxImageClipHandle* clip, OfxPropertySetHandle* propertySet);
    OfxStatus (*clipGetPropertySet)(OfxImageClipHandle clip, OfxPropertySetHandle* propHandle);
    OfxStatus (*clipGetImage)(OfxImageClipHandle clip, double time, const OfxRectD* region, OfxPropertySetHandle* imageHandle);
    OfxStatus (*clipReleaseImage)(OfxPropertySetHandle imageHandle);
    int       (*abort)(OfxImageEffectHandle imageEffect);
} OfxImageEffectSuiteV1;

#ifdef __cplusplus
}
#endif

#endif /* NEOVEG_THIRD_PARTY_OFX_IMAGE_EFFECT_H */
