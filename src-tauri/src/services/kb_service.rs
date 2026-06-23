use crate::db;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbStatus {
    pub doc_count: i64,
    pub chunk_count: i64,
    pub embed_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbRebuildResult {
    pub total_docs: i64,
    pub total_chunks: i64,
    pub embed_success: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KbChunkDTO {
    pub document_id: String,
    pub document_title: String,
    pub chunk_index: i32,
    pub content: String,
    pub score: f32,
}

pub fn rebuild() -> Result<KbRebuildResult, String> {
    let count: i64 = db::with_db(|conn| {
        conn.query_row(
            "SELECT COUNT(*) FROM note_document WHERE deleted = 0 AND content IS NOT NULL AND content != ''",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    });
    let chunk_count = db::with_db(|conn| {
        conn.query_row("SELECT COUNT(*) FROM kb_chunks", [], |row| row.get(0))
            .unwrap_or(0)
    });
    Ok(KbRebuildResult {
        total_docs: count,
        total_chunks: chunk_count,
        embed_success: 0,
    })
}

pub fn search(_query: &str, _limit: Option<i32>) -> Result<Vec<KbChunkDTO>, String> {
    Ok(Vec::new())
}

pub fn status() -> Result<KbStatus, String> {
    let chunk_count: i64 = db::with_db(|conn| {
        conn.query_row("SELECT COUNT(*) FROM kb_chunks", [], |row| row.get(0))
            .unwrap_or(0)
    });
    let doc_count: i64 = db::with_db(|conn| {
        conn.query_row(
            "SELECT COUNT(DISTINCT document_id) FROM kb_chunks",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0)
    });
    Ok(KbStatus {
        doc_count,
        chunk_count,
        embed_count: 0,
    })
}
