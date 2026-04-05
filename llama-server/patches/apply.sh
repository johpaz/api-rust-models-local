#!/bin/bash
# Aplica los parches de TurboQuant a llama.cpp vanilla
set -e

cd /src
PATCHES_DIR="/src/patches"

echo "🔧 Aplicando parches TurboQuant a llama.cpp..."

# 1. Agregar tipos TurboQuant a ggml.h
echo "  → Patching ggml/include/ggml.h (adding TURBO2/3/4 types)..."
# Delete the old GGML_TYPE_COUNT line and add new types before it
sed -i '/^        GGML_TYPE_COUNT   = 41,$/d' ggml/include/ggml.h

# Add TurboQuant types + updated count after NVFP4
sed -i '/GGML_TYPE_NVFP4.*= 40/a\
\
        // TurboQuant KV cache types (Google Research, arXiv:2504.19874)\
        GGML_TYPE_TURBO2  = 41,\
        GGML_TYPE_TURBO3  = 42,\
        GGML_TYPE_TURBO4  = 43,\
\
        GGML_TYPE_COUNT   = 44,' \
    ggml/include/ggml.h

# 2. Agregar include de TurboQuant en ggml.c
echo "  → Patching ggml/src/ggml.c (adding TurboQuant include)..."
sed -i '/#include "ggml-quants.h"/a\
\
// TurboQuant: KV cache vector quantization (Google Research)\
#include "ggml-turboquant.h"' \
    ggml/src/ggml.c

# 3. Agregar type traits para TurboQuant
echo "  → Patching ggml/src/ggml.c (adding TurboQuant type traits)..."
sed -i '/\[GGML_TYPE_TQ2_0\] = {/,/},/{
    /},/a\
\
    // TurboQuant KV cache types (Google Research, arXiv:2504.19874)\
    // WHT rotation + Lloyd-Max optimal scalar quantization\
    [GGML_TYPE_TURBO2] = {\
        .type_name                = "turbo2",\
        .blck_size                = 32,\
        .type_size                = 12,  /* 8 bytes indices + 4 bytes norm */\
        .is_quantized             = true,\
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,\
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,\
    },\
    [GGML_TYPE_TURBO3] = {\
        .type_name                = "turbo3",\
        .blck_size                = 32,\
        .type_size                = 16,  /* 12 bytes indices + 4 bytes norm */\
        .is_quantized             = true,\
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,\
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,\
    },\
    [GGML_TYPE_TURBO4] = {\
        .type_name                = "turbo4",\
        .blck_size                = 32,\
        .type_size                = 20,  /* 16 bytes indices + 4 bytes norm */\
        .is_quantized             = true,\
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,\
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,\
    },
}' ggml/src/ggml.c

# 4. Agregar case para TurboQuant en ggml_quantize_chunk
echo "  → Patching ggml/src/ggml.c (adding TurboQuant quantize cases)..."
sed -i '/case GGML_TYPE_TQ2_0:.*quantize_tq2_0/a\
\
        // TurboQuant: WHT + Lloyd-Max KV cache compression\
        case GGML_TYPE_TURBO2:\
        case GGML_TYPE_TURBO3:\
        case GGML_TYPE_TURBO4:\
            result = ggml_quantize_turbo(type, src + start, (char *) dst + start_row * row_size, nrows, n_per_row, NULL);\
            break;' ggml/src/ggml.c

# 5. Agregar archivos TurboQuant al CMakeLists.txt
echo "  → Patching ggml/src/CMakeLists.txt (adding TurboQuant files)..."
sed -i '/ggml-quants.h/a\            ggml-turboquant.c\
            ggml-turboquant.h' ggml/src/CMakeLists.txt

# 6. Agregar TurboQuant a los tipos de cache válidos en arg.cpp
echo "  → Patching common/arg.cpp (adding turbo2/3/4 as valid cache types)..."
sed -i '/GGML_TYPE_Q5_1,$/a\
    // TurboQuant KV cache types (Google Research, arXiv:2504.19874)\
    GGML_TYPE_TURBO2,  // 2-bit (4 centroids)\
    GGML_TYPE_TURBO3,  // 3-bit (8 centroids)\
    GGML_TYPE_TURBO4,  // 4-bit (16 centroids)' common/arg.cpp

# 7. Copiar archivos fuente de TurboQuant
echo "  → Copying TurboQuant source files..."
cp "$PATCHES_DIR/ggml-turboquant.c" ggml/src/ggml-turboquant.c
cp "$PATCHES_DIR/ggml-turboquant.h" ggml/src/ggml-turboquant.h

echo "✅ Todos los parches aplicados correctamente."
echo "📋 Ahora puedes compilar llama-server con TurboQuant integrado."
