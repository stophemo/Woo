#![allow(dead_code)]
/// Stub embedding service — uses @xenova/transformers in Electron,
/// will be ported to Rust with candle or kept as Tauri sidecar

pub fn get_status() -> &'static str {
    "not_available"
}

pub async fn generate_embedding(_text: &str) -> Result<Vec<f32>, String> {
    Err("嵌入模型尚未加载（Tauri 迁移中）".to_string())
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum();
    let norm_b: f32 = b.iter().map(|x| x * x).sum();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}
