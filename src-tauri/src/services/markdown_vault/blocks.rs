//! Markdown 块切分与序列比对。
//!
//! 设计原则：块保存原始文本切片，`join_blocks` 只做拼接，不重新序列化。
//! 因此 `join_blocks(split_blocks(source)) == source` 必须字节级成立。

use serde::Serialize;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Block {
    /// frontmatter / heading / fence / list / quote / table / hr / blank / paragraph
    pub kind: String,
    /// 原始文本切片，包含尾随空行
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockChange {
    /// insert / delete / replace
    pub kind: String,
    pub index: usize,
    pub old_hash: Option<String>,
    pub new_hash: Option<String>,
}

pub fn content_hash(source: &str) -> String {
    let digest = Sha256::digest(source.as_bytes());
    format!("sha256:{}", hex(&digest))
}

fn hex(bytes: &[u8]) -> String {
    const TABLE: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(TABLE[(byte >> 4) as usize] as char);
        out.push(TABLE[(byte & 0x0f) as usize] as char);
    }
    out
}

fn trimmed_line(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn is_fence(line: &str) -> bool {
    let value = line.trim_start();
    value.starts_with("```") || value.starts_with("~~~")
}

fn block_kind(first_line: &str, is_first: bool) -> &'static str {
    let line = first_line.trim_end_matches(['\r', '\n']);
    let trimmed = line.trim();
    if is_first && trimmed == "---" {
        return "frontmatter";
    }
    if is_fence(line) {
        return "fence";
    }
    if trimmed.is_empty() {
        return "blank";
    }
    let leading = line.trim_start();
    if leading.starts_with('#') {
        let hashes = leading.chars().take_while(|c| *c == '#').count();
        if (1..=6).contains(&hashes) && leading[hashes..].starts_with(' ') {
            return "heading";
        }
    }
    if leading.starts_with('>') {
        return "quote";
    }
    if leading.starts_with("- ")
        || leading.starts_with("* ")
        || leading.starts_with("+ ")
        || leading.starts_with("1. ")
        || leading.starts_with("1) ")
    {
        return "list";
    }
    if leading.starts_with('|') {
        return "table";
    }
    if trimmed == "---" || trimmed == "***" || trimmed == "___" {
        return "hr";
    }
    "paragraph"
}

/// 按空行切块；frontmatter 和 fenced code 内部不切。
///
/// 每块包含从起点到下一块起点之前的全部文本（含尾随空行），
/// 这样拼接可以字节级还原原文。
pub fn split_blocks(source: &str) -> Vec<Block> {
    if source.is_empty() {
        return Vec::new();
    }
    let mut starts: Vec<usize> = vec![0];
    let mut offset = 0usize;
    let mut in_fence = false;
    let mut in_frontmatter = false;
    let mut pending_start = false;
    let mut first_line = true;

    for line in source.split_inclusive('\n') {
        let text = trimmed_line(line);
        let trimmed = text.trim();

        if first_line && trimmed == "---" {
            in_frontmatter = true;
            first_line = false;
            offset += line.len();
            continue;
        }
        first_line = false;

        if in_frontmatter {
            if trimmed == "---" {
                in_frontmatter = false;
                pending_start = true;
            }
            offset += line.len();
            continue;
        }

        if is_fence(text) {
            if in_fence {
                in_fence = false;
                pending_start = true;
            } else {
                if pending_start && offset > 0 {
                    starts.push(offset);
                }
                pending_start = false;
                in_fence = true;
            }
            offset += line.len();
            continue;
        }

        if in_fence {
            offset += line.len();
            continue;
        }

        if trimmed.is_empty() {
            pending_start = true;
        } else if pending_start && offset > 0 {
            starts.push(offset);
            pending_start = false;
        }
        offset += line.len();
    }

    starts.sort_unstable();
    starts.dedup();

    let mut blocks = Vec::with_capacity(starts.len());
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(source.len());
        if *start >= end {
            continue;
        }
        let text = &source[*start..end];
        let first = text.lines().next().unwrap_or("");
        blocks.push(Block {
            kind: block_kind(first, *start == 0).to_string(),
            text: text.to_string(),
            hash: content_hash(text),
        });
    }
    blocks
}

pub fn join_blocks(blocks: &[Block]) -> String {
    let mut out = String::new();
    for block in blocks {
        out.push_str(&block.text);
    }
    out
}

pub fn block_hashes(blocks: &[Block]) -> Vec<String> {
    blocks.iter().map(|block| block.hash.clone()).collect()
}

/// 简单的前缀/后缀 diff；中间变化整体标记为 replace。
///
/// 0.9.0 只用于本地预览和测试，真正逐块 LCS 在后续版本补上。
pub fn diff_block_hashes(old: &[String], new: &[String]) -> Vec<BlockChange> {
    let mut prefix = 0usize;
    while prefix < old.len() && prefix < new.len() && old[prefix] == new[prefix] {
        prefix += 1;
    }
    let mut suffix = 0usize;
    while suffix < old.len() - prefix
        && suffix < new.len() - prefix
        && old[old.len() - 1 - suffix] == new[new.len() - 1 - suffix]
    {
        suffix += 1;
    }

    let old_mid = &old[prefix..old.len() - suffix];
    let new_mid = &new[prefix..new.len() - suffix];
    let mut changes = Vec::new();

    if old_mid.is_empty() && new_mid.is_empty() {
        return changes;
    }
    if old_mid.is_empty() {
        for (offset, hash) in new_mid.iter().enumerate() {
            changes.push(BlockChange {
                kind: "insert".to_string(),
                index: prefix + offset,
                old_hash: None,
                new_hash: Some(hash.clone()),
            });
        }
        return changes;
    }
    if new_mid.is_empty() {
        for (offset, hash) in old_mid.iter().enumerate() {
            changes.push(BlockChange {
                kind: "delete".to_string(),
                index: prefix + offset,
                old_hash: Some(hash.clone()),
                new_hash: None,
            });
        }
        return changes;
    }

    let mut left = 0usize;
    let mut right = 0usize;
    while left < old_mid.len() || right < new_mid.len() {
        if left < old_mid.len() && right < new_mid.len() && old_mid[left] == new_mid[right] {
            left += 1;
            right += 1;
            continue;
        }
        if left < old_mid.len() && right < new_mid.len() {
            changes.push(BlockChange {
                kind: "replace".to_string(),
                index: prefix + right,
                old_hash: Some(old_mid[left].clone()),
                new_hash: Some(new_mid[right].clone()),
            });
            left += 1;
            right += 1;
            continue;
        }
        if left < old_mid.len() {
            changes.push(BlockChange {
                kind: "delete".to_string(),
                index: prefix + right,
                old_hash: Some(old_mid[left].clone()),
                new_hash: None,
            });
            left += 1;
        } else {
            changes.push(BlockChange {
                kind: "insert".to_string(),
                index: prefix + right,
                old_hash: None,
                new_hash: Some(new_mid[right].clone()),
            });
            right += 1;
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(source: &str) {
        let blocks = split_blocks(source);
        assert_eq!(join_blocks(&blocks), source);
    }

    #[test]
    fn round_trips_common_markdown() {
        round_trip("");
        round_trip("hello");
        round_trip("hello\n");
        round_trip("# 标题\n\n第一段。\n\n- a\n- b\n\n> 引用\n");
        round_trip("---\ntitle: 测试\n---\n\n正文\n");
        round_trip("```rust\nfn main() {}\n```\n\n后面\n");
        round_trip("| a | b |\n|---|---|\n| 1 | 2 |\n");
        round_trip("没有尾换行");
        round_trip("\n\n前导空行\n");
        round_trip("尾随空行\n\n\n");
        round_trip("CRLF\r\n第二行\r\n");
        round_trip("\u{feff}# BOM\n\n正文\n");
    }

    #[test]
    fn keeps_fence_in_one_block() {
        let blocks = split_blocks("```\n\n\n```\n\n正文\n");
        assert_eq!(blocks[0].kind, "fence");
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn detects_replace_and_insert() {
        let old = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let new = vec![
            "a".to_string(),
            "x".to_string(),
            "c".to_string(),
            "d".to_string(),
        ];
        let changes = diff_block_hashes(&old, &new);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].kind, "replace");
        assert_eq!(changes[1].kind, "insert");
    }

    #[test]
    fn hash_is_stable() {
        assert_eq!(content_hash("a"), content_hash("a"));
        assert_ne!(content_hash("a"), content_hash("b"));
    }
}
