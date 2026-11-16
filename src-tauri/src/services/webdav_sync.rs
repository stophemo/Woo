//! WebDAV（坚果云等）同步后端。
//!
//! 设计目标：零服务器、只同步变化的部分；远端可以是任意 WebDAV。
//!
//! 远端布局（根目录 = `url/folder`）：
//! ```text
//! manifest.json              同步清单：每条记录的 updateTime / hash / 删除标记 + 墓碑
//! documents/<id>.json        文稿整行数据
//! folders/<id>.json          文件夹整行数据
//! ```
//!
//! 同步采用「清单比对 + 最后写入胜出」：
//! - 本地行序列化后的 SHA-256 与清单里的 hash 不同，才上传；
//! - 远端 updateTime 更新，才下载；
//! - 本地硬删除通过「上一轮已知 ID - 当前本地 ID」生成墓碑，避免把远端误删；
//! - 墓碑保留 90 天，够慢设备追上。
//!
//! 版本历史、知识库分块不进入 WebDAV 同步，避免把免费流量耗在小文件上。

use crate::db;
use crate::services::sync_engine::{merge_into_local_for_user, SyncResult};
use crate::services::utils::{now_str, sha256};
use chrono::{Duration, FixedOffset, Utc};
use once_cell::sync::Lazy;
use reqwest::{Client, Method, StatusCode};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration as StdDuration;

const CONFIG_FILE: &str = "webdav-config.json";
const MANIFEST_FILE: &str = "manifest.json";
const MANIFEST_VERSION: i32 = 1;
const META_LAST_SYNC: &str = "webdav_last_sync_time";
const META_BACKOFF_UNTIL: &str = "webdav_backoff_until";
const META_DIRS_READY: &str = "webdav_dirs_ready";
const TOMBSTONE_KEEP_DAYS: i64 = 90;
/// 坚果云免费版限制 600 请求 / 30 分钟。按每分钟一轮计算，一轮最多
/// 12 次文件读写再加 1 次清单 HEAD，留足余量给 PROPFIND/重试和其他应用。
/// 剩下的变更留给 60 秒后的下一轮。
const MAX_FILE_OPERATIONS_PER_SYNC: usize = 12;

struct Entity {
    table: &'static str,
    dir: &'static str,
    known_meta: &'static str,
}

const ENTITIES: &[Entity] = &[
    Entity {
        table: "note_folder",
        dir: "folders",
        known_meta: "webdav_known_folders",
    },
    Entity {
        table: "note_document",
        dir: "documents",
        known_meta: "webdav_known_documents",
    },
];

// ============================================================
// Config
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WebdavConfig {
    pub enabled: bool,
    pub url: String,
    pub username: String,
    pub password: String,
    pub folder: String,
    /// 配置绑定的数据库账号 key（None = 未登录的本地库）。
    /// 退出登录后不会拿 A 账号的 WebDAV 配置去同步本地库，避免跨账号污染。
    pub bound_user: Option<String>,
}

impl Default for WebdavConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            username: String::new(),
            password: String::new(),
            folder: "Woo".to_string(),
            bound_user: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebdavConfigView {
    pub enabled: bool,
    pub active: bool,
    pub url: String,
    pub username: String,
    pub folder: String,
    pub has_password: bool,
    pub configured: bool,
}

fn config_path() -> PathBuf {
    db::data_dir().join(CONFIG_FILE)
}

pub fn load_config() -> WebdavConfig {
    let mut config = std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<WebdavConfig>(&text).ok())
        .unwrap_or_default();
    config.folder = sanitize_folder(&config.folder);
    config
}

fn config_is_active(config: &WebdavConfig) -> bool {
    config.enabled
        && !config.url.trim().is_empty()
        && config.bound_user == db::current_user_key()
}

fn config_view(config: &WebdavConfig) -> WebdavConfigView {
    WebdavConfigView {
        enabled: config.enabled,
        active: config_is_active(config),
        url: config.url.clone(),
        username: config.username.clone(),
        folder: config.folder.clone(),
        has_password: !config.password.is_empty(),
        configured: !config.url.trim().is_empty() && !config.username.trim().is_empty(),
    }
}

pub fn get_config_view() -> WebdavConfigView {
    config_view(&load_config())
}

pub fn is_enabled() -> bool {
    config_is_active(&load_config())
}

pub fn save_config_from_input(
    enabled: bool,
    url: &str,
    username: &str,
    password: Option<String>,
    folder: &str,
) -> Result<WebdavConfigView, String> {
    let existing = load_config();
    let url = if url.trim().is_empty() {
        String::new()
    } else {
        normalize_base_url(url)?
    };
    let username = username.trim().to_string();
    if enabled && url.is_empty() {
        return Err("请先填写 WebDAV 地址".to_string());
    }
    if enabled && username.is_empty() {
        return Err("请先填写 WebDAV 用户名".to_string());
    }

    // 只有同一个数据库账号才能复用已保存的密码；换账号后必须重新输入，
    // 否则 B 账号可以在设置页直接继承 A 账号的 WebDAV 凭据。
    let can_reuse_password = existing.bound_user == db::current_user_key()
        && username == existing.username;
    let password = match password {
        Some(value) if !value.is_empty() => value,
        _ if can_reuse_password => existing.password.clone(),
        _ => String::new(),
    };
    if enabled && password.is_empty() {
        return Err("请填写 WebDAV 应用密码".to_string());
    }

    let config = WebdavConfig {
        enabled,
        url,
        username,
        password,
        folder: sanitize_folder(folder),
        bound_user: db::current_user_key(),
    };
    write_atomic(
        &config_path(),
        &serde_json::to_vec_pretty(&config).map_err(|e| format!("保存 WebDAV 配置失败: {}", e))?,
    )?;
    let _ = set_meta(META_DIRS_READY, "");
    clear_manifest_cache();
    log::info!(
        "[WebDAV] 配置已保存 (enabled={}, url={}, folder={})",
        config.enabled,
        config.url,
        config.folder
    );
    Ok(config_view(&config))
}

pub async fn test_connection(
    url: &str,
    username: &str,
    password: Option<String>,
    folder: &str,
) -> Result<String, String> {
    let existing = load_config();
    let username = username.trim().to_string();
    let can_reuse_password = existing.bound_user == db::current_user_key()
        && username == existing.username;
    let password = match password {
        Some(value) if !value.is_empty() => value,
        _ if can_reuse_password => existing.password.clone(),
        _ => String::new(),
    };
    let config = WebdavConfig {
        enabled: true,
        url: normalize_base_url(url)?,
        username,
        password,
        folder: sanitize_folder(folder),
        bound_user: db::current_user_key(),
    };
    if config.username.is_empty() {
        return Err("请先填写 WebDAV 用户名".to_string());
    }

    ensure_dirs(&config).await?;
    let test_url = join_path(&remote_root(&config), "woo-write-test.json");
    put_bytes(&config, &test_url, b"{\"ok\":true}".to_vec()).await?;
    let _ = delete_file(&config, &test_url).await;
    Ok("连接成功，可读写".to_string())
}

/// 局域网同步走明文 HTTP，但只允许本机/私有网段，避免把配对码发到公网。
fn is_private_http_host(parsed: &reqwest::Url) -> bool {
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if host == "localhost" {
        return true;
    }
    if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
        return ip.is_loopback() || ip.is_private() || ip.is_link_local();
    }
    false
}

fn normalize_base_url(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err("请填写 WebDAV 地址".to_string());
    }
    let parsed = reqwest::Url::parse(trimmed).map_err(|_| "WebDAV 地址格式不正确".to_string())?;
    if parsed.scheme() != "https"
        && !(parsed.scheme() == "http" && is_private_http_host(&parsed))
    {
        return Err(
            "WebDAV 地址必须使用 https（本机或局域网地址允许 http）".to_string()
        );
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err("WebDAV 地址不能包含用户名或密码，请分别填写".to_string());
    }
    if parsed.query().is_some() || parsed.fragment().is_some() {
        return Err("WebDAV 地址不能带查询参数或片段".to_string());
    }
    if parsed.host_str().is_none() {
        return Err("WebDAV 地址缺少主机名".to_string());
    }
    Ok(trimmed.to_string())
}

/// 同步目录只允许单层，避免 MKCOL 逐级创建的复杂度和路径注入。
pub(crate) fn sanitize_folder(raw: &str) -> String {
    let cleaned: String = raw
        .trim()
        .trim_matches('/')
        .chars()
        .filter(|c| !matches!(c, '/' | '\\' | '?' | '#') && !c.is_control())
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        "Woo".to_string()
    } else {
        cleaned
    }
}

fn remote_root(config: &WebdavConfig) -> String {
    format!(
        "{}/{}",
        config.url.trim_end_matches('/'),
        sanitize_folder(&config.folder)
    )
}

fn join_path(root: &str, path: &str) -> String {
    format!(
        "{}/{}",
        root.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, bytes).map_err(|e| format!("写入临时配置失败: {}", e))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::rename(&temp, path).map_err(|e| {
        let _ = std::fs::remove_file(&temp);
        format!("保存配置失败: {}", e)
    })
}

// ============================================================
// HTTP
// ============================================================

static HTTP: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent(concat!("Woo/", env!("CARGO_PKG_VERSION"), " WebDAV"))
        .connect_timeout(StdDuration::from_secs(15))
        .timeout(StdDuration::from_secs(60))
        .build()
        .expect("Failed to create WebDAV client")
});

fn propfind_method() -> Method {
    Method::from_bytes(b"PROPFIND").expect("PROPFIND is a valid method")
}

fn mkcol_method() -> Method {
    Method::from_bytes(b"MKCOL").expect("MKCOL is a valid method")
}

fn network_error(error: impl std::fmt::Display) -> String {
    format!("网络错误: {}", error)
}

fn build_request(config: &WebdavConfig, method: Method, url: &str) -> reqwest::RequestBuilder {
    let mut request = HTTP.request(method, url).header("Accept", "*/*");
    if !config.username.is_empty() {
        request = request.basic_auth(&config.username, Some(&config.password));
    }
    request
}

fn http_error(action: &str, status: StatusCode, body: &str) -> String {
    match status {
        StatusCode::UNAUTHORIZED => {
            "WebDAV 认证失败：请确认用户名和密码；坚果云请使用「应用密码」而不是登录密码".to_string()
        }
        StatusCode::FORBIDDEN => format!("WebDAV 没有权限（{}）", action),
        StatusCode::NOT_FOUND => format!("WebDAV 路径不存在（{}）", action),
        StatusCode::TOO_MANY_REQUESTS => "云盘请求过于频繁，请稍后再试".to_string(),
        StatusCode::SERVICE_UNAVAILABLE => "云盘暂时不可用，请稍后再试".to_string(),
        _ => {
            let detail = body.trim();
            let detail: String = detail.chars().take(200).collect();
            if detail.is_empty() {
                format!("WebDAV 请求失败（{}）：HTTP {}", action, status.as_u16())
            } else {
                format!(
                    "WebDAV 请求失败（{}）：HTTP {} {}",
                    action,
                    status.as_u16(),
                    detail
                )
            }
        }
    }
}

async fn ensure_dirs(config: &WebdavConfig) -> Result<(), String> {
    let root = remote_root(config);
    let paths = [
        root.clone(),
        join_path(&root, "documents"),
        join_path(&root, "folders"),
    ];
    for path in paths {
        let response = build_request(config, propfind_method(), &path)
            .header("Depth", "0")
            .send()
            .await
            .map_err(network_error)?;
        let status = response.status();
        if status.is_success() {
            continue;
        }
        if status == StatusCode::NOT_FOUND || status == StatusCode::METHOD_NOT_ALLOWED {
            let response = build_request(config, mkcol_method(), &path)
                .send()
                .await
                .map_err(network_error)?;
            let status = response.status();
            if status.is_success() || status == StatusCode::METHOD_NOT_ALLOWED {
                continue;
            }
            let body = response.text().await.unwrap_or_default();
            return Err(http_error("创建同步目录", status, &body));
        }
        let body = response.text().await.unwrap_or_default();
        return Err(http_error("检查同步目录", status, &body));
    }
    Ok(())
}

fn header_string(response: &reqwest::Response, name: &str) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// 清单只在远端发生变化时才下载。坚果云免费版下载额度有限，
/// 每分钟整包 GET 一份清单很快就会把额度吃光。
fn read_manifest_cache() -> Option<ManifestCache> {
    let text = std::fs::read_to_string(db::data_dir().join("webdav-manifest-cache.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_manifest_cache(cache: &ManifestCache) -> Result<(), String> {
    let bytes = serde_json::to_vec(cache)
        .map_err(|e| format!("序列化清单缓存失败: {}", e))?;
    write_atomic(
        &db::data_dir().join("webdav-manifest-cache.json"),
        &bytes,
    )
}

fn clear_manifest_cache() {
    let _ = std::fs::remove_file(db::data_dir().join("webdav-manifest-cache.json"));
}

/// 清单 404 但本机缓存里有条目：说明远端清单被删/被搬走。
/// 此时直接按空清单上传会覆盖远端索引，先停下来让用户检查。
fn manifest_missing_error() -> String {
    "云端同步清单被删除，但本机记录显示其中仍有数据。为避免覆盖，请先检查 WebDAV 上的 Woo 目录；如需重新开始，请在 WebDAV 设置里重新保存一次。".to_string()
}

async fn get_manifest(config: &WebdavConfig) -> Result<(Manifest, bool), String> {
    let root = remote_root(config);
    let url = join_path(&root, MANIFEST_FILE);
    // 缓存按「目录 + 账号」隔离，换账号后不会误用另一个账号的清单。
    let cache_scope = format!("{}|{}", root, config.username);
    let cached = read_manifest_cache().filter(|cache| cache.root == cache_scope);

    // 先 HEAD 取 ETag / Last-Modified；未变化直接复用本地清单缓存。
    if let Ok(response) = build_request(config, Method::HEAD, &url).send().await {
        let status = response.status();
        if status == StatusCode::NOT_FOUND {
            if cached.as_ref().map_or(false, |cache| {
                !cache.manifest.documents.is_empty()
                    || !cache.manifest.folders.is_empty()
                    || !cache.manifest.tombstones.is_empty()
            }) {
                return Err(manifest_missing_error());
            }
            return Ok((Manifest::default(), false));
        }
        if status.is_success() {
            let etag = header_string(&response, "etag");
            let last_modified = header_string(&response, "last-modified");
            if let Some(cache) = cached.as_ref() {
                let etag_matches =
                    !etag.is_empty() && !cache.etag.is_empty() && etag == cache.etag;
                let modified_matches = !last_modified.is_empty()
                    && !cache.last_modified.is_empty()
                    && last_modified == cache.last_modified;
                if etag_matches || modified_matches {
                    let mut manifest = cache.manifest.clone();
                    manifest.version = MANIFEST_VERSION;
                    return Ok((manifest, true));
                }
            }
        }
        // HEAD 不受支持或校验器变化时继续走 GET，以拿到清单本体或明确错误。
    }

    let response = build_request(config, Method::GET, &url)
        .send()
        .await
        .map_err(network_error)?;
    let status = response.status();
    if status == StatusCode::NOT_FOUND {
        if cached.as_ref().map_or(false, |cache| {
            !cache.manifest.documents.is_empty()
                || !cache.manifest.folders.is_empty()
                || !cache.manifest.tombstones.is_empty()
        }) {
            return Err(manifest_missing_error());
        }
        return Ok((Manifest::default(), false));
    }
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(http_error("读取同步清单", status, &body));
    }
    let etag = header_string(&response, "etag");
    let last_modified = header_string(&response, "last-modified");
    let bytes = response.bytes().await.map_err(network_error)?;
    if bytes.is_empty() {
        return Ok((Manifest::default(), true));
    }
    let mut manifest: Manifest = serde_json::from_slice(&bytes).map_err(|e| {
        format!(
            "云端同步清单损坏，已停止同步以免覆盖数据：{}",
            e
        )
    })?;
    if manifest.version > MANIFEST_VERSION {
        return Err("云端同步清单来自更新版本的应用，请先升级 Woo".to_string());
    }
    manifest.version = MANIFEST_VERSION;
    let _ = write_manifest_cache(&ManifestCache {
        root: cache_scope,
        etag,
        last_modified,
        manifest: manifest.clone(),
    });
    Ok((manifest, true))
}

async fn put_bytes_with_headers(
    config: &WebdavConfig,
    url: &str,
    bytes: Vec<u8>,
) -> Result<(String, String), String> {
    let response = build_request(config, Method::PUT, url)
        .header("Content-Type", "application/json; charset=utf-8")
        .body(bytes)
        .send()
        .await
        .map_err(network_error)?;
    let status = response.status();
    if status.is_success() {
        Ok((
            header_string(&response, "etag"),
            header_string(&response, "last-modified"),
        ))
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(http_error("上传", status, &body))
    }
}

async fn put_bytes(config: &WebdavConfig, url: &str, bytes: Vec<u8>) -> Result<(), String> {
    put_bytes_with_headers(config, url, bytes).await.map(|_| ())
}

async fn delete_file(config: &WebdavConfig, url: &str) -> Result<(), String> {
    let response = build_request(config, Method::DELETE, url)
        .send()
        .await
        .map_err(network_error)?;
    let status = response.status();
    if status.is_success() || status == StatusCode::NOT_FOUND {
        Ok(())
    } else {
        let body = response.text().await.unwrap_or_default();
        Err(http_error("删除", status, &body))
    }
}

// ============================================================
// Manifest
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Manifest {
    version: i32,
    updated_at: String,
    documents: BTreeMap<String, ManifestEntry>,
    folders: BTreeMap<String, ManifestEntry>,
    /// key = `note_document:<id>` / `note_folder:<id>`，value = 删除时间
    tombstones: BTreeMap<String, String>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            version: MANIFEST_VERSION,
            updated_at: String::new(),
            documents: BTreeMap::new(),
            folders: BTreeMap::new(),
            tombstones: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestEntry {
    update_time: String,
    hash: String,
    deleted: i64,
}

/// HEAD 校验器 + 上次下载的清单。只存在本机，不参与同步。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct ManifestCache {
    root: String,
    etag: String,
    last_modified: String,
    manifest: Manifest,
}

impl Default for ManifestCache {
    fn default() -> Self {
        Self {
            root: String::new(),
            etag: String::new(),
            last_modified: String::new(),
            manifest: Manifest::default(),
        }
    }
}

impl Manifest {
    fn entries(&self, dir: &str) -> &BTreeMap<String, ManifestEntry> {
        if dir == "folders" {
            &self.folders
        } else {
            &self.documents
        }
    }

    fn entries_mut(&mut self, dir: &str) -> &mut BTreeMap<String, ManifestEntry> {
        if dir == "folders" {
            &mut self.folders
        } else {
            &mut self.documents
        }
    }
}

fn tombstone_key(table: &str, id: &str) -> String {
    format!("{}:{}", table, id)
}

#[derive(Debug, PartialEq)]
enum CompareAction {
    UpToDate,
    Upload,
    Pull,
}

fn compare_entry(
    local_hash: &str,
    local_update_time: &str,
    remote: Option<&ManifestEntry>,
) -> CompareAction {
    match remote {
        None => CompareAction::Upload,
        Some(entry) if entry.hash == local_hash => CompareAction::UpToDate,
        Some(entry) if local_update_time > entry.update_time.as_str() => CompareAction::Upload,
        Some(entry) if local_update_time < entry.update_time.as_str() => CompareAction::Pull,
        // 同一秒内两端都改过：用内容哈希做确定性平局决胜，避免双方每轮
        // 都认为「本地更新」而互相覆盖、永不收敛。
        Some(entry) if local_hash > entry.hash.as_str() => CompareAction::Upload,
        Some(_) => CompareAction::Pull,
    }
}

// ============================================================
// Local helpers
// ============================================================

fn get_meta(key: &str) -> Option<String> {
    db::with_db(|conn| {
        conn.query_row("SELECT value FROM sync_meta WHERE key = ?", [key], |row| {
            row.get(0)
        })
        .optional()
        .ok()
        .flatten()
    })
}

fn set_meta(key: &str, value: &str) -> Result<(), String> {
    db::with_db(|conn| -> Result<(), String> {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            [key, value],
        )
        .map(|_| ())
        .map_err(|e| format!("写入同步元数据失败: {}", e))
    })
}

fn set_meta_for_user(
    expected_user: Option<&str>,
    key: &str,
    value: &str,
) -> Result<(), String> {
    db::with_db_for_user(expected_user, |conn| -> Result<(), String> {
        conn.execute(
            "INSERT OR REPLACE INTO sync_meta (key, value) VALUES (?, ?)",
            [key, value],
        )
        .map(|_| ())
        .map_err(|e| format!("写入同步元数据失败: {}", e))
    })?
}

pub fn last_sync_time() -> Option<String> {
    get_meta(META_LAST_SYNC).filter(|value| !value.is_empty())
}

pub fn count_pending_changes() -> i32 {
    let Some(last_sync) = last_sync_time() else {
        return -1;
    };
    let mut total = 0i32;
    for entity in ENTITIES {
        let count: Option<i32> = db::with_db(|conn| {
            conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM {} WHERE update_time > ?",
                    entity.table
                ),
                [&last_sync],
                |row| row.get(0),
            )
            .ok()
        });
        total += count.unwrap_or(0);
    }
    total
}

fn read_known_ids(meta_key: &str) -> HashSet<String> {
    get_meta(meta_key)
        .and_then(|text| serde_json::from_str::<Vec<String>>(&text).ok())
        .unwrap_or_default()
        .into_iter()
        .collect()
}

fn load_local_rows(table: &str) -> Result<Vec<serde_json::Value>, String> {
    db::with_db(|conn| -> Result<Vec<serde_json::Value>, String> {
        let sql = format!("SELECT * FROM {}", table);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| format!("读取本地数据失败 [{}]: {}", table, e))?;
        let column_count = stmt.column_count();
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).map(String::from))
            .collect::<Result<_, _>>()
            .map_err(|e| format!("读取本地字段失败 [{}]: {}", table, e))?;
        let rows = stmt
            .query_map([], |row| {
                let mut map = serde_json::Map::new();
                for (i, name) in column_names.iter().enumerate() {
                    let value = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => serde_json::Value::Null,
                        rusqlite::types::ValueRef::Integer(v) => serde_json::Value::from(v),
                        rusqlite::types::ValueRef::Real(v) => serde_json::Value::from(v),
                        rusqlite::types::ValueRef::Text(v) => {
                            serde_json::Value::String(String::from_utf8_lossy(v).into_owned())
                        }
                        rusqlite::types::ValueRef::Blob(v) => {
                            serde_json::Value::String(String::from_utf8_lossy(v).into_owned())
                        }
                    };
                    map.insert(name.clone(), value);
                }
                Ok(serde_json::Value::Object(map))
            })
            .map_err(|e| format!("读取本地数据失败 [{}]: {}", table, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取本地数据失败 [{}]: {}", table, e))
    })
}

fn local_ids(table: &str) -> Result<Vec<String>, String> {
    db::with_db(|conn| -> Result<Vec<String>, String> {
        let mut stmt = conn
            .prepare(&format!("SELECT id FROM {}", table))
            .map_err(|e| format!("读取本地 ID 失败 [{}]: {}", table, e))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| format!("读取本地 ID 失败 [{}]: {}", table, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("读取本地 ID 失败 [{}]: {}", table, e))
    })
}

fn local_update_time(table: &str, id: &str) -> Result<Option<String>, String> {
    db::with_db(|conn| -> Result<Option<String>, String> {
        conn.query_row(
            &format!("SELECT update_time FROM {} WHERE id = ?", table),
            [id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("读取本地更新时间失败 [{}:{}]: {}", table, id, e))
    })
}

fn delete_local_rows(
    table: &str,
    ids: &[String],
    expected_user: Option<&str>,
) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    db::with_db_for_user(expected_user, |conn| -> Result<(), String> {
        let tx = conn
            .unchecked_transaction()
            .map_err(|e| format!("开始删除事务失败: {}", e))?;
        for id in ids {
            if table == "note_document" {
                tx.execute(
                    "DELETE FROM note_document_version WHERE document_id = ?",
                    [id],
                )
                .map_err(|e| format!("删除本地文稿版本失败 [{}]: {}", id, e))?;
            }
            tx.execute(&format!("DELETE FROM {} WHERE id = ?", table), [id])
                .map_err(|e| format!("应用墓碑失败 [{}:{}]: {}", table, id, e))?;
        }
        tx.commit()
            .map_err(|e| format!("提交删除事务失败: {}", e))
    })?
}

fn row_id(row: &serde_json::Value) -> Option<&str> {
    row.get("id").and_then(|value| value.as_str())
}

fn row_update_time(row: &serde_json::Value) -> String {
    row.get("update_time")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string()
}

fn row_deleted(row: &serde_json::Value) -> i64 {
    row.get("deleted")
        .and_then(|value| value.as_i64())
        .unwrap_or(0)
}

fn hash_row(row: &serde_json::Value) -> Result<String, String> {
    let text = serde_json::to_string(row)
        .map_err(|e| format!("序列化本地记录失败: {}", e))?;
    Ok(sha256(&text))
}

fn hash_bytes(bytes: &[u8]) -> Result<String, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| format!("云端记录编码无效: {}", e))?;
    Ok(sha256(text))
}

fn cutoff_days_ago(days: i64) -> String {
    let beijing = FixedOffset::east_opt(8 * 3600).expect("valid offset");
    (Utc::now().with_timezone(&beijing) - Duration::days(days))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

fn cutoff_seconds_ago(seconds: i64) -> String {
    let beijing = FixedOffset::east_opt(8 * 3600).expect("valid offset");
    (Utc::now().with_timezone(&beijing) - Duration::seconds(seconds))
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string()
}

// ============================================================
// Sync
// ============================================================

/// 网络等待期间可能发生登录/退出并切换数据库。每次写库前复核，
/// 避免把 A 账号的远端数据写进刚切换到的 B 账号库。
fn ensure_session(expected: &Option<String>) -> Result<(), String> {
    if &db::current_user_key() != expected {
        return Err("登录状态已切换，已取消本次同步".to_string());
    }
    Ok(())
}

pub async fn sync_now() -> Result<SyncResult, String> {
    let config = load_config();
    if !config.enabled {
        return Err("未启用 WebDAV 同步".to_string());
    }
    if config.url.trim().is_empty() {
        return Err("请先填写 WebDAV 地址".to_string());
    }
    if !config_is_active(&config) {
        return Err("WebDAV 配置属于另一个账号，请先在当前账号下重新启用".to_string());
    }
    let expected_user = db::current_user_key();
    sync_with_config(&config, &expected_user).await
}

/// 使用显式配置执行一轮同步。局域网同步也走这里，只是把 URL 换成本机服务。
pub(crate) async fn sync_with_config(
    config: &WebdavConfig,
    expected_user: &Option<String>,
) -> Result<SyncResult, String> {
    if let Some(until) = get_meta(META_BACKOFF_UNTIL) {
        if !until.is_empty() && until.as_str() > now_str().as_str() {
            return Err("云盘限流，稍后会自动重试".to_string());
        }
    }

    let dirs_ready = get_meta(META_DIRS_READY).as_deref() == Some("1");
    if !dirs_ready {
        ensure_dirs(&config).await?;
        ensure_session(expected_user)?;
        set_meta_for_user(expected_user.as_deref(), META_DIRS_READY, "1")?;
    }
    let (mut manifest, manifest_exists) = get_manifest(&config).await?;
    if !manifest_exists && dirs_ready {
        // 目录标记还在，但清单不在了：远端目录可能被删除/搬走。
        // 立即重建一次，避免本轮对着空路径上传失败。
        ensure_dirs(&config).await?;
        ensure_session(expected_user)?;
    }
    let sync_time = now_str();
    let root = remote_root(&config);

    let mut result = SyncResult {
        pushed_count: 0,
        pulled_count: 0,
        conflict_count: 0,
        cleanup_count: 0,
        tombstone_count: 0,
        sync_time: sync_time.clone(),
    };
    let mut manifest_dirty = !manifest_exists;
    let mut first_error: Option<String> = None;
    let mut truncated = false;
    let mut operations = 0usize;
    let mut uploads: Vec<(&'static str, String, serde_json::Value)> = Vec::new();

    for entity in ENTITIES {
        ensure_session(expected_user)?;
        let rows = load_local_rows(entity.table)?;
        let mut local_map: HashMap<String, serde_json::Value> = HashMap::new();
        let mut local_hashes: HashMap<String, String> = HashMap::new();
        for row in rows {
            let Some(id) = row_id(&row) else { continue };
            local_hashes.insert(id.to_string(), hash_row(&row)?);
            local_map.insert(id.to_string(), row);
        }
        let local_id_set: HashSet<String> = local_map.keys().cloned().collect();
        let known = read_known_ids(entity.known_meta);
        let remote_entries = manifest.entries(entity.dir).clone();

        // 1. 本地硬删除 → 墓碑 + 删除远端文件。
        for id in known.difference(&local_id_set).cloned().collect::<Vec<_>>() {
            let key = tombstone_key(entity.table, &id);
            if !manifest.tombstones.contains_key(&key) {
                manifest
                    .tombstones
                    .insert(key, sync_time.clone());
                manifest_dirty = true;
            }
            // 墓碑生效后清单里不能再保留这条记录，否则 90 天墓碑回收后
            // 新设备会把已删除的笔记重新拉回来。
            if manifest.entries_mut(entity.dir).remove(&id).is_some() {
                manifest_dirty = true;
            }
            let file_url = join_path(&root, &format!("{}/{}.json", entity.dir, id));
            if operations < MAX_FILE_OPERATIONS_PER_SYNC {
                match delete_file(&config, &file_url).await {
                    Ok(()) => operations += 1,
                    Err(error) => log::warn!("[WebDAV] 删除远端文件失败 [{}]: {}", id, error),
                }
            }
        }


        // 2. 本地已过清理窗口的 deleted=2 → 硬删除 + 墓碑，避免 WebDAV
        // 模式下的回收站数据永远留在本地和远端。
        let cleanup_cutoff = cutoff_seconds_ago(
            crate::services::sync_engine::parse_cleanup_seconds(),
        );
        let expired: Vec<String> = local_map
            .iter()
            .filter(|(_, row)| {
                row_deleted(row) == 2 && row_update_time(row).as_str() < cleanup_cutoff.as_str()
            })
            .map(|(id, _)| id.clone())
            .collect();
        if !expired.is_empty() {
            ensure_session(expected_user)?;
            delete_local_rows(entity.table, &expired, expected_user.as_deref())?;
            for id in &expired {
                local_map.remove(id);
                local_hashes.remove(id);
                manifest
                    .tombstones
                    .insert(tombstone_key(entity.table, id), sync_time.clone());
                manifest.entries_mut(entity.dir).remove(id);
                manifest_dirty = true;
                let file_url = join_path(&root, &format!("{}/{}.json", entity.dir, id));
                if operations < MAX_FILE_OPERATIONS_PER_SYNC {
                    match delete_file(&config, &file_url).await {
                        Ok(()) => operations += 1,
                        Err(error) => {
                            log::warn!("[WebDAV] 清理远端文件失败 [{}]: {}", id, error)
                        }
                    }
                }
            }
            result.cleanup_count += expired.len() as i32;
        }

        // 3. 应用远端墓碑。
        let prefix = format!("{}:", entity.table);
        let tombstone_ids: Vec<String> = manifest
            .tombstones
            .keys()
            .filter_map(|key| key.strip_prefix(&prefix))
            .filter(|id| local_map.contains_key(*id))
            .map(str::to_string)
            .collect();
        if !tombstone_ids.is_empty() {
            ensure_session(expected_user)?;
            delete_local_rows(entity.table, &tombstone_ids, expected_user.as_deref())?;
            for id in &tombstone_ids {
                local_map.remove(id);
                local_hashes.remove(id);
                if manifest.entries_mut(entity.dir).remove(id).is_some() {
                    manifest_dirty = true;
                }
            }
            result.tombstone_count += tombstone_ids.len() as i32;
        }

        // 4. 比对清单，决定上传/下载。
        let mut pulls: Vec<(String, String)> = Vec::new();
        for (id, row) in &local_map {
            let action = compare_entry(
                local_hashes.get(id).map(String::as_str).unwrap_or_default(),
                &row_update_time(row),
                remote_entries.get(id),
            );
            match action {
                CompareAction::UpToDate => {}
                CompareAction::Upload => {
                    if remote_entries.contains_key(id) {
                        result.conflict_count += 1;
                    }
                    uploads.push((entity.table, id.clone(), row.clone()));
                }
                CompareAction::Pull => {
                    if let Some(entry) = remote_entries.get(id) {
                        pulls.push((id.clone(), entry.update_time.clone()));
                    }
                }
            }
        }
        for (id, entry) in &remote_entries {
            if local_map.contains_key(id) {
                continue;
            }
            if manifest
                .tombstones
                .contains_key(&tombstone_key(entity.table, id))
            {
                continue;
            }
            // known 里有、本地已没有 = 本轮刚硬删除，已经写了墓碑，不能把它拉回来。
            if known.contains(id) {
                continue;
            }
            pulls.push((id.clone(), entry.update_time.clone()));
        }
        pulls.sort_by(|a, b| a.1.cmp(&b.1));

        // 5. 下载远端变更。
        for (id, _) in pulls {
            if operations >= MAX_FILE_OPERATIONS_PER_SYNC {
                truncated = true;
                break;
            }
            let file_url = join_path(&root, &format!("{}/{}.json", entity.dir, id));
            let response = build_request(&config, Method::GET, &file_url)
                .send()
                .await
                .map_err(network_error)?;
            let status = response.status();
            if status == StatusCode::NOT_FOUND {
                log::warn!("[WebDAV] 清单引用的文件不存在，移除陈旧条目 [{}:{}]", entity.table, id);
                if manifest.entries_mut(entity.dir).remove(&id).is_some() {
                    manifest_dirty = true;
                }
                continue;
            }
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                first_error = Some(http_error("下载", status, &body));
                break;
            }
            let bytes = response.bytes().await.map_err(network_error)?;
            if let Some(entry) = remote_entries.get(&id) {
                if hash_bytes(&bytes)? != entry.hash {
                    return Err(format!(
                        "云端记录校验失败 [{}:{}]，已停止同步以免写入损坏数据",
                        entity.table, id
                    ));
                }
            }
            let row: serde_json::Value = serde_json::from_slice(&bytes)
                .map_err(|e| format!("云端记录损坏 [{}:{}]: {}", entity.table, id, e))?;
            if row_id(&row) != Some(id.as_str()) {
                return Err(format!("云端记录与清单不一致 [{}:{}]", entity.table, id));
            }
            let remote_update = row_update_time(&row);
            let local_update = local_update_time(entity.table, &id)?;
            if local_update.is_none() && known.contains(&id) {
                // 本地在下载期间被硬删除：写入墓碑，不要把它拉回来。
                manifest
                    .tombstones
                    .insert(tombstone_key(entity.table, &id), sync_time.clone());
                if manifest.entries_mut(entity.dir).remove(&id).is_some() {
                    manifest_dirty = true;
                }
                continue;
            }
            if local_update
                .as_deref()
                .map_or(false, |value| value >= remote_update.as_str())
            {
                // 下载期间本地又改了：本地胜出。这里不急着上传——手上只有
                // 本轮开始时的旧快照，上传它反而会把最新本地修改和刚下载的
                // 远端修改一起盖掉。下一轮同步会用最新的本地行完成推送。
                result.conflict_count += 1;
                continue;
            }
            ensure_session(expected_user)?;
            let (pulled, conflicts, _) = merge_into_local_for_user(
                entity.table,
                "id",
                std::slice::from_ref(&row),
                expected_user.as_deref(),
            )?;
            result.pulled_count += pulled;
            result.conflict_count += conflicts;
            operations += 1;
        }

        if first_error.is_some() {
            break;
        }
    }

    // 6. 上传本地变更（含下载冲突时本地胜出的记录）。
    for (table, id, row) in uploads {
        if first_error.is_some() {
            break;
        }
        if operations >= MAX_FILE_OPERATIONS_PER_SYNC {
            truncated = true;
            break;
        }
        let entity = match ENTITIES.iter().find(|entity| entity.table == table) {
            Some(entity) => entity,
            None => continue,
        };
        let bytes = serde_json::to_vec(&row)
            .map_err(|e| format!("序列化本地记录失败 [{}:{}]: {}", table, id, e))?;
        let hash = hash_row(&row)?;
        let file_url = join_path(&root, &format!("{}/{}.json", entity.dir, id));
        match put_bytes(&config, &file_url, bytes).await {
            Ok(()) => {
                manifest.entries_mut(entity.dir).insert(
                    id,
                    ManifestEntry {
                        update_time: row_update_time(&row),
                        hash,
                        deleted: row_deleted(&row),
                    },
                );
                manifest_dirty = true;
                result.pushed_count += 1;
                operations += 1;
            }
            Err(error) => {
                if error.contains("路径不存在") {
                    let _ = set_meta_for_user(expected_user.as_deref(), META_DIRS_READY, "");
                }
                if error.contains("频繁") || error.contains("不可用") {
                    let until = (Utc::now().with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap())
                        + Duration::minutes(5))
                    .format("%Y-%m-%dT%H:%M:%S")
                    .to_string();
                    if ensure_session(expected_user).is_ok() {
                        let _ = set_meta_for_user(expected_user.as_deref(), META_BACKOFF_UNTIL, &until);
                    }
                }
                first_error = Some(error);
            }
        }
    }

    // 7. 清理过期墓碑并写回清单。文件名先落、清单后落，中断时最多留下无害的孤儿文件。
    manifest.tombstones.retain(|_, deleted_at| {
        deleted_at.as_str() >= cutoff_days_ago(TOMBSTONE_KEEP_DAYS).as_str()
    });
    let mut manifest_persisted = !manifest_dirty;
    if manifest_dirty {
        // 即使本轮因请求上限被截断，也要提交已经完成的变更（实体文件 + 清单）。
        // 否则下一轮会重新上传同一批文件，永远推进不到后面的数据。
        // 截断只阻止推进 last_sync_time，清单本身必须保持与实体文件一致。
        manifest.version = MANIFEST_VERSION;
        manifest.updated_at = sync_time.clone();
        let bytes = serde_json::to_vec(&manifest)
            .map_err(|e| format!("序列化同步清单失败: {}", e))?;
        match put_bytes_with_headers(&config, &join_path(&root, MANIFEST_FILE), bytes).await {
            Ok((etag, last_modified)) => {
                manifest_persisted = true;
                let _ = write_manifest_cache(&ManifestCache {
                    root: format!("{}|{}", root, config.username),
                    etag,
                    last_modified,
                    manifest: manifest.clone(),
                });
            }
            Err(error) => {
                if error.contains("路径不存在") {
                    let _ = set_meta_for_user(expected_user.as_deref(), META_DIRS_READY, "");
                }
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    // 8. 记录已知 ID，供下一轮识别本地硬删除。
    // 只有清单真正落盘后才更新，否则墓碑状态没发布却忘了本地硬删除，会复活数据。
    if manifest_persisted {
        ensure_session(expected_user)?;
        for entity in ENTITIES {
            let ids = local_ids(entity.table)?;
            if let Ok(text) = serde_json::to_string(&ids) {
                set_meta_for_user(expected_user.as_deref(), entity.known_meta, &text)?;
            }
        }
    }

    if first_error.is_none() && !truncated {
        ensure_session(expected_user)?;
        set_meta_for_user(expected_user.as_deref(), META_LAST_SYNC, &sync_time)?;
        set_meta_for_user(expected_user.as_deref(), META_BACKOFF_UNTIL, "")?;
    } else if truncated {
        log::info!("[WebDAV] 本轮达到请求上限，剩余变更将在下一轮继续");
    }

    log::info!(
        "[WebDAV] ⇄ push={} pull={} conflict={} tombstone={} ops={} truncated={}",
        result.pushed_count,
        result.pulled_count,
        result.conflict_count,
        result.tombstone_count,
        operations,
        truncated
    );

    match first_error {
        Some(error) => Err(error),
        None => Ok(result),
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_uses_hash_then_last_write_wins() {
        let remote = ManifestEntry {
            update_time: "2026-07-01T10:00:00".to_string(),
            hash: "remote-hash".to_string(),
            deleted: 0,
        };
        assert_eq!(
            compare_entry("remote-hash", "2026-06-01T10:00:00", Some(&remote)),
            CompareAction::UpToDate
        );
        assert_eq!(
            compare_entry("local-hash", "2026-07-02T10:00:00", Some(&remote)),
            CompareAction::Upload
        );
        assert_eq!(
            compare_entry("local-hash", "2026-06-02T10:00:00", Some(&remote)),
            CompareAction::Pull
        );
        // 同一秒：哈希较大的一方胜出，双方结果一致。
        assert_eq!(
            compare_entry("zzzz", "2026-07-01T10:00:00", Some(&remote)),
            CompareAction::Upload
        );
        assert_eq!(
            compare_entry("aaaa", "2026-07-01T10:00:00", Some(&remote)),
            CompareAction::Pull
        );
        assert_eq!(
            compare_entry("local-hash", "2026-01-01T00:00:00", None),
            CompareAction::Upload
        );
    }

    #[test]
    fn folder_is_sanitized_to_single_segment() {
        assert_eq!(sanitize_folder(""), "Woo");
        assert_eq!(sanitize_folder("/Notes/Woo/"), "NotesWoo");
        assert_eq!(sanitize_folder(".."), "Woo");
        assert_eq!(sanitize_folder("我的笔记"), "我的笔记");
    }

    #[test]
    fn manifest_roundtrip_keeps_tombstones() {
        let mut manifest = Manifest::default();
        manifest.tombstones.insert(
            tombstone_key("note_document", "abc"),
            "2026-07-01T10:00:00".to_string(),
        );
        manifest.documents.insert(
            "abc".to_string(),
            ManifestEntry {
                update_time: "2026-07-01T09:00:00".to_string(),
                hash: "hash".to_string(),
                deleted: 1,
            },
        );
        let text = serde_json::to_string(&manifest).unwrap();
        let parsed: Manifest = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.version, MANIFEST_VERSION);
        assert!(parsed
            .tombstones
            .contains_key("note_document:abc"));
        assert_eq!(parsed.documents.get("abc").unwrap().deleted, 1);
    }

    #[test]
    fn hash_changes_when_content_changes() {
        let first = serde_json::json!({ "id": "a", "content": "第一版" });
        let second = serde_json::json!({ "id": "a", "content": "第二版" });
        assert_ne!(hash_row(&first).unwrap(), hash_row(&second).unwrap());
    }

    #[test]
    fn tombstone_key_is_table_scoped() {
        assert_eq!(
            tombstone_key("note_document", "x"),
            "note_document:x"
        );
        assert_ne!(
            tombstone_key("note_document", "x"),
            tombstone_key("note_folder", "x")
        );
    }

    /// 本地冒烟测试（默认忽略，需要本机 WebDAV 服务）：
    /// ```text
    /// python3 -m venv /tmp/woo-wsgi
    /// /tmp/woo-wsgi/bin/pip install wsgidav cheroot
    /// /tmp/woo-wsgi/bin/wsgidav --host 127.0.0.1 --port 18080 \
    ///   --root /tmp/woo-webdav --auth anonymous
    /// cargo test smoke_sync_against_local_webdav --lib -- --ignored
    /// ```
    /// 覆盖：首次播种 → 单篇增量 → 新设备恢复 → 删除墓碑传播。
    #[tokio::test]
    #[ignore]
    async fn smoke_sync_against_local_webdav() {
        let folder = format!("WooSmoke{}", uuid::Uuid::new_v4().simple());
        let device_a = std::env::temp_dir().join(format!("woo-webdav-a-{}", uuid::Uuid::new_v4()));
        let device_b = std::env::temp_dir().join(format!("woo-webdav-b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&device_a).unwrap();
        std::fs::create_dir_all(&device_b).unwrap();

        let config = WebdavConfig {
            enabled: true,
            url: "http://127.0.0.1:18080".to_string(),
            username: String::new(),
            password: String::new(),
            folder: folder.clone(),
            bound_user: None,
        };
        let write_config = |dir: &std::path::Path| {
            std::fs::write(dir.join(CONFIG_FILE), serde_json::to_vec(&config).unwrap()).unwrap();
        };

        db::set_data_dir(device_a.clone());
        db::close_db();
        db::set_current_user(None);
        write_config(&device_a);
        db::with_db(|conn| {
            conn.execute(
                "INSERT INTO note_folder (id,parent_id,name,sort_order,create_time,update_time,deleted,is_locked)
                 VALUES ('f1',NULL,'测试目录',0,'2026-11-16T10:00:00','2026-11-16T10:00:00',0,0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO note_document (id,folder_id,title,content,branch_name,sort_order,create_time,update_time,deleted,is_locked)
                 VALUES ('d1','f1','第一版','<p>正文</p>',NULL,0,'2026-11-16T10:00:00','2026-11-16T10:00:00',0,0)",
                [],
            ).unwrap();
        });

        let first = sync_now().await.unwrap();
        assert_eq!(first.pushed_count, 2, "seed sync should push folder+doc");

        // 修改本地文稿，第二轮只推送这一篇。
        db::with_db(|conn| {
            conn.execute(
                "UPDATE note_document SET content = '<p>正文第二版</p>', update_time = '2026-11-16T10:05:00' WHERE id = 'd1'",
                [],
            ).unwrap();
        });
        let second = sync_now().await.unwrap();
        assert_eq!(second.pushed_count, 1, "changed doc should be pushed");

        // 新设备空库恢复。
        db::close_db();
        db::set_data_dir(device_b.clone());
        db::set_current_user(None);
        write_config(&device_b);
        let third = sync_now().await.unwrap();
        assert_eq!(third.pulled_count, 2, "new device should pull folder+doc");
        let content: String = db::with_db(|conn| {
            conn.query_row("SELECT content FROM note_document WHERE id = 'd1'", [], |row| row.get(0)).unwrap()
        });
        assert_eq!(content, "<p>正文第二版</p>");

        // 设备 B 删除文稿并同步，设备 A 下一轮应收到墓碑。
        db::with_db(|conn| {
            conn.execute("DELETE FROM note_document WHERE id = 'd1'", []).unwrap();
        });
        let _ = sync_now().await.unwrap();

        db::close_db();
        db::set_data_dir(device_a.clone());
        db::set_current_user(None);
        let fourth = sync_now().await.unwrap();
        assert!(fourth.tombstone_count >= 1, "device A should apply tombstone");
        let remaining: i64 = db::with_db(|conn| {
            conn.query_row("SELECT COUNT(*) FROM note_document WHERE id = 'd1'", [], |row| row.get(0)).unwrap()
        });
        assert_eq!(remaining, 0);

        let _ = std::fs::remove_dir_all(device_a);
        let _ = std::fs::remove_dir_all(device_b);
    }
}
