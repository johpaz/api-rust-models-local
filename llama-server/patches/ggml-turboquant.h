#ifndef GGML_TURBOQUANT_H
#define GGML_TURBOQUANT_H

#include "ggml.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * TurboQuant: KV Cache Vector Quantization
 *
 * Based on: "TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate"
 * Amir Zandieh, Majid Daliri, Majid Hadian, Vahab Mirrokni
 * arXiv:2504.19874
 *
 * Pipeline (PolarQuant without QJL):
 *   1. Extract norm: γ = ||x||
 *   2. Normalize: x̂ = x/γ
 *   3. Random rotation: WHT + random sign flips
 *   4. Lloyd-Max optimal scalar quantization
 *   5. Output: packed indices (2-4 bits) + norm
 */

// Quantize a chunk of float values to TurboQuant format
int ggml_quantize_turbo(
    int type,
    const float * src,
    void * dst,
    int64_t nrow,
    int64_t n_per_row,
    int64_t * hist
);

// Dequantize TurboQuant block to float values
void ggml_dequantize_turbo(
    int type,
    const void * src,
    float * dst,
    int nelem
);

#ifdef __cplusplus
}
#endif

#endif // GGML_TURBOQUANT_H
