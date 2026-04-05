/**
 * TurboQuant: KV Cache Vector Quantization
 *
 * Implementation based on:
 * "TurboQuant: Online Vector Quantization with Near-optimal Distortion Rate"
 * Amir Zandieh, Majid Daliri, Majid Hadian, Vahab Mirrokni
 * arXiv:2504.19874
 *
 * Pipeline (PolarQuant without QJL, following turboquant_plus):
 *   1. Extract norm: γ = ||x||
 *   2. Normalize: x̂ = x/γ
 *   3. Random rotation: WHT + random sign flips
 *   4. Lloyd-Max optimal scalar quantization
 *   5. Output: packed indices + norm
 */

#include "ggml-turboquant.h"
#include "ggml-common.h"

#include <math.h>
#include <string.h>
#include <stdlib.h>
#include <assert.h>

// ============================================================================
// Walsh-Hadamard Transform
// ============================================================================

static void fwht(float * x, int n) {
    for (int h = 1; h < n; h <<= 1) {
        for (int i = 0; i < n; i += 2 * h) {
            for (int j = i; j < i + h; j++) {
                float u = x[j];
                float v = x[j + h];
                x[j] = u + v;
                x[j + h] = u - v;
            }
        }
    }
}

static void random_signs(float * x, int n, uint64_t seed) {
    uint64_t state = seed;
    for (int i = 0; i < n; i++) {
        state = state * 6364136223846793005ULL + 1442695040888963407ULL;
        int sign = (state >> 63) ? -1 : 1;
        x[i] *= sign;
    }
}

static void turboquant_wht(float * x, int dim, uint64_t seed) {
    assert((dim & (dim - 1)) == 0 && "Dimension must be power of 2");
    random_signs(x, dim, seed);
    fwht(x, dim);
    float scale = 1.0f / sqrtf((float)dim);
    for (int i = 0; i < dim; i++) x[i] *= scale;
}

static void turboquant_iwht(float * x, int dim, uint64_t seed) {
    assert((dim & (dim - 1)) == 0 && "Dimension must be power of 2");
    fwht(x, dim);
    float scale = 1.0f / sqrtf((float)dim);
    for (int i = 0; i < dim; i++) x[i] *= scale;
    uint64_t state = seed;
    for (int i = 0; i < dim; i++) {
        state = state * 6364136223846793005ULL + 1442695040888963407ULL;
        int sign = (state >> 63) ? -1 : 1;
        x[i] *= sign;
    }
}

// ============================================================================
// Lloyd-Max Optimal Scalar Quantizer (pre-computed for N(0,1))
// ============================================================================

static const float turbo2_centroids[4] = { -1.5104f, -0.4527f, 0.4527f, 1.5104f };
static const float turbo2_boundaries[3] = { -0.9816f, 0.0f, 0.9816f };

static const float turbo3_centroids[8] = {
    -2.1553f, -1.3454f, -0.7556f, -0.2500f,
     0.2500f,  0.7556f,  1.3454f, 2.1553f
};
static const float turbo3_boundaries[7] = {
    -1.7504f, -1.0505f, -0.5028f, 0.0f,
     0.5028f,  1.0505f,  1.7504f
};

static const float turbo4_centroids[16] = {
    -2.5183f, -2.0325f, -1.6472f, -1.3193f, -1.0235f, -0.7482f, -0.4855f, -0.2305f,
     0.2305f,  0.4855f,  0.7482f,  1.0235f,  1.3193f,  1.6472f,  2.0325f, 2.5183f
};
static const float turbo4_boundaries[15] = {
    -2.2754f, -1.8404f, -1.4923f, -1.1714f, -0.8859f, -0.6178f, -0.3580f, 0.0f,
     0.3580f,  0.6178f,  0.8859f,  1.1714f,  1.4923f,  1.8404f,  2.2754f
};

static inline int scalar_quantize(float x, int bits) {
    switch (bits) {
        case 2:
            if (x < turbo2_boundaries[0]) return 0;
            if (x < turbo2_boundaries[1]) return 1;
            if (x < turbo2_boundaries[2]) return 2;
            return 3;
        case 3:
            if (x < turbo3_boundaries[0]) return 0;
            if (x < turbo3_boundaries[1]) return 1;
            if (x < turbo3_boundaries[2]) return 2;
            if (x < turbo3_boundaries[3]) return 3;
            if (x < turbo3_boundaries[4]) return 4;
            if (x < turbo3_boundaries[5]) return 5;
            if (x < turbo3_boundaries[6]) return 6;
            return 7;
        case 4:
            for (int i = 0; i < 15; i++) {
                if (x < turbo4_boundaries[i]) return i;
            }
            return 15;
        default: return 0;
    }
}

static inline float scalar_dequantize(int idx, int bits) {
    switch (bits) {
        case 2: return turbo2_centroids[idx & 3];
        case 3: return turbo3_centroids[idx & 7];
        case 4: return turbo4_centroids[idx & 15];
        default: return 0.0f;
    }
}

// ============================================================================
// Quantize / Dequantize
// ============================================================================

#define TURBO_BLOCK_SIZE 32
#define TURBO_SEED 42

static size_t turboquant_quantize(const float * x, int dim, int bits, uint8_t * out, float * out_norm, uint64_t seed) {
    assert(bits == 2 || bits == 3 || bits == 4);
    assert((dim & (dim - 1)) == 0 && "Dimension must be power of 2");

    float norm = 0.0f;
    for (int i = 0; i < dim; i++) norm += x[i] * x[i];
    norm = sqrtf(norm);
    *out_norm = norm;

    if (norm < 1e-8f) {
        memset(out, 0, (dim * bits + 7) / 8);
        return (dim * bits + 7) / 8;
    }

    float * buf = (float *)malloc(dim * sizeof(float));
    float inv_norm = 1.0f / norm;
    for (int i = 0; i < dim; i++) buf[i] = x[i] * inv_norm;

    turboquant_wht(buf, dim, seed);

    int out_idx = 0, bit_pos = 0;
    uint8_t current_byte = 0;
    for (int i = 0; i < dim; i++) {
        int idx = scalar_quantize(buf[i], bits);
        current_byte |= (idx << bit_pos);
        bit_pos += bits;
        while (bit_pos >= 8) {
            out[out_idx++] = current_byte & 0xFF;
            current_byte >>= 8;
            bit_pos -= 8;
        }
    }
    if (bit_pos > 0) out[out_idx++] = current_byte;

    free(buf);
    return out_idx;
}

static void turboquant_dequantize(const uint8_t * indices, float norm, int dim, int bits, float * out, uint64_t seed) {
    assert(bits == 2 || bits == 3 || bits == 4);

    if (norm < 1e-8f) {
        memset(out, 0, dim * sizeof(float));
        return;
    }

    int in_idx = 0, bit_pos = 0;
    uint16_t current_bits = 0;
    const int mask = (1 << bits) - 1;

    for (int i = 0; i < dim; i++) {
        while (bit_pos < bits) {
            current_bits |= ((uint16_t)indices[in_idx++]) << bit_pos;
            bit_pos += 8;
        }
        int idx = current_bits & mask;
        current_bits >>= bits;
        bit_pos -= bits;
        out[i] = scalar_dequantize(idx, bits) * norm;
    }

    turboquant_iwht(out, dim, seed);
}

// ============================================================================
// GGML Integration
// ============================================================================

int ggml_quantize_turbo(int type, const float * src, void * dst, int64_t nrow, int64_t n_per_row, int64_t * hist) {
    const int bits = (type == GGML_TYPE_TURBO2) ? 2 :
                     (type == GGML_TYPE_TURBO3) ? 3 : 4;
    const int64_t block_size = TURBO_BLOCK_SIZE;
    const int64_t nblocks = (nrow * n_per_row) / block_size;

    for (int64_t i = 0; i < nblocks; i++) {
        const float * x = src + i * block_size;
        if (type == GGML_TYPE_TURBO2) {
            uint8_t * qs = (uint8_t *)dst + i * 12;
            turboquant_quantize(x, block_size, bits, qs, (float *)(qs + 8), TURBO_SEED);
        } else if (type == GGML_TYPE_TURBO3) {
            uint8_t * qs = (uint8_t *)dst + i * 16;
            turboquant_quantize(x, block_size, bits, qs, (float *)(qs + 12), TURBO_SEED);
        } else {
            uint8_t * qs = (uint8_t *)dst + i * 20;
            turboquant_quantize(x, block_size, bits, qs, (float *)(qs + 16), TURBO_SEED);
        }
    }
    return nblocks;
}

void ggml_dequantize_turbo(int type, const void * src, float * dst, int nelem) {
    const int bits = (type == GGML_TYPE_TURBO2) ? 2 :
                     (type == GGML_TYPE_TURBO3) ? 3 : 4;
    const int64_t block_size = TURBO_BLOCK_SIZE;
    const int64_t nblocks = nelem / block_size;

    for (int64_t i = 0; i < nblocks; i++) {
        float * x = dst + i * block_size;
        if (type == GGML_TYPE_TURBO2) {
            const uint8_t * qs = (const uint8_t *)src + i * 12;
            turboquant_dequantize(qs, *(const float *)(qs + 8), block_size, bits, x, TURBO_SEED);
        } else if (type == GGML_TYPE_TURBO3) {
            const uint8_t * qs = (const uint8_t *)src + i * 16;
            turboquant_dequantize(qs, *(const float *)(qs + 12), block_size, bits, x, TURBO_SEED);
        } else {
            const uint8_t * qs = (const uint8_t *)src + i * 20;
            turboquant_dequantize(qs, *(const float *)(qs + 16), block_size, bits, x, TURBO_SEED);
        }
    }
}
