#![allow(dead_code)]
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Folder {
    pub id: String,
    pub parent_id: Option<String>,
    pub name: String,
    pub sort_order: i32,
    pub create_time: String,
    pub update_time: String,
    pub deleted: i32,
    pub is_locked: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Document {
    pub id: String,
    pub folder_id: String,
    pub title: String,
    pub content: Option<String>,
    pub branch_name: Option<String>,
    pub sort_order: i32,
    pub create_time: String,
    pub update_time: String,
    pub deleted: i32,
    pub is_locked: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DocumentVersion {
    pub id: String,
    pub document_id: String,
    pub version_no: i32,
    pub title: Option<String>,
    pub content: Option<String>,
    pub content_hash: Option<String>,
    pub change_type: String,
    pub operator_id: Option<String>,
    pub create_time: String,
    pub update_time: String,
    pub deleted: i32,
}
