use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use sha2::{Digest, Sha256};

use crate::wire::{ChunkRef, MAX_CIPHERTEXT_BYTES};

/// 单块明文的硬上限。密文 = 压缩块 + 16 字节 GCM tag，服务端限制 32 KiB。
pub const MAX_CHUNK_BYTES: usize = 24 * 1024;
const MIN_CHUNK_BYTES: usize = 4 * 1024;
const CHUNK_MASK: u64 = (1 << 14) - 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub hash: String,
    /// 压缩后的字节；未压缩时与明文相同。
    pub stored: Vec<u8>,
    pub plain_bytes: usize,
    pub compressed: bool,
}

impl Chunk {
    pub fn entity_id(&self) -> String {
        format!("chunk:{}", self.hash)
    }

    pub fn to_ref(&self) -> ChunkRef {
        ChunkRef {
            hash: self.hash.clone(),
            plain_bytes: self.plain_bytes as u32,
            stored_bytes: self.stored.len() as u32,
            compressed: self.compressed,
        }
    }
}

/// 按内容定义边界切分正文。相同内容无论前后文如何变化，边界尽量保持稳定，
/// 因此编辑一段文字只会重传少量块。
pub fn chunk_content(content: &str) -> Vec<Chunk> {
    let bytes = content.as_bytes();
    if bytes.is_empty() {
        return Vec::new();
    }
    let gear = gear_table();
    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut hash = 0u64;
    for (index, byte) in bytes.iter().enumerate() {
        hash = hash
            .wrapping_shl(1)
            .wrapping_add(gear[*byte as usize]);
        let length = index + 1 - start;
        let at_boundary = length >= MIN_CHUNK_BYTES && (hash & CHUNK_MASK) == 0;
        if at_boundary || length >= MAX_CHUNK_BYTES {
            chunks.push(make_chunk(&bytes[start..=index]));
            start = index + 1;
            hash = 0;
        }
    }
    if start < bytes.len() {
        chunks.push(make_chunk(&bytes[start..]));
    }
    chunks
}

fn make_chunk(plain: &[u8]) -> Chunk {
    debug_assert!(plain.len() <= MAX_CHUNK_BYTES);
    let compressed = compress(plain);
    // 压缩后反而更大时保留原文；两者都远低于 32 KiB 密文上限。
    let (stored, compressed_flag) = if compressed.len() + 16 < plain.len() {
        (compressed, true)
    } else {
        (plain.to_vec(), false)
    };
    debug_assert!(stored.len() + 16 <= MAX_CIPHERTEXT_BYTES);
    // 哈希覆盖“压缩标志 + 实际存储字节”，这样压缩实现或版本变化时
    // 不会把不同存储形式误认为同一个内容块。
    let hash = hex_chunk_hash(compressed_flag, &stored);
    Chunk {
        hash,
        stored,
        plain_bytes: plain.len(),
        compressed: compressed_flag,
    }
}

/// 把一个已经完整的内存块（例如内容清单 JSON）打包成可同步的 Chunk。
pub fn chunk_bytes(data: &[u8]) -> Result<Chunk, String> {
    if data.is_empty() {
        return Err("内容块不能为空".to_string());
    }
    let compressed = compress(data);
    let (stored, compressed_flag) = if compressed.len() + 16 < data.len() {
        (compressed, true)
    } else {
        (data.to_vec(), false)
    };
    if stored.len() + 16 > MAX_CIPHERTEXT_BYTES {
        return Err("内容清单分片超过单条同步上限".to_string());
    }
    Ok(Chunk {
        hash: hex_chunk_hash(compressed_flag, &stored),
        stored,
        plain_bytes: data.len(),
        compressed: compressed_flag,
    })
}

pub fn decompress_chunk(stored: &[u8], compressed: bool) -> Result<Vec<u8>, String> {
    if !compressed {
        return Ok(stored.to_vec());
    }
    let mut decoder = ZlibDecoder::new(stored);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|error| format!("同步块解压失败：{error}"))?;
    Ok(output)
}

pub fn verify_chunk_ref(chunk_ref: &ChunkRef, stored: &[u8]) -> Result<(), String> {
    if hex_chunk_hash(chunk_ref.compressed, stored) != chunk_ref.hash {
        return Err("同步块哈希与清单不一致".to_string());
    }
    Ok(())
}

pub fn verify_chunk(chunk_ref: &ChunkRef, plain: &[u8]) -> Result<(), String> {
    if plain.len() != chunk_ref.plain_bytes as usize {
        return Err("同步块长度与清单不一致".to_string());
    }
    Ok(())
}

fn compress(data: &[u8]) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(data).is_err() {
        return Vec::new();
    }
    encoder.finish().unwrap_or_default()
}

fn hex_chunk_hash(compressed: bool, stored: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update([u8::from(compressed)]);
    hasher.update(stored);
    let digest = hasher.finalize();
    let mut output = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// 固定种子的 gear 表；只影响分块边界，不涉及安全。
fn gear_table() -> [u64; 256] {
    let mut table = [0u64; 256];
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    for value in &mut table {
        state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        *value = z ^ (z >> 31);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reassemble(chunks: &[Chunk]) -> String {
        let mut bytes = Vec::new();
        for chunk in chunks {
            bytes.extend_from_slice(
                &decompress_chunk(&chunk.stored, chunk.compressed).unwrap(),
            );
        }
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn empty_content_has_no_chunks() {
        assert!(chunk_content("").is_empty());
    }

    #[test]
    fn round_trips_text_and_respects_size_limits() {
        let content = "<p>无我笔记</p>".repeat(12_000);
        let chunks = chunk_content(&content);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(chunk.plain_bytes <= MAX_CHUNK_BYTES);
            assert!(chunk.stored.len() + 16 <= MAX_CIPHERTEXT_BYTES);
            assert_eq!(chunk.hash.len(), 64);
            assert!(chunk.entity_id().starts_with("chunk:"));
            let plain = decompress_chunk(&chunk.stored, chunk.compressed).unwrap();
            verify_chunk_ref(&chunk.to_ref(), &chunk.stored).unwrap();
            verify_chunk(&chunk.to_ref(), &plain).unwrap();
        }
        assert_eq!(reassemble(&chunks), content);
    }

    #[test]
    fn editing_the_tail_preserves_earlier_chunks() {
        let prefix = "一段足够长的正文。".repeat(8_000);
        let original = format!("{prefix}尾巴一");
        let edited = format!("{prefix}尾巴二");
        let original_chunks = chunk_content(&original);
        let edited_chunks = chunk_content(&edited);
        let original_hashes: std::collections::HashSet<_> =
            original_chunks.iter().map(|chunk| &chunk.hash).collect();
        let preserved = edited_chunks
            .iter()
            .filter(|chunk| original_hashes.contains(&chunk.hash))
            .count();
        assert!(
            preserved + 2 >= edited_chunks.len(),
            "仅修改尾部时大部分块应复用：preserved={preserved}, total={}",
            edited_chunks.len()
        );
    }

    #[test]
    fn multi_byte_utf8_survives_arbitrary_boundaries() {
        let content = "😀中文é".repeat(20_000);
        let chunks = chunk_content(&content);
        assert_eq!(reassemble(&chunks), content);
    }
}
