#!/usr/bin/env python3
"""
Apply TurboQuant patches to llama.cpp (compatible with b8668+)

This script is idempotent - safe to run multiple times.
It checks if patches are already applied before modifying files.
"""

import os
import sys
import re
from pathlib import Path

def find_llama_root():
    """Find llama.cpp root by checking for key files."""
    candidates = [
        Path.cwd(),
        Path(__file__).parent.parent / "build-native" / "llama.cpp",
        Path("/src"),
    ]
    for p in candidates:
        if (p / "ggml" / "include" / "ggml.h").exists():
            return p
    # Fallback: assume we're running from llama root
    if (Path.cwd() / "ggml" / "include" / "ggml.h").exists():
        return Path.cwd()
    return None

def patch_ggml_h(root):
    """Add TurboQuant types to ggml/include/ggml.h"""
    filepath = root / "ggml" / "include" / "ggml.h"
    content = filepath.read_text()
    
    # Check if already patched
    if "GGML_TYPE_TURBO2" in content:
        print("  ✓ ggml.h already patched (TURBO2 found)")
        return True
    
    # Replace GGML_TYPE_COUNT line with TurboQuant types + new COUNT
    # Pattern: GGML_TYPE_NVFP4   = 40, ... GGML_TYPE_COUNT   = 41,
    old_pattern = r'(        GGML_TYPE_NVFP4   = 40, // NVFP4 \(4 blocks, E4M3 scale\))\n(        GGML_TYPE_COUNT   = 41,)'
    new_text = r'''\1

        // TurboQuant KV cache types (Google Research, arXiv:2504.19874)
        GGML_TYPE_TURBO2  = 41,
        GGML_TYPE_TURBO3  = 42,
        GGML_TYPE_TURBO4  = 43,

        GGML_TYPE_COUNT   = 44,'''
    
    # Try direct replacement
    new_content = re.sub(old_pattern, new_text, content)
    if new_content == content:
        print(f"  ✗ Failed to patch ggml.h - pattern not found")
        return False
    
    filepath.write_text(new_content)
    print("  ✓ Patched ggml.h (added TURBO2/3/4 types)")
    return True

def patch_ggml_c(root):
    """Add TurboQuant include and type traits to ggml/src/ggml.c"""
    filepath = root / "ggml" / "src" / "ggml.c"
    content = filepath.read_text()
    
    # Check if already patched
    if "ggml-turboquant.h" in content:
        print("  ✓ ggml.c already patched (turboquant include found)")
        return True
    
    # 1. Add include after ggml-quants.h
    content = content.replace(
        '#include "ggml-quants.h"',
        '#include "ggml-quants.h"\n\n// TurboQuant: KV cache vector quantization (Google Research)\n#include "ggml-turboquant.h"'
    )
    
    # 2. Add type traits after TQ2_0 entry
    tq2_0_block = '''    [GGML_TYPE_TQ2_0] = {
        .type_name                = "tq2_0",
        .blck_size                = QK_K,
        .type_size                = sizeof(block_tq2_0),
        .is_quantized             = true,
        .to_float                 = (ggml_to_float_t) dequantize_row_tq2_0,
        .from_float_ref           = (ggml_from_float_t) quantize_row_tq2_0_ref,
    },'''
    
    turboquant_traits = '''    [GGML_TYPE_TQ2_0] = {
        .type_name                = "tq2_0",
        .blck_size                = QK_K,
        .type_size                = sizeof(block_tq2_0),
        .is_quantized             = true,
        .to_float                 = (ggml_to_float_t) dequantize_row_tq2_0,
        .from_float_ref           = (ggml_from_float_t) quantize_row_tq2_0_ref,
    },

    // TurboQuant KV cache types (Google Research, arXiv:2504.19874)
    // WHT rotation + Lloyd-Max optimal scalar quantization
    [GGML_TYPE_TURBO2] = {
        .type_name                = "turbo2",
        .blck_size                = 32,
        .type_size                = 12,  /* 8 bytes indices + 4 bytes norm */
        .is_quantized             = true,
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,
    },
    [GGML_TYPE_TURBO3] = {
        .type_name                = "turbo3",
        .blck_size                = 32,
        .type_size                = 16,  /* 12 bytes indices + 4 bytes norm */
        .is_quantized             = true,
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,
    },
    [GGML_TYPE_TURBO4] = {
        .type_name                = "turbo4",
        .blck_size                = 32,
        .type_size                = 20,  /* 16 bytes indices + 4 bytes norm */
        .is_quantized             = true,
        .to_float                 = (ggml_to_float_t) ggml_dequantize_turbo,
        .from_float_ref           = (ggml_from_float_t) ggml_quantize_turbo,
    },'''
    
    content = content.replace(tq2_0_block, turboquant_traits)
    
    # 3. Add quantize cases after TQ2_0 case
    old_case = '        case GGML_TYPE_TQ2_0:   result = quantize_tq2_0(src + start, (char *) dst + start_row * row_size, nrows, n_per_row, imatrix); break;'
    new_cases = '''        case GGML_TYPE_TQ2_0:   result = quantize_tq2_0(src + start, (char *) dst + start_row * row_size, nrows, n_per_row, imatrix); break;

        // TurboQuant: WHT + Lloyd-Max KV cache compression
        case GGML_TYPE_TURBO2:
        case GGML_TYPE_TURBO3:
        case GGML_TYPE_TURBO4:
            result = ggml_quantize_turbo(type, src + start, (char *) dst + start_row * row_size, nrows, n_per_row, NULL);
            break;'''
    
    content = content.replace(old_case, new_cases)
    
    filepath.write_text(content)
    print("  ✓ Patched ggml.c (added include, type traits, quantize cases)")
    return True

def patch_cmake(root):
    """Add TurboQuant files to ggml/src/CMakeLists.txt"""
    filepath = root / "ggml" / "src" / "CMakeLists.txt"
    content = filepath.read_text()
    
    if "ggml-turboquant.c" in content:
        print("  ✓ CMakeLists.txt already patched")
        return True
    
    # Add after ggml-quants.h
    content = content.replace(
        "ggml-quants.h",
        "ggml-quants.h\n            ggml-turboquant.c\n            ggml-turboquant.h"
    )
    
    filepath.write_text(content)
    print("  ✓ Patched CMakeLists.txt (added turboquant sources)")
    return True

def patch_arg_cpp(root):
    """Add TurboQuant types to cache types in common/arg.cpp"""
    filepath = root / "common" / "arg.cpp"
    content = filepath.read_text()
    
    if "GGML_TYPE_TURBO2" in content:
        print("  ✓ arg.cpp already patched")
        return True
    
    # Add TurboQuant types before closing brace of kv_cache_types
    old_list = '''    GGML_TYPE_Q5_1,
};'''
    new_list = '''    GGML_TYPE_Q5_1,
    // TurboQuant KV cache types (Google Research, arXiv:2504.19874)
    GGML_TYPE_TURBO2,  // 2-bit (4 centroids)
    GGML_TYPE_TURBO3,  // 3-bit (8 centroids)
    GGML_TYPE_TURBO4,  // 4-bit (16 centroids)
};'''
    
    content = content.replace(old_list, new_list)
    filepath.write_text(content)
    print("  ✓ Patched arg.cpp (added turbo2/3/4 cache types)")
    return True

def copy_turboquant_sources(root):
    """Copy TurboQuant source files to llama.cpp"""
    patches_dir = Path(__file__).parent
    dest_dir = root / "ggml" / "src"
    
    for filename in ["ggml-turboquant.c", "ggml-turboquant.h"]:
        src = patches_dir / filename
        dst = dest_dir / filename
        if src.exists():
            dst.write_text(src.read_text())
            print(f"  ✓ Copied {filename}")
        else:
            print(f"  ⚠ Source file {filename} not found in patches/")

def main():
    root = find_llama_root()
    if not root:
        print("ERROR: Could not find llama.cpp root directory")
        print("Run this script from the llama.cpp root directory")
        sys.exit(1)
    
    print(f"🔧 Applying TurboQuant patches to llama.cpp at: {root}")
    print()
    
    all_ok = True
    all_ok &= patch_ggml_h(root)
    all_ok &= patch_ggml_c(root)
    all_ok &= patch_cmake(root)
    all_ok &= patch_arg_cpp(root)
    copy_turboquant_sources(root)
    
    print()
    if all_ok:
        print("✅ All TurboQuant patches applied successfully.")
    else:
        print("❌ Some patches failed. Check errors above.")
        sys.exit(1)

if __name__ == "__main__":
    main()
