use anyhow::Result;
use crate::gguf_parser::GGMLType;

/// Dequantize a tensor from GGUF quantized format to f32
pub fn dequantize(data: &[u8], n_elements: usize, dtype: GGMLType) -> Result<Vec<f32>> {
    match dtype {
        GGMLType::F32 => {
            assert_eq!(data.len(), n_elements * 4);
            Ok(dequantize_f32(data))
        }
        GGMLType::F16 => {
            assert_eq!(data.len(), n_elements * 2);
            Ok(dequantize_f16(data))
        }
        GGMLType::BF16 => {
            assert_eq!(data.len(), n_elements * 2);
            Ok(dequantize_bf16(data))
        }
        GGMLType::Q6K => dequantize_q6k(data, n_elements),
        GGMLType::Q5K => dequantize_q5k(data, n_elements),
        GGMLType::Q4K => dequantize_q4k(data, n_elements),
        GGMLType::Q8_0 => dequantize_q8_0(data, n_elements),
        _ => anyhow::bail!("Unsupported quantization type: {:?}", dtype),
    }
}

fn dequantize_f32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

fn dequantize_f16(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|chunk| {
            let bits = u16::from_le_bytes([chunk[0], chunk[1]]) as u32;
            f16_to_f32(bits)
        })
        .collect()
}

fn dequantize_bf16(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|chunk| {
            let bits = u32::from_le_bytes([chunk[0], chunk[1], 0, 0]);
            f32::from_bits(bits)
        })
        .collect()
}

fn f16_to_f32(bits: u32) -> f32 {
    let sign = (bits >> 15) & 1;
    let exp = ((bits >> 10) & 0x1f) as i32;
    let frac = (bits & 0x3ff) as u32;
    if exp == 0 {
        return if frac == 0 { if sign != 0 { -0.0 } else { 0.0 } } 
            else { (if sign != 0 { -1.0 } else { 1.0 }) * (frac as f32) * (2.0f32).powi(-24) };
    }
    let new_exp = exp + (127 - 15);
    if new_exp >= 255 { return f32::from_bits((sign << 31) | 0x7f800000); }
    f32::from_bits((sign << 31) | ((new_exp as u32) << 23) | (frac << 13))
}

const QK_K: usize = 256;

fn dequantize_q6k(data: &[u8], n_elements: usize) -> Result<Vec<f32>> {
    let block_bytes = 210usize;
    let n_blocks = n_elements / QK_K;
    if n_blocks == 0 || data.len() < n_blocks * block_bytes {
        eprintln!("⚠️  Q6K: n_elements={}, n_blocks={}, data.len()={}, need={}",
            n_elements, n_blocks, data.len(), n_blocks * block_bytes);
        return Ok(vec![0.0; n_elements]);
    }
    let mut result = vec![0.0f32; n_blocks * QK_K];
    for i in 0..n_blocks {
        let b = &data[i * block_bytes..];
        let ql = &b[0..128];
        let qh = &b[128..192];
        let scales = unsafe { std::slice::from_raw_parts(b[192..208].as_ptr() as *const i8, 16) };
        let d = f16_to_f32(u16::from_le_bytes([b[208], b[209]]) as u32);
        for j in 0..QK_K {
            let ql_v = (ql[j / 2] >> ((j % 2) * 4)) & 0xf;
            let qh_v = (qh[j / 4] >> ((j % 4) * 2)) & 0x3;
            let q = (ql_v | (qh_v << 4)) as i32;
            let sc = scales[j / 16] as f32;
            result[i * QK_K + j] = d * sc * (q as f32 - 32.0);
        }
    }
    Ok(result)
}

fn dequantize_q5k(data: &[u8], n_elements: usize) -> Result<Vec<f32>> {
    let block_bytes = 176usize;
    let n_blocks = n_elements / QK_K;
    if n_blocks == 0 || data.len() < n_blocks * block_bytes {
        return Ok(vec![0.0; n_elements]);
    }
    let mut result = vec![0.0f32; n_blocks * QK_K];
    for i in 0..n_blocks {
        let b = &data[i * block_bytes..];
        let d = f16_to_f32(u16::from_le_bytes([b[0], b[1]]) as u32);
        let mut scales = [0u8; 16];
        for j in 0..6 { scales[j*2] = b[4+j] & 0xf; scales[j*2+1] = b[4+j] >> 4; }
        scales[12] = b[10] & 0xf; scales[13] = b[10] >> 4;
        scales[14] = b[11] & 0xf; scales[15] = b[11] >> 4;
        let qs = &b[48..176]; // 128 bytes at offset 48
        for j in 0..QK_K {
            let q4 = if j % 2 == 0 { qs[j/2] & 0xf } else { qs[j/2] >> 4 };
            let qh = (b[16 + j/8] >> (j % 8)) & 1;
            let q = (q4 | (qh << 4)) as i32;
            result[i * QK_K + j] = d * scales[j/16] as f32 * (q as f32 - 16.0);
        }
    }
    Ok(result)
}

fn dequantize_q4k(data: &[u8], n_elements: usize) -> Result<Vec<f32>> {
    let block_bytes = 144usize;
    let n_blocks = n_elements / QK_K;
    if n_blocks == 0 || data.len() < n_blocks * block_bytes {
        return Ok(vec![0.0; n_elements]);
    }
    let mut result = vec![0.0f32; n_blocks * QK_K];
    for i in 0..n_blocks {
        let b = &data[i * block_bytes..];
        let d = f16_to_f32(u16::from_le_bytes([b[0], b[1]]) as u32);
        let dmin = f16_to_f32(u16::from_le_bytes([b[2], b[3]]) as u32);
        let mut scales = [0u8; 16];
        for j in 0..6 { scales[j*2] = b[4+j] & 0xf; scales[j*2+1] = b[4+j] >> 4; }
        scales[12] = b[10] & 0xf; scales[13] = b[10] >> 4;
        scales[14] = b[11] & 0xf; scales[15] = b[11] >> 4;
        let qs = &b[16..144];
        for j in 0..QK_K {
            let q = if j % 2 == 0 { qs[j/2] & 0xf } else { qs[j/2] >> 4 } as i32;
            result[i * QK_K + j] = d * scales[j/16] as f32 * q as f32 - dmin * 8.0;
        }
    }
    Ok(result)
}

fn dequantize_q8_0(data: &[u8], n_elements: usize) -> Result<Vec<f32>> {
    const QK: usize = 32;
    let block_bytes = 34usize;
    let n_blocks = n_elements / QK;
    if n_blocks == 0 || data.len() < n_blocks * block_bytes {
        return Ok(vec![0.0; n_elements]);
    }
    let mut result = vec![0.0f32; n_blocks * QK];
    for i in 0..n_blocks {
        let b = &data[i * block_bytes..];
        let d = f16_to_f32(u16::from_le_bytes([b[0], b[1]]) as u32);
        for j in 0..QK {
            result[i * QK + j] = d * b[2 + j] as i8 as f32;
        }
    }
    Ok(result)
}

pub fn quantized_size(n_elements: usize, dtype: GGMLType) -> usize {
    match dtype {
        GGMLType::F32 => n_elements * 4,
        GGMLType::F16 | GGMLType::BF16 => n_elements * 2,
        GGMLType::Q4K => ((n_elements + QK_K - 1) / QK_K) * 144,
        GGMLType::Q5K => ((n_elements + QK_K - 1) / QK_K) * 176,
        GGMLType::Q6K => ((n_elements + QK_K - 1) / QK_K) * 210,
        GGMLType::Q8_0 => ((n_elements + 31) / 32) * 34,
        _ => n_elements,
    }
}

/// Byte offset + byte count for a single row of a row-major 2D tensor.
///
/// GGUF stores the innermost dimension first (d0 = row width).
/// Row `row_idx` occupies bytes `[start, start + count)` in the tensor data.
pub fn row_byte_range(row_idx: usize, n_cols: usize, dtype: GGMLType) -> (usize, usize) {
    let bytes = quantized_size(n_cols, dtype);
    (row_idx * bytes, bytes)
}

/// Dequantize a single row from a 2D tensor (row-major, quantized).
///
/// `full_tensor_data`: raw bytes of the entire tensor starting at its data offset.
/// `row_idx`: zero-based row index.
/// `n_cols`: number of scalar elements per row (d0 in GGUF dims).
pub fn dequantize_row(full_tensor_data: &[u8], row_idx: usize, n_cols: usize, dtype: GGMLType) -> Result<Vec<f32>> {
    let (start, count) = row_byte_range(row_idx, n_cols, dtype);
    if start + count > full_tensor_data.len() {
        anyhow::bail!(
            "Row {} out of bounds: need bytes {}..{}, data len {}",
            row_idx, start, start + count, full_tensor_data.len()
        );
    }
    dequantize(&full_tensor_data[start..start + count], n_cols, dtype)
}
