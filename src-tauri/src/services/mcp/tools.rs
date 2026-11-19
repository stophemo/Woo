use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router, ErrorData, ServerHandler,
};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::Ordering,
    Arc,
};
use tauri::Emitter;

use super::{convert::ContentFormat, repo, McpShared, ToolError};

#[derive(Clone)]
pub struct WooMcp {
    shared: Arc<McpShared>,
    tool_router: ToolRouter<Self>,
}

impl WooMcp {
    pub fn new(shared: Arc<McpShared>) -> Self {
        Self {
            shared,
            tool_router: Self::tool_router(),
        }
    }

    fn writes_enabled(&self) -> bool {
        self.shared.allow_writes.load(Ordering::Relaxed)
    }

    fn after_write(&self, tool: &str, document_ids: Vec<String>, folder_ids: Vec<String>) {
        if let Some(app) = &self.shared.app_handle {
            let _ = app.emit(
                "sync-data-changed",
                serde_json::json!({
                    "source": "mcp",
                    "tool": tool,
                    "documentIds": document_ids,
                    "folderIds": folder_ids,
                }),
            );
            tauri::async_runtime::spawn(async {
                let _ = crate::services::sync_engine::sync_now_async().await;
            });
        }
    }
}

async fn blocking<T, F>(tool: &'static str, f: F) -> Result<T, ToolError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ToolError> + Send + 'static,
{
    let started = std::time::Instant::now();
    let result = match tokio::task::spawn_blocking(f).await {
        Ok(result) => result,
        Err(error) => Err(ToolError::Internal(format!("MCP 任务失败: {error}"))),
    };
    let duration_ms = started.elapsed().as_millis();
    match &result {
        Ok(_) => log::info!("[MCP] tool={tool} ok=true duration_ms={duration_ms}"),
        Err(error) => log::warn!(
            "[MCP] tool={tool} ok=false code={} duration_ms={duration_ms}",
            error.code()
        ),
    }
    result
}

fn success<T: Serialize>(value: &T) -> CallToolResult {
    let payload = serde_json::json!({ "ok": true, "data": value });
    CallToolResult::success(vec![ContentBlock::text(payload.to_string())])
}

fn from_result<T: Serialize>(result: Result<T, ToolError>) -> CallToolResult {
    match result {
        Ok(value) => success(&value),
        Err(error) => error.to_call_result(),
    }
}

// ============================================================
// Read tools
// ============================================================

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListNotesArgs {
    #[schemars(description = "Folder id. Omit to list recent notes across all folders.")]
    pub folder_id: Option<String>,
    #[schemars(description = "Maximum number of notes to return. Default 50, max 200.")]
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchNotesArgs {
    #[schemars(description = "Keyword to search in note titles and content.")]
    pub query: String,
    pub folder_id: Option<String>,
    #[schemars(description = "Default 20, max 100.")]
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetNoteArgs {
    #[schemars(description = "Note id returned by list_notes/search_notes.")]
    pub id: String,
    #[schemars(description = "markdown (default) or html.")]
    pub format: Option<ContentFormat>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListTrashArgs {
    #[schemars(description = "Default 50, max 200.")]
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ListVersionsArgs {
    pub document_id: String,
    #[schemars(description = "Default 20, max 200.")]
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GetVersionArgs {
    pub document_id: String,
    pub version_no: i32,
    pub format: Option<ContentFormat>,
}

#[tool_router]
impl WooMcp {
    #[tool(description = "List the folder tree with ids, names, parent ids and lock state. Call this before create_note/move_note if you need a folder id.")]
    async fn list_folders(&self) -> Result<CallToolResult, ErrorData> {
        let shared = self.shared.clone();
        let result = blocking("list_folders", move || repo::list_folders(&shared)).await;
        Ok(from_result(result))
    }

    #[tool(description = "List note metadata and short previews. Omit folderId to list recent notes across all folders. Does not return full content; use get_note for that.")]
    async fn list_notes(
        &self,
        Parameters(args): Parameters<ListNotesArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = repo::clamp_limit(args.limit, 50, 200);
        let offset = repo::clamp_offset(args.offset);
        let shared = self.shared.clone();
        let folder_id = args.folder_id;
        let result = blocking("list_notes", move || {
            repo::list_notes(&shared, folder_id.as_deref(), limit, offset)
        })
        .await;
        Ok(match result {
            Ok((items, has_more)) => success(&serde_json::json!({
                "items": items,
                "hasMore": has_more,
            })),
            Err(error) => error.to_call_result(),
        })
    }

    #[tool(description = "Search note titles and content. Returns matching notes with short snippets, not full content.")]
    async fn search_notes(
        &self,
        Parameters(args): Parameters<SearchNotesArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let query = args.query.trim().to_string();
        if query.is_empty() {
            return Ok(success(&serde_json::json!({
                "items": [],
                "hasMore": false,
            })));
        }
        if query.chars().count() > 200 {
            return Ok(ToolError::Invalid("搜索词超过 200 字符".to_string()).to_call_result());
        }
        let limit = repo::clamp_limit(args.limit, 20, 100);
        let offset = repo::clamp_offset(args.offset);
        let shared = self.shared.clone();
        let folder_id = args.folder_id;
        let result = blocking("search_notes", move || {
            repo::search_notes(&shared, &query, folder_id.as_deref(), limit, offset)
        })
        .await;
        Ok(match result {
            Ok((items, has_more)) => success(&serde_json::json!({
                "items": items,
                "hasMore": has_more,
            })),
            Err(error) => error.to_call_result(),
        })
    }

    #[tool(description = "Get a note by id. Returns title, content (Markdown by default), contentHash, updateTime and folder info. Locked notes are not accessible. Use the returned contentHash before update_note/append_note.")]
    async fn get_note(
        &self,
        Parameters(args): Parameters<GetNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let format = args.format.unwrap_or_default();
        let shared = self.shared.clone();
        let id = args.id;
        let result = blocking("get_note", move || repo::get_note(&shared, &id, format)).await;
        Ok(from_result(result))
    }

    #[tool(description = "List notes in the recycle bin. Deleted notes are not returned by list_notes/search_notes; use restore_note to recover them.")]
    async fn list_trash(
        &self,
        Parameters(args): Parameters<ListTrashArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = repo::clamp_limit(args.limit, 50, 200);
        let offset = repo::clamp_offset(args.offset);
        let shared = self.shared.clone();
        let result = blocking("list_trash", move || repo::list_trash(&shared, limit, offset)).await;
        Ok(match result {
            Ok((items, has_more)) => success(&serde_json::json!({
                "items": items,
                "hasMore": has_more,
            })),
            Err(error) => error.to_call_result(),
        })
    }

    #[tool(description = "List version history for a note. Returns version numbers, change types, timestamps and previews; use get_note_version for full content.")]
    async fn list_note_versions(
        &self,
        Parameters(args): Parameters<ListVersionsArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = repo::clamp_limit(args.limit, 20, 200);
        let offset = repo::clamp_offset(args.offset);
        let shared = self.shared.clone();
        let document_id = args.document_id;
        let result = blocking("list_note_versions", move || {
            repo::list_note_versions(&shared, &document_id, limit, offset)
        })
        .await;
        Ok(match result {
            Ok((items, has_more)) => success(&serde_json::json!({
                "items": items,
                "hasMore": has_more,
            })),
            Err(error) => error.to_call_result(),
        })
    }

    #[tool(description = "Get a specific note version's content by version number.")]
    async fn get_note_version(
        &self,
        Parameters(args): Parameters<GetVersionArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        let format = args.format.unwrap_or_default();
        let shared = self.shared.clone();
        let document_id = args.document_id;
        let version_no = args.version_no;
        let result = blocking("get_note_version", move || {
            repo::get_note_version(&shared, &document_id, version_no, format)
        })
        .await;
        Ok(from_result(result))
    }

    // ============================================================
    // Write tools
    // ============================================================

    #[tool(description = "Create a note in an existing folder. Requires write permission enabled in Woo settings. Content is Markdown by default; title is derived from the first line when omitted.")]
    async fn create_note(
        &self,
        Parameters(args): Parameters<CreateNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let format = args.format.unwrap_or_default();
        let shared = self.shared.clone();
        let folder_id = args.folder_id;
        let title = args.title;
        let content = args.content;
        let result = blocking("create_note", move || {
            repo::create_note(
                &shared,
                &folder_id,
                title.as_deref(),
                &content,
                format,
            )
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write(
                    "create_note",
                    vec![value.id.clone()],
                    value.folder_id.clone().into_iter().collect(),
                );
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Replace a note's content. Requires write permission. You MUST first call get_note and pass its contentHash as expectedContentHash; if the note changed since then, the tool returns a CONFLICT error. The title is preserved unless title or deriveTitle is provided.")]
    async fn update_note(
        &self,
        Parameters(args): Parameters<UpdateNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let format = args.format.unwrap_or_default();
        let shared = self.shared.clone();
        let id = args.id;
        let content = args.content;
        let title = args.title;
        let derive_title = args.derive_title.unwrap_or(false);
        let expected_hash = args.expected_content_hash;
        let expected_time = args.expected_update_time;
        let result = blocking("update_note", move || {
            repo::update_note(
                &shared,
                &id,
                &content,
                format,
                title.as_deref(),
                derive_title,
                &expected_hash,
                expected_time.as_deref(),
            )
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write("update_note", vec![value.id.clone()], Vec::new());
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Append Markdown/HTML content to an existing note. Requires write permission and the contentHash from get_note as expectedContentHash. The title is preserved.")]
    async fn append_note(
        &self,
        Parameters(args): Parameters<AppendNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let format = args.format.unwrap_or_default();
        let shared = self.shared.clone();
        let id = args.id;
        let content = args.content;
        let separator = args.separator;
        let expected_hash = args.expected_content_hash;
        let expected_time = args.expected_update_time;
        let result = blocking("append_note", move || {
            repo::append_note(
                &shared,
                &id,
                &content,
                format,
                separator.as_deref(),
                &expected_hash,
                expected_time.as_deref(),
            )
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write("append_note", vec![value.id.clone()], Vec::new());
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Rename a note without changing its content. Requires write permission. expectedContentHash/expectedUpdateTime are optional optimistic checks.")]
    async fn rename_note(
        &self,
        Parameters(args): Parameters<RenameNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let shared = self.shared.clone();
        let id = args.id;
        let title = args.title;
        let expected_hash = args.expected_content_hash;
        let expected_time = args.expected_update_time;
        let result = blocking("rename_note", move || {
            repo::rename_note(
                &shared,
                &id,
                &title,
                expected_hash.as_deref(),
                expected_time.as_deref(),
            )
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write("rename_note", vec![value.id.clone()], Vec::new());
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Move a note to another folder. Requires write permission. The source note and target folder must be unlocked.")]
    async fn move_note(
        &self,
        Parameters(args): Parameters<MoveNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let shared = self.shared.clone();
        let id = args.id;
        let folder_id = args.folder_id;
        let expected_time = args.expected_update_time;
        let result = blocking("move_note", move || {
            repo::move_note(&shared, &id, &folder_id, expected_time.as_deref())
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write(
                    "move_note",
                    vec![value.id.clone()],
                    vec![value.folder_id.clone()],
                );
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Create a folder. Requires write permission. parentId is optional; omit it for a root folder.")]
    async fn create_folder(
        &self,
        Parameters(args): Parameters<CreateFolderArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let shared = self.shared.clone();
        let name = args.name;
        let parent_id = args.parent_id;
        let result = blocking("create_folder", move || {
            repo::create_folder(&shared, &name, parent_id.as_deref())
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write("create_folder", Vec::new(), vec![value.id.clone()]);
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Rename a folder. Requires write permission. The folder must be unlocked.")]
    async fn rename_folder(
        &self,
        Parameters(args): Parameters<RenameFolderArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let shared = self.shared.clone();
        let folder_id = args.folder_id;
        let name = args.name;
        let result = blocking("rename_folder", move || repo::rename_folder(&shared, &folder_id, &name)).await;
        let output = match result {
            Ok(value) => {
                self.after_write("rename_folder", Vec::new(), vec![value.id.clone()]);
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Move a note to the recycle bin (soft delete). Requires write permission and confirm=true. Use restore_note to undo.")]
    async fn delete_note(
        &self,
        Parameters(args): Parameters<DeleteNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        if !args.confirm {
            return Ok(ToolError::Invalid(
                "delete_note 需要 confirm=true，并且应先向用户确认。".to_string(),
            )
            .to_call_result());
        }
        let shared = self.shared.clone();
        let id = args.id;
        let expected_time = args.expected_update_time;
        let result = blocking("delete_note", move || {
            repo::delete_note(&shared, &id, expected_time.as_deref())
        })
        .await;
        let output = match result {
            Ok(value) => {
                self.after_write("delete_note", vec![value.id.clone()], Vec::new());
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }

    #[tool(description = "Restore a note from the recycle bin. Requires write permission.")]
    async fn restore_note(
        &self,
        Parameters(args): Parameters<RestoreNoteArgs>,
    ) -> Result<CallToolResult, ErrorData> {
        if !self.writes_enabled() {
            return Ok(ToolError::ReadOnly.to_call_result());
        }
        let shared = self.shared.clone();
        let id = args.id;
        let result = blocking("restore_note", move || repo::restore_note(&shared, &id)).await;
        let output = match result {
            Ok(value) => {
                self.after_write(
                    "restore_note",
                    vec![value.id.clone()],
                    value.folder_id.clone().into_iter().collect(),
                );
                success(&value)
            }
            Err(error) => error.to_call_result(),
        };
        Ok(output)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for WooMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(
                "woo-notes",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Woo Notes 本地笔记服务。读取工具可自由调用；写工具需要用户在 Woo 菜单 MCP → MCP 服务设置… 中开启「允许 AI 修改笔记」。正文默认使用 Markdown。update_note/append_note 必须先调用 get_note 获取 contentHash 并传入 expectedContentHash。锁定文稿不可访问；删除是软删除（回收站）。",
            )
    }
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteArgs {
    #[schemars(description = "Target folder id. Use list_folders to discover ids.")]
    pub folder_id: String,
    #[schemars(description = "Optional title. Derived from the first non-empty line when omitted.")]
    pub title: Option<String>,
    #[schemars(description = "Note body. Markdown by default.")]
    pub content: String,
    pub format: Option<ContentFormat>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateNoteArgs {
    pub id: String,
    #[schemars(description = "New note body.")]
    pub content: String,
    pub format: Option<ContentFormat>,
    #[schemars(description = "Optional new title. Omit to keep the current title.")]
    pub title: Option<String>,
    #[schemars(description = "Set true to derive the title from the first line of content.")]
    pub derive_title: Option<bool>,
    #[schemars(description = "contentHash returned by get_note. Required for optimistic concurrency.")]
    pub expected_content_hash: String,
    #[schemars(description = "Optional updateTime returned by get_note.")]
    pub expected_update_time: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AppendNoteArgs {
    pub id: String,
    #[schemars(description = "Markdown/HTML content to append.")]
    pub content: String,
    pub format: Option<ContentFormat>,
    #[schemars(description = "Separator inserted before the new content. Defaults to a newline.")]
    pub separator: Option<String>,
    #[schemars(description = "contentHash returned by get_note. Required.")]
    pub expected_content_hash: String,
    pub expected_update_time: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenameNoteArgs {
    pub id: String,
    pub title: String,
    pub expected_content_hash: Option<String>,
    pub expected_update_time: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveNoteArgs {
    pub id: String,
    pub folder_id: String,
    pub expected_update_time: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateFolderArgs {
    pub name: String,
    pub parent_id: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenameFolderArgs {
    pub folder_id: String,
    pub name: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNoteArgs {
    pub id: String,
    #[schemars(description = "Must be true. Ask the user to confirm before deleting.")]
    pub confirm: bool,
    pub expected_update_time: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RestoreNoteArgs {
    pub id: String,
}
