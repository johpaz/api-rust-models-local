use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::Path;
use tracing::{debug, info};

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const GGUF_VERSION: u32 = 3;

/// GGUF tensor data type
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy)]
pub enum GGMLType {
    F32, F16, Q4_0, Q4_1, Q5_0, Q5_1, Q8_0, Q8_1,
    Q2K, Q3K, Q4K, Q5K, Q6K, Q8K,
    IQ2_XXS, IQ2_XS, IQ3_XXS, IQ3_S, IQ2_S, IQ1_S, IQ4_NL, IQ4_XS,
    I8, I16, I32, I64, F64, IQ1_M,
    BF16, TQ1_0, TQ2_0, MXFP4,
}

impl GGMLType {
    pub fn from_u32(v: u32) -> Result<Self> {
        Ok(match v {
            0 => GGMLType::F32, 1 => GGMLType::F16, 2 => GGMLType::Q4_0, 3 => GGMLType::Q4_1,
            6 => GGMLType::Q5_0, 7 => GGMLType::Q5_1, 8 => GGMLType::Q8_0, 9 => GGMLType::Q8_1,
            10 => GGMLType::Q2K, 11 => GGMLType::Q3K, 12 => GGMLType::Q4K, 13 => GGMLType::Q5K,
            14 => GGMLType::Q6K, 15 => GGMLType::Q8K, 16 => GGMLType::IQ2_XXS, 17 => GGMLType::IQ2_XS,
            18 => GGMLType::IQ3_XXS, 19 => GGMLType::IQ1_S, 20 => GGMLType::IQ4_NL, 21 => GGMLType::IQ3_S,
            22 => GGMLType::IQ2_S, 23 => GGMLType::IQ4_XS, 24 => GGMLType::I8, 25 => GGMLType::I16,
            26 => GGMLType::I32, 27 => GGMLType::I64, 28 => GGMLType::F64, 29 => GGMLType::IQ1_M,
            30 => GGMLType::BF16, 34 => GGMLType::TQ1_0, 35 => GGMLType::TQ2_0, 39 => GGMLType::MXFP4,
            other => anyhow::bail!("Unknown ggml type: {}", other),
        })
    }

    /// Size in bytes of a full block for this type
    pub fn block_size_bytes(&self) -> usize {
        match self {
            GGMLType::F32 => 4,
            GGMLType::F16 | GGMLType::BF16 => 2,
            GGMLType::Q4_0 | GGMLType::Q4_1 | GGMLType::Q5_0 | GGMLType::Q5_1
            | GGMLType::Q8_0 | GGMLType::Q8_1 => {
                // 32-element blocks
                match self {
                    GGMLType::Q4_0 => 2 + 16,       // d(2) + qs(16)
                    GGMLType::Q4_1 => 2 + 2 + 16,   // d(2) + m(2) + qs(16)
                    GGMLType::Q5_0 => 2 + 16 + 2,   // d(2) + qs(16) + qh(2)
                    GGMLType::Q5_1 => 2 + 2 + 16 + 2,
                    GGMLType::Q8_0 => 2 + 32,
                    GGMLType::Q8_1 => 2 + 2 + 32,
                    _ => unreachable!(),
                }
            }
            // 256-element K-quants
            GGMLType::Q2K => 2 + 2 + 32 + 64,       // d + dmin + scales + qs
            GGMLType::Q3K => 2 + 2 + 64 + 16 + 32,  // d + qs(64) + h(16) + scales(32)
            GGMLType::Q4K => 2 + 2 + 12 + 128,      // d + dmin + scales + qs
            GGMLType::Q5K => 2 + 2 + 12 + 32 + 128, // d + dmin + scales + qh + qs
            GGMLType::Q6K => 128 + 64 + 16 + 2,     // ql + qh + scales + d
            GGMLType::Q8K => 2 + 256 + 64,          // d + qs + scales
            GGMLType::IQ2_XXS => 2 + 32 + 64,
            GGMLType::IQ2_XS => 2 + 64 + 32 + 2,
            GGMLType::IQ3_XXS => 2 + 64 + 32,
            GGMLType::IQ1_S => 2 + 64 + 32,
            GGMLType::IQ4_NL => 2 + 128,
            GGMLType::IQ4_XS => 2 + 2 + 128 + 32,
            GGMLType::IQ2_S => 2 + 64 + 32 + 2,
            GGMLType::IQ3_S => 2 + 64 + 32 + 2,
            GGMLType::IQ1_M => 2 + 64 + 32,
            GGMLType::TQ1_0 | GGMLType::TQ2_0 => 256 / 2, // TurboQuant approximate
            GGMLType::MXFP4 => 2 + 256 / 2,
            GGMLType::I8 => 1, GGMLType::I16 => 2, GGMLType::I32 => 4,
            GGMLType::I64 => 8, GGMLType::F64 => 8,
        }
    }

    /// Number of elements per block
    pub fn block_elements(&self) -> usize {
        match self {
            GGMLType::F32 | GGMLType::F16 | GGMLType::BF16
            | GGMLType::I8 | GGMLType::I16 | GGMLType::I32
            | GGMLType::I64 | GGMLType::F64 => 1,
            GGMLType::Q4_0 | GGMLType::Q4_1 | GGMLType::Q5_0 | GGMLType::Q5_1
            | GGMLType::Q8_0 | GGMLType::Q8_1 => 32,
            _ => 256, // K-quants and others
        }
    }

    /// Calculate byte size for n elements
    pub fn byte_size(&self, n_elements: usize) -> usize {
        let block_elems = self.block_elements();
        let block_bytes = self.block_size_bytes();
        let n_blocks = (n_elements + block_elems - 1) / block_elems;
        n_blocks * block_bytes
    }
}

/// GGUF metadata value
#[derive(Debug, Clone)]
pub enum GGUFValue {
    U8(u8), I8(i8), U16(u16), I16(i16),
    U32(u32), I32(i32), F32(f32), Bool(bool),
    String(String), Array(Vec<GGUFValue>),
    U64(u64), I64(i64), F64(f64),
}

/// GGUF model architecture
#[derive(Debug, Clone)]
pub enum ModelArch {
    Llama, Mistral, Gemma, Qwen, Unknown(String),
}

impl ModelArch {
    pub fn from_string(s: &str) -> Self {
        match s {
            "llama" => ModelArch::Llama,
            "mistral" => ModelArch::Mistral,
            "gemma" | "gemma4" => ModelArch::Gemma,
            "qwen2" | "qwen3" => ModelArch::Qwen,
            other => ModelArch::Unknown(other.to_string()),
        }
    }
}

/// Parsed tensor info
#[derive(Debug, Clone)]
pub struct TensorInfo {
    pub name: String,
    pub dims: Vec<u64>,
    pub dtype: GGMLType,
    pub offset: u64,
    pub size_bytes: u64,
}

/// Parsed GGUF model info
#[derive(Debug, Clone)]
pub struct GGUFModelInfo {
    pub key_values: HashMap<String, GGUFValue>,
    pub tensors: Vec<TensorInfo>,
    pub architecture: ModelArch,
    pub n_layers: Option<u32>,
    pub n_embd: Option<u32>,
    pub n_head: Option<u32>,
    pub n_vocab: Option<u32>,
    pub tensor_data_start: u64,
}

struct GGUFReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> GGUFReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len() - self.pos
    }

    fn read_u8(&mut self) -> Result<u8> {
        if self.remaining() < 1 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = self.data[self.pos];
        self.pos += 1;
        Ok(v)
    }

    fn read_u16(&mut self) -> Result<u16> {
        if self.remaining() < 2 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = u16::from_le_bytes([self.data[self.pos], self.data[self.pos + 1]]);
        self.pos += 2;
        Ok(v)
    }

    fn read_u32(&mut self) -> Result<u32> {
        if self.remaining() < 4 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = u32::from_le_bytes([
            self.data[self.pos], self.data[self.pos + 1],
            self.data[self.pos + 2], self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_u64(&mut self) -> Result<u64> {
        if self.remaining() < 8 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = u64::from_le_bytes([
            self.data[self.pos], self.data[self.pos + 1],
            self.data[self.pos + 2], self.data[self.pos + 3],
            self.data[self.pos + 4], self.data[self.pos + 5],
            self.data[self.pos + 6], self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_i8(&mut self) -> Result<i8> { Ok(self.read_u8()? as i8) }
    fn read_i32(&mut self) -> Result<i32> { Ok(self.read_u32()? as i32) }

    fn read_f32(&mut self) -> Result<f32> {
        if self.remaining() < 4 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = f32::from_le_bytes([
            self.data[self.pos], self.data[self.pos + 1],
            self.data[self.pos + 2], self.data[self.pos + 3],
        ]);
        self.pos += 4;
        Ok(v)
    }

    fn read_f64(&mut self) -> Result<f64> {
        if self.remaining() < 8 { anyhow::bail!("EOF at pos {}", self.pos); }
        let v = f64::from_le_bytes([
            self.data[self.pos], self.data[self.pos + 1],
            self.data[self.pos + 2], self.data[self.pos + 3],
            self.data[self.pos + 4], self.data[self.pos + 5],
            self.data[self.pos + 6], self.data[self.pos + 7],
        ]);
        self.pos += 8;
        Ok(v)
    }

    fn read_string(&mut self) -> Result<String> {
        let len = self.read_u64()? as usize;
        if self.remaining() < len {
            anyhow::bail!("String length {} exceeds remaining data ({})", len, self.remaining());
        }
        let s = String::from_utf8_lossy(&self.data[self.pos..self.pos + len]).to_string();
        self.pos += len;
        Ok(s)
    }

    fn read_value(&mut self, value_type: u32) -> Result<GGUFValue> {
        match value_type {
            0 => Ok(GGUFValue::U8(self.read_u8()?)),
            1 => Ok(GGUFValue::I8(self.read_i8()?)),
            2 => Ok(GGUFValue::U16(self.read_u16()?)),
            3 => Ok(GGUFValue::I16(self.read_i16()?)),
            4 => Ok(GGUFValue::U32(self.read_u32()?)),
            5 => Ok(GGUFValue::I32(self.read_i32()?)),
            6 => Ok(GGUFValue::F32(self.read_f32()?)),
            7 => Ok(GGUFValue::Bool(self.read_u8()? != 0)),
            8 => Ok(GGUFValue::String(self.read_string()?)),
            9 => {
                // Array: read element type, then count, then elements
                let elem_type = self.read_u32()?;
                let count = self.read_u64()?;
                let mut arr = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    arr.push(self.read_value(elem_type)?);
                }
                Ok(GGUFValue::Array(arr))
            }
            10 => Ok(GGUFValue::U64(self.read_u64()?)),
            11 => Ok(GGUFValue::I64(self.read_u64()? as i64)),
            12 => Ok(GGUFValue::F64(self.read_f64()?)),
            other => anyhow::bail!("Unknown value type: {}", other),
        }
    }

    fn read_i16(&mut self) -> Result<i16> { Ok(self.read_u16()? as i16) }

    fn parse(&mut self) -> Result<GGUFModelInfo> {
        // Header
        if self.remaining() < 4 { anyhow::bail!("File too small for GGUF header"); }
        let magic = &self.data[0..4];
        self.pos = 4;
        if magic != GGUF_MAGIC {
            anyhow::bail!("Invalid GGUF magic: {:?}", magic);
        }

        let version = self.read_u32()?;
        if version != GGUF_VERSION {
            anyhow::bail!("Unsupported GGUF version: {}", version);
        }

        let tensor_count = self.read_u64()?;
        let kv_count = self.read_u64()?;
        info!("GGUF model: {} tensors, {} key-value pairs", tensor_count, kv_count);

        // Key-value pairs (GGUF v3 does NOT align individual values - only tensor data is aligned)
        let mut key_values = HashMap::new();
        for i in 0..kv_count {
            let key = self.read_string()?;
            let value_type = self.read_u32()?;
            let value = self.read_value(value_type)?;
            debug!("KV[{}]: {} = {:?}", i, key, value);
            key_values.insert(key, value);
        }

        // Extract architecture info
        let arch_str = key_values.get("general.architecture")
            .and_then(|v| if let GGUFValue::String(s) = v { Some(s.clone()) } else { None })
            .unwrap_or_else(|| "unknown".to_string());
        let architecture = ModelArch::from_string(&arch_str);

        let get_u32 = |key: &str| -> Option<u32> {
            key_values.get(key).and_then(|v| match v {
                GGUFValue::U32(n) => Some(*n),
                GGUFValue::U64(n) => Some(*n as u32),
                GGUFValue::I32(n) => Some(*n as u32),
                _ => None,
            })
        };

        let n_layers = get_u32(&format!("{}.block_count", arch_str));
        let n_embd = get_u32(&format!("{}.embedding_length", arch_str));
        let n_head = get_u32(&format!("{}.attention.head_count", arch_str));
        let n_vocab = get_u32("general.vocab_size");

        debug!("Architecture: {:?}, layers: {:?}, embd: {:?}, heads: {:?}, vocab: {:?}",
            architecture, n_layers, n_embd, n_head, n_vocab);

        // Tensor info - first pass: read all offsets
        let mut tensors = Vec::with_capacity(tensor_count as usize);
        let mut offsets = Vec::with_capacity(tensor_count as usize);

        for i in 0..tensor_count {
            let name = self.read_string()?;
            let n_dims = self.read_u32()? as usize;
            let mut dims = Vec::with_capacity(n_dims);
            for _ in 0..n_dims { dims.push(self.read_u64()?); }
            let dtype = GGMLType::from_u32(self.read_u32()?)?;
            let offset = self.read_u64()?;
            offsets.push(offset);

            let element_count: u64 = dims.iter().product();
            debug!("Tensor[{}]: {} dims={:?} type={:?} offset={}", i, name, dims, dtype, offset);
            tensors.push((name, dims, dtype, element_count));
        }

        // Second pass: use theoretical sizes (offset diffs include padding)
        let mut tensor_infos = Vec::with_capacity(tensor_count as usize);
        for i in 0..tensors.len() {
            let (name, dims, dtype, element_count) = &tensors[i];
            let offset = offsets[i];
            let theoretical_size = dtype.byte_size(*element_count as usize) as u64;

            tensor_infos.push(TensorInfo {
                name: name.clone(),
                dims: dims.clone(),
                dtype: *dtype,
                offset,
                size_bytes: theoretical_size,
            });
        }

        let tensors = tensor_infos;

        // Tensor data starts after metadata, aligned to alignment boundary
        let tensor_data_start = self.pos as u64;
        let total_size: u64 = tensors.iter().map(|t| t.size_bytes).sum();

        info!("✅ Parsed GGUF: {} tensors, {:.2} MB, tensor_data_start={}",
            tensors.len(), total_size as f64 / (1024.0 * 1024.0), tensor_data_start);

        Ok(GGUFModelInfo {
            key_values, tensors, architecture,
            n_layers, n_embd, n_head, n_vocab,
            tensor_data_start,
        })
    }
}

/// Parse GGUF file and return model info
pub fn parse_gguf<P: AsRef<Path>>(path: P) -> Result<GGUFModelInfo> {
    let path = path.as_ref();
    info!("Parsing GGUF file: {}", path.display());

    let file = std::fs::File::open(path)
        .with_context(|| format!("Failed to open GGUF file: {}", path.display()))?;
    let mmap = unsafe { memmap2::Mmap::map(&file) }
        .with_context(|| format!("Failed to memory map: {}", path.display()))?;

    let mut reader = GGUFReader::new(&mmap);
    reader.parse()
}
