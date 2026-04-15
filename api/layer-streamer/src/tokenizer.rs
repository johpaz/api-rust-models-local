use std::collections::HashMap;
use anyhow::Result;
use crate::gguf_parser::{GGUFModelInfo, GGUFValue};

/// Token type as stored in GGUF
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TokenType {
    Normal = 0,
    Unknown = 1,
    Control = 2,
    UserDefined = 3,
    Unused = 4,
    Byte = 5,
}

impl TokenType {
    fn from_int(n: u32) -> Self {
        match n {
            1 => Self::Unknown,
            2 => Self::Control,
            3 => Self::UserDefined,
            4 => Self::Unused,
            5 => Self::Byte,
            _ => Self::Normal,
        }
    }
}

/// BPE/SentencePiece tokenizer built from GGUF metadata.
///
/// Supports:
/// - `tokenizer.ggml.model = "llama"` / `"gemma"`: SentencePiece BPE (▁ for spaces)
/// - `tokenizer.ggml.model = "gpt2"`: Byte-level BPE (Ġ for spaces)
/// - Byte-fallback tokens `<0xNN>`
pub struct GGUFTokenizer {
    /// Vocabulary: token_id → token string
    pub vocab: Vec<String>,
    /// Reverse map: token string → token_id
    token_to_id: HashMap<String, u32>,
    /// BPE merge ranks: (left, right) → rank (lower = higher priority)
    merge_ranks: HashMap<(String, String), usize>,
    /// Special token IDs
    pub bos_token_id: u32,
    pub eos_token_id: u32,
    pub unk_token_id: u32,
    pub pad_token_id: Option<u32>,
    /// Token types (Normal, Control, Byte, etc.)
    pub token_types: Vec<TokenType>,
    /// Underlying tokenizer model: "llama", "gpt2", "gemma", etc.
    pub tokenizer_model: String,
}

impl GGUFTokenizer {
    /// Construct tokenizer from parsed GGUF metadata.
    pub fn from_model_info(info: &GGUFModelInfo) -> Result<Self> {
        // --- Vocabulary ---
        let vocab: Vec<String> = match info.key_values.get("tokenizer.ggml.tokens") {
            Some(GGUFValue::Array(tokens)) => tokens
                .iter()
                .filter_map(|v| {
                    if let GGUFValue::String(s) = v {
                        Some(s.clone())
                    } else {
                        None
                    }
                })
                .collect(),
            _ => anyhow::bail!("GGUF does not contain tokenizer.ggml.tokens"),
        };

        if vocab.is_empty() {
            anyhow::bail!("Tokenizer vocabulary is empty");
        }

        // --- Token types ---
        let token_types: Vec<TokenType> = match info.key_values.get("tokenizer.ggml.token_type") {
            Some(GGUFValue::Array(types)) => types
                .iter()
                .filter_map(|v| match v {
                    GGUFValue::I32(n) => Some(TokenType::from_int(*n as u32)),
                    GGUFValue::U32(n) => Some(TokenType::from_int(*n)),
                    _ => None,
                })
                .collect(),
            _ => vec![TokenType::Normal; vocab.len()],
        };

        // --- Reverse map ---
        let mut token_to_id = HashMap::with_capacity(vocab.len());
        for (id, token) in vocab.iter().enumerate() {
            token_to_id.insert(token.clone(), id as u32);
        }

        // --- BPE merge rules ---
        let merges_raw: Vec<(String, String)> =
            match info.key_values.get("tokenizer.ggml.merges") {
                Some(GGUFValue::Array(arr)) => arr
                    .iter()
                    .filter_map(|v| {
                        if let GGUFValue::String(s) = v {
                            let mut parts = s.splitn(2, ' ');
                            let a = parts.next()?.to_string();
                            let b = parts.next()?.to_string();
                            Some((a, b))
                        } else {
                            None
                        }
                    })
                    .collect(),
                _ => Vec::new(),
            };

        let mut merge_ranks = HashMap::with_capacity(merges_raw.len());
        for (rank, pair) in merges_raw.iter().enumerate() {
            merge_ranks.insert(pair.clone(), rank);
        }

        // --- Special tokens ---
        let bos_token_id = get_u32_kv(&info.key_values, "tokenizer.ggml.bos_token_id")
            .unwrap_or(1);
        let eos_token_id = get_u32_kv(&info.key_values, "tokenizer.ggml.eos_token_id")
            .unwrap_or(2);
        let unk_token_id = get_u32_kv(&info.key_values, "tokenizer.ggml.unknown_token_id")
            .unwrap_or(0);
        let pad_token_id = get_u32_kv(&info.key_values, "tokenizer.ggml.padding_token_id");

        let tokenizer_model = match info.key_values.get("tokenizer.ggml.model") {
            Some(GGUFValue::String(s)) => s.clone(),
            _ => "llama".to_string(),
        };

        tracing::info!(
            "Tokenizer loaded: model={}, vocab={}, merges={}, bos={}, eos={}",
            tokenizer_model,
            vocab.len(),
            merges_raw.len(),
            bos_token_id,
            eos_token_id,
        );

        Ok(Self {
            vocab,
            token_to_id,
            merge_ranks,
            bos_token_id,
            eos_token_id,
            unk_token_id,
            pad_token_id,
            token_types,
            tokenizer_model,
        })
    }

    pub fn vocab_size(&self) -> usize {
        self.vocab.len()
    }

    pub fn bos_token(&self) -> u32 {
        self.bos_token_id
    }

    pub fn eos_token(&self) -> u32 {
        self.eos_token_id
    }

    // ------------------------------------------------------------------
    // Encoding
    // ------------------------------------------------------------------

    /// Encode text to token IDs.
    ///
    /// `add_bos`: prepend the BOS token (recommended for new prompts).
    pub fn encode(&self, text: &str, add_bos: bool) -> Vec<u32> {
        let mut tokens = Vec::new();
        if add_bos {
            tokens.push(self.bos_token_id);
        }
        match self.tokenizer_model.as_str() {
            "gpt2" => tokens.extend(self.encode_gpt2(text)),
            _ => tokens.extend(self.encode_spm(text)),
        }
        tokens
    }

    /// SentencePiece BPE: prepend ▁ at word boundaries.
    fn encode_spm(&self, text: &str) -> Vec<u32> {
        if text.is_empty() {
            return Vec::new();
        }
        // Replace leading space / word-initial positions with ▁ (U+2581)
        let mut normalized = String::with_capacity(text.len() + 4);
        let mut prev_was_space = true; // start-of-string acts like space

        for c in text.chars() {
            if c == ' ' {
                prev_was_space = true;
            } else {
                if prev_was_space {
                    normalized.push('▁');
                    prev_was_space = false;
                }
                normalized.push(c);
            }
        }

        self.bpe_tokenize(&normalized)
    }

    /// GPT-2 byte-level BPE: Ġ prefix before each word (except the first).
    fn encode_gpt2(&self, text: &str) -> Vec<u32> {
        let mut all_tokens = Vec::new();
        for (i, word) in text.split_whitespace().enumerate() {
            let prefixed = if i > 0 {
                format!("Ġ{}", word)
            } else {
                word.to_string()
            };
            all_tokens.extend(self.bpe_tokenize(&prefixed));
        }
        all_tokens
    }

    /// Core BPE tokenization of a pre-processed string.
    ///
    /// Algorithm:
    /// 1. Split into individual characters.
    /// 2. Repeatedly find the pair with the lowest merge rank.
    /// 3. Merge that pair until no more merges apply.
    /// 4. Map symbols → token IDs, using byte-fallback for unknowns.
    fn bpe_tokenize(&self, text: &str) -> Vec<u32> {
        if text.is_empty() {
            return Vec::new();
        }

        // Start: one symbol per Unicode character
        let mut symbols: Vec<String> = text.chars().map(|c| c.to_string()).collect();

        // If there are no merge rules, just do a direct vocab lookup
        if self.merge_ranks.is_empty() {
            return symbols
                .iter()
                .flat_map(|s| self.lookup_with_fallback(s))
                .collect();
        }

        // Iteratively apply the best (lowest-rank) merge
        loop {
            let mut best_rank = usize::MAX;
            let mut best_pos = usize::MAX;

            for i in 0..symbols.len().saturating_sub(1) {
                let key = (symbols[i].clone(), symbols[i + 1].clone());
                if let Some(&rank) = self.merge_ranks.get(&key) {
                    if rank < best_rank {
                        best_rank = rank;
                        best_pos = i;
                    }
                }
            }

            if best_pos == usize::MAX {
                break; // no more merges possible
            }

            let merged = format!("{}{}", symbols[best_pos], symbols[best_pos + 1]);
            symbols[best_pos] = merged;
            symbols.remove(best_pos + 1);
        }

        // Map symbols to IDs, using byte fallback for unknown tokens
        symbols
            .iter()
            .flat_map(|s| self.lookup_with_fallback(s))
            .collect()
    }

    /// Look up a symbol in the vocab.
    /// If not found, try per-byte `<0xNN>` fallback tokens.
    /// Returns a list of IDs (usually 1, possibly more for multi-byte fallback).
    fn lookup_with_fallback(&self, s: &str) -> Vec<u32> {
        if let Some(&id) = self.token_to_id.get(s) {
            return vec![id];
        }

        // Byte fallback: encode each byte as <0xNN>
        let mut ids = Vec::new();
        for b in s.bytes() {
            let key = format!("<0x{:02X}>", b);
            if let Some(&id) = self.token_to_id.get(&key) {
                ids.push(id);
            } else {
                ids.push(self.unk_token_id);
            }
        }
        ids
    }

    // ------------------------------------------------------------------
    // Decoding
    // ------------------------------------------------------------------

    /// Decode token IDs back to a UTF-8 string.
    ///
    /// Handles:
    /// - SentencePiece ▁ → space
    /// - Byte-fallback tokens `<0xNN>` → raw byte
    pub fn decode(&self, tokens: &[u32]) -> String {
        let mut bytes: Vec<u8> = Vec::new();

        for &token_id in tokens {
            let id = token_id as usize;
            if id >= self.vocab.len() {
                continue;
            }
            let s = &self.vocab[id];

            // Byte fallback: <0xNN>
            if let Some(byte) = parse_byte_token(s) {
                bytes.push(byte);
                continue;
            }

            // SentencePiece: ▁ (U+2581) → space
            let decoded = s.replace('▁', " ");

            // GPT-2: Ġ (U+0120) → space
            let decoded = decoded.replace('Ġ', " ");

            bytes.extend_from_slice(decoded.as_bytes());
        }

        // Lossy UTF-8: handles partial byte sequences from byte fallbacks
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

/// Parse `<0xNN>` byte-fallback token string → byte value.
fn parse_byte_token(s: &str) -> Option<u8> {
    // Format: <0xNN> where NN is exactly 2 hex digits
    if s.len() == 6 && s.starts_with("<0x") && s.ends_with('>') {
        u8::from_str_radix(&s[3..5], 16).ok()
    } else {
        None
    }
}

fn get_u32_kv(kv: &HashMap<String, GGUFValue>, key: &str) -> Option<u32> {
    kv.get(key).and_then(|v| match v {
        GGUFValue::U32(n) => Some(*n),
        GGUFValue::I32(n) => Some(*n as u32),
        GGUFValue::U64(n) => Some(*n as u32),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_byte_token() {
        assert_eq!(parse_byte_token("<0x41>"), Some(0x41)); // 'A'
        assert_eq!(parse_byte_token("<0xFF>"), Some(0xFF));
        assert_eq!(parse_byte_token("<0x0A>"), Some(0x0A)); // '\n'
        assert_eq!(parse_byte_token("hello"), None);
        assert_eq!(parse_byte_token("<0x>"), None);
    }
}
