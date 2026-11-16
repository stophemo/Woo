//! 局域网同步（完全离线）。
//!
//! 桌面端在本机启动一个只面向局域网的 WebDAV 服务：
//! - 手机和电脑连同一个 Wi-Fi，手机在 WebDAV 设置里填 `http://电脑IP:端口`；
//! - 用户名固定 `woo`，密码用桌面端生成的配对码；
//! - 数据落在 `{dataDir}/lan-sync/`，桌面端自己也通过回环 WebDAV 同步到同一目录。
//!
//! 不走互联网、不依赖 Supabase/坚果云。服务只实现了 WoO 同步需要的最小
//! WebDAV 子集（PROPFIND/MKCOL/GET/HEAD/PUT/DELETE/OPTIONS），不做目录列表。

use crate::db;
use crate::services::sync_engine::SyncResult;
use crate::services::webdav_sync::{self, WebdavConfig};
use base64::Engine;
use once_cell::sync::Lazy;
use percent_encoding::percent_decode_str;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use tiny_http::{Header, Request, Response, Server, StatusCode};

const CONFIG_FILE: &str = "lan-sync-config.json";
const DEFAULT_PORT: u16 = 45231;
/// 手机通过 UDP 广播发现桌面端的固定端口（与 WebDAV 端口无关）。
const PAIR_PORT: u16 = 45232;
const MAX_PUT_BYTES: u64 = 64 * 1024 * 1024;
const USERNAME: &str = "woo";
const PROPFIND_XML: &str =
    r#"<?xml version="1.0" encoding="utf-8"?><D:multistatus xmlns:D="DAV:"></D:multistatus>"#;

// ============================================================
// Config
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LanSyncConfig {
    pub enabled: bool,
    pub port: u16,
    pub token: String,
    pub folder: String,
    pub bound_user: Option<String>,
    /// 手机端输入的 6 位配对码；只用于 UDP 发现，不参与 WebDAV 认证。
    pub pair_code: String,
}

impl Default for LanSyncConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            port: DEFAULT_PORT,
            token: String::new(),
            folder: "Woo".to_string(),
            bound_user: None,
            pair_code: String::new(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanSyncView {
    pub enabled: bool,
    pub active: bool,
    pub running: bool,
    pub port: u16,
    pub token: String,
    pub folder: String,
    /// 6 位配对码：手机在 WebDAV 页输入后通过 UDP 自动发现本机。
    pub pair_code: String,
    /// 形如 http://192.168.1.23:45231；未启用或找不到局域网 IP 时为 null。
    pub address: Option<String>,
}

fn config_path() -> PathBuf {
    db::data_dir().join(CONFIG_FILE)
}

pub fn load_config() -> LanSyncConfig {
    let mut config = fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<LanSyncConfig>(&text).ok())
        .unwrap_or_default();
    config.folder = webdav_sync::sanitize_folder(&config.folder);
    if config.port == 0 {
        config.port = DEFAULT_PORT;
    }
    config
}

fn save_config(config: &LanSyncConfig) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(config)
        .map_err(|e| format!("序列化局域网同步配置失败: {}", e))?;
    webdav_sync::write_atomic(&config_path(), &bytes)
}

fn new_token() -> String {
    uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(12)
        .collect()
}

fn new_pair_code() -> String {
    format!("{:06}", rand::random::<u32>() % 1_000_000)
}

fn ensure_pair_code(config: &mut LanSyncConfig) -> bool {
    if config.pair_code.len() == 6 && config.pair_code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    config.pair_code = new_pair_code();
    true
}

fn lan_root() -> PathBuf {
    db::data_dir().join("lan-sync")
}

/// 找到当前默认路由对应的局域网 IPv4（不会真的发包）。
fn lan_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() => Some(ip.to_string()),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanPairResult {
    pub url: String,
    pub username: String,
    pub password: String,
    pub folder: String,
}

/// 手机端：用 6 位配对码在局域网广播发现桌面端。
/// 返回可用的 WebDAV 地址、用户名、令牌和目录；不依赖手动输入 IP。
pub fn discover_pair(code: &str) -> Result<LanPairResult, String> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err("请输入 6 位数字配对码".to_string());
    }
    let socket = UdpSocket::bind(("0.0.0.0", 0))
        .map_err(|e| format!("无法创建配对连接: {}", e))?;
    socket.set_broadcast(true)
        .map_err(|e| format!("无法启用局域网广播: {}", e))?;
    socket.set_read_timeout(Some(Duration::from_millis(700)))
        .map_err(|e| format!("设置配对超时失败: {}", e))?;

    let request = format!("WOO-PAIR {}", code);
    let mut targets = vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::BROADCAST), PAIR_PORT)];
    if let Ok(interfaces) = if_addrs::get_if_addrs() {
        for iface in interfaces {
            if iface.is_loopback() {
                continue;
            }
            if let if_addrs::IfAddr::V4(v4) = iface.addr {
                let ip = u32::from(v4.ip);
                let mask = u32::from(v4.netmask);
                if ip == 0 || v4.ip.is_loopback() || v4.ip.is_link_local() {
                    continue;
                }
                let broadcast = Ipv4Addr::from(ip | !mask);
                targets.push(SocketAddr::new(IpAddr::V4(broadcast), PAIR_PORT));
            }
        }
    }
    targets.sort();
    targets.dedup();
    for target in &targets {
        let _ = socket.send_to(request.as_bytes(), target);
    }

    let deadline = std::time::Instant::now() + Duration::from_secs(4);
    let mut buf = [0u8; 768];
    while std::time::Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                let text = String::from_utf8_lossy(&buf[..n]);
                let value: serde_json::Value = match serde_json::from_str(&text) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if value.get("type").and_then(|v| v.as_str()) != Some("woo-lan") {
                    continue;
                }
                let port = value
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .and_then(|value| u16::try_from(value).ok())
                    .ok_or_else(|| "配对响应缺少端口".to_string())?;
                let password = value
                    .get("password")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "配对响应缺少局域网密码".to_string())?
                    .to_string();
                let folder = value
                    .get("folder")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Woo")
                    .to_string();
                return Ok(LanPairResult {
                    url: format!("http://{}:{}", src.ip(), port),
                    username: value
                        .get("username")
                        .and_then(|v| v.as_str())
                        .unwrap_or(USERNAME)
                        .to_string(),
                    password,
                    folder,
                });
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut =>
            {
                // 广播丢包时继续重试，直到 4 秒超时。
                for target in &targets {
                    let _ = socket.send_to(request.as_bytes(), target);
                }
            }
            Err(error) => return Err(format!("配对失败: {}", error)),
        }
    }
    Err("未找到电脑。请确认手机和 Mac 在同一 Wi-Fi，且 Mac 已启用局域网同步".to_string())
}

// ============================================================
// Server lifecycle
// ============================================================

struct RunningServer {
    stop: Arc<AtomicBool>,
    port: u16,
}

static SERVER: Lazy<Mutex<Option<RunningServer>>> = Lazy::new(|| Mutex::new(None));

fn running_port() -> Option<u16> {
    SERVER
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|server| server.port))
}

fn stop_server() {
    if let Ok(mut guard) = SERVER.lock() {
        if let Some(server) = guard.take() {
            server.stop.store(true, Ordering::Relaxed);
        }
    }
}

/// 启动一个只服务 `root` 的 WebDAV 服务。返回实际绑定的端口。
fn start_server_with_root(
    root: PathBuf,
    preferred_port: u16,
    token: &str,
    folder: &str,
    pair_code: Option<&str>,
) -> Result<u16, String> {
    if token.is_empty() {
        return Err("配对码为空".to_string());
    }
    fs::create_dir_all(&root).map_err(|e| format!("创建局域网同步目录失败: {}", e))?;
    let expected_auth = format!(
        "Basic {}",
        base64::engine::general_purpose::STANDARD.encode(format!("{}:{}", USERNAME, token))
    );
    let stop = Arc::new(AtomicBool::new(false));

    let mut last_error = String::new();
    for offset in 0..10u16 {
        let port = preferred_port.saturating_add(offset);
        if port == 0 {
            continue;
        }
        match Server::http(("0.0.0.0", port)) {
            Ok(server) => {
                let root = root.clone();
                let expected_auth = expected_auth.clone();
                let stop_flag = Arc::clone(&stop);
                std::thread::spawn(move || {
                    log::info!("[LAN] WebDAV server listening on 0.0.0.0:{}", port);
                    while !stop_flag.load(Ordering::Relaxed) {
                        match server.recv_timeout(Duration::from_millis(200)) {
                            Ok(Some(request)) => handle_request(request, &root, &expected_auth),
                            Ok(None) => {}
                            Err(error) => {
                                log::warn!("[LAN] receive failed: {}", error);
                                break;
                            }
                        }
                    }
                    log::info!("[LAN] WebDAV server stopped");
                });
                // UDP 配对发现：手机广播 6 位配对码，桌面端只在码匹配时返回连接信息。
                if let Some(code) = pair_code.filter(|code| code.len() == 6) {
                    match UdpSocket::bind(("0.0.0.0", PAIR_PORT)) {
                        Ok(socket) => {
                            let _ = socket.set_read_timeout(Some(Duration::from_millis(500)));
                            let stop_udp = Arc::clone(&stop);
                            let code = code.to_string();
                            let token = token.to_string();
                            let folder = folder.to_string();
                            std::thread::spawn(move || {
                                log::info!("[LAN] pairing discovery listening on UDP {}", PAIR_PORT);
                                let mut buf = [0u8; 256];
                                while !stop_udp.load(Ordering::Relaxed) {
                                    match socket.recv_from(&mut buf) {
                                        Ok((n, src)) => {
                                            let request = String::from_utf8_lossy(&buf[..n]);
                                            let matched = request
                                                .trim()
                                                .strip_prefix("WOO-PAIR ")
                                                .map(|value| value.trim() == code)
                                                .unwrap_or(false);
                                            if matched {
                                                let response = serde_json::json!({
                                                    "type": "woo-lan",
                                                    "port": port,
                                                    "username": USERNAME,
                                                    "password": token,
                                                    "folder": folder,
                                                })
                                                .to_string();
                                                let _ = socket.send_to(response.as_bytes(), src);
                                            } else {
                                                // 错误配对码慢速处理，降低局域网内暴力尝试。
                                                std::thread::sleep(Duration::from_millis(350));
                                            }
                                        }
                                        Err(_) => {}
                                    }
                                }
                                log::info!("[LAN] pairing discovery stopped");
                            });
                        }
                        Err(error) => log::warn!("[LAN] pairing discovery unavailable on UDP {}: {}", PAIR_PORT, error),
                    }
                }
                if let Ok(mut guard) = SERVER.lock() {
                    if let Some(previous) = guard.replace(RunningServer { stop, port }) {
                        previous.stop.store(true, Ordering::Relaxed);
                    }
                }
                return Ok(port);
            }
            Err(error) => {
                last_error = format!("端口 {} 不可用: {}", port, error);
            }
        }
    }
    Err(if last_error.is_empty() {
        "无法启动局域网同步服务".to_string()
    } else {
        last_error
    })
}

fn start_server(
    preferred_port: u16,
    token: &str,
    folder: &str,
    pair_code: Option<&str>,
) -> Result<u16, String> {
    start_server_with_root(lan_root(), preferred_port, token, folder, pair_code)
}

/// 应用启动时恢复上次启用的服务；账号不匹配时不启动，避免跨账号共享。
pub fn restore_on_startup() {
    let mut config = load_config();
    if !config.enabled || config.token.is_empty() {
        return;
    }
    if config.bound_user != db::current_user_key() {
        log::warn!("[LAN] 配置绑定的是另一个账号，跳过启动");
        return;
    }
    if ensure_pair_code(&mut config) {
        let _ = save_config(&config);
    }
    match start_server(config.port, &config.token, &config.folder, Some(&config.pair_code)) {
        Ok(port) => {
            if port != config.port {
                let mut updated = config;
                updated.port = port;
                let _ = save_config(&updated);
            }
        }
        Err(error) => log::warn!("[LAN] 启动失败: {}", error),
    }
}

pub fn is_enabled() -> bool {
    let config = load_config();
    config.enabled
        && !config.token.is_empty()
        && config.bound_user == db::current_user_key()
        && running_port().is_some()
}

pub fn get_config_view() -> LanSyncView {
    let config = load_config();
    let port = running_port().unwrap_or(config.port);
    let running = running_port().is_some();
    LanSyncView {
        enabled: config.enabled,
        active: is_enabled(),
        running,
        port,
        token: config.token.clone(),
        folder: config.folder.clone(),
        pair_code: config.pair_code.clone(),
        address: if running {
            lan_ip().map(|ip| format!("http://{}:{}", ip, port))
        } else {
            None
        },
    }
}

pub fn set_enabled(enabled: bool) -> Result<LanSyncView, String> {
    let mut config = load_config();
    if enabled {
        if config.token.is_empty() {
            config.token = new_token();
        }
        config.folder = webdav_sync::sanitize_folder(&config.folder);
        config.bound_user = db::current_user_key();
        ensure_pair_code(&mut config);
        // 先停掉旧服务，避免端口/配对码变化后仍在跑旧配置。
        stop_server();
        let port = start_server(config.port, &config.token, &config.folder, Some(&config.pair_code))?;
        config.enabled = true;
        config.port = port;
        if let Err(error) = save_config(&config) {
            stop_server();
            return Err(error);
        }
    } else {
        config.enabled = false;
        save_config(&config)?;
        stop_server();
    }
    Ok(get_config_view())
}

// ============================================================
// Sync
// ============================================================

pub async fn sync_now() -> Result<SyncResult, String> {
    let config = load_config();
    if !config.enabled {
        return Err("未启用局域网同步".to_string());
    }
    if config.bound_user != db::current_user_key() {
        return Err("局域网同步属于另一个账号，请重新启用".to_string());
    }
    let port = running_port().ok_or_else(|| "局域网同步服务未启动".to_string())?;
    let expected_user = db::current_user_key();
    let webdav = WebdavConfig {
        enabled: true,
        url: format!("http://127.0.0.1:{}", port),
        username: USERNAME.to_string(),
        password: config.token.clone(),
        folder: config.folder.clone(),
        bound_user: config.bound_user.clone(),
    };
    webdav_sync::sync_with_config(&webdav, &expected_user).await
}

// ============================================================
// Minimal WebDAV server
// ============================================================

fn handle_request(mut request: Request, root: &Path, expected_auth: &str) {
    let method = request.method().as_str().to_ascii_uppercase();

    if method == "OPTIONS" {
        respond_text(request, 200, "OK", &[("dav", "1,2"), ("allow", "OPTIONS,GET,HEAD,PUT,DELETE,PROPFIND,MKCOL")]);
        return;
    }

    if !authorized(&request, expected_auth) {
        respond_text(
            request,
            401,
            "unauthorized",
            &[("www-authenticate", "Basic realm=\"Woo LAN Sync\"")],
        );
        return;
    }

    let decoded = decoded_path(request.url());
    let Some(relative) = safe_relative(&decoded) else {
        respond_text(request, 400, "bad path", &[]);
        return;
    };
    let full = root.join(&relative);

    match method.as_str() {
        "PROPFIND" => {
            if full.exists() {
                let mut response = Response::from_string(PROPFIND_XML)
                    .with_status_code(StatusCode(207));
                add_header(&mut response, "content-type", "application/xml; charset=utf-8");
                add_header(&mut response, "dav", "1,2");
                let _ = request.respond(response);
            } else {
                respond_text(request, 404, "not found", &[]);
            }
        }
        "MKCOL" => {
            if full.exists() {
                respond_text(request, 405, "already exists", &[]);
            } else {
                match fs::create_dir_all(&full) {
                    Ok(()) => respond_text(request, 201, "created", &[]),
                    Err(error) => {
                        log::warn!("[LAN] MKCOL failed {:?}: {}", full, error);
                        respond_text(request, 500, "mkdir failed", &[]);
                    }
                }
            }
        }
        "PUT" => {
            let mut body = Vec::new();
            let read = request
                .as_reader()
                .take(MAX_PUT_BYTES)
                .read_to_end(&mut body);
            if let Err(error) = read {
                log::warn!("[LAN] read body failed: {}", error);
                respond_text(request, 400, "read failed", &[]);
                return;
            }
            if let Some(parent) = full.parent() {
                if let Err(error) = fs::create_dir_all(parent) {
                    log::warn!("[LAN] create parent failed {:?}: {}", parent, error);
                    respond_text(request, 500, "mkdir failed", &[]);
                    return;
                }
            }
            let temp = full.with_extension(format!(
                "tmp-{}",
                uuid::Uuid::new_v4().simple()
            ));
            if let Err(error) = fs::write(&temp, &body) {
                log::warn!("[LAN] write temp failed {:?}: {}", temp, error);
                respond_text(request, 500, "write failed", &[]);
                return;
            }
            #[cfg(windows)]
            {
                let _ = fs::remove_file(&full);
            }
            match fs::rename(&temp, &full) {
                Ok(()) => respond_text(request, 201, "created", &[]),
                Err(error) => {
                    let _ = fs::remove_file(&temp);
                    log::warn!("[LAN] rename failed {:?}: {}", full, error);
                    respond_text(request, 500, "write failed", &[]);
                }
            }
        }
        "GET" | "HEAD" => {
            if full.is_file() {
                let metadata = match fs::metadata(&full) {
                    Ok(metadata) => metadata,
                    Err(_) => {
                        respond_text(request, 404, "not found", &[]);
                        return;
                    }
                };
                let etag = file_etag(&metadata);
                if method == "HEAD" {
                    let mut response = Response::empty(StatusCode(200));
                    add_header(&mut response, "etag", &etag);
                    let _ = request.respond(response);
                } else {
                    match fs::File::open(&full) {
                        Ok(file) => {
                            let mut response = Response::from_file(file);
                            add_header(&mut response, "etag", &etag);
                            add_header(
                                &mut response,
                                "content-type",
                                if full.extension().and_then(|ext| ext.to_str()) == Some("json") {
                                    "application/json; charset=utf-8"
                                } else {
                                    "application/octet-stream"
                                },
                            );
                            let _ = request.respond(response);
                        }
                        Err(_) => respond_text(request, 404, "not found", &[]),
                    }
                }
            } else if full.is_dir() {
                respond_text(request, 405, "is a directory", &[]);
            } else {
                respond_text(request, 404, "not found", &[]);
            }
        }
        "DELETE" => {
            if full.is_file() {
                match fs::remove_file(&full) {
                    Ok(()) => respond_text(request, 204, "", &[]),
                    Err(error) => {
                        log::warn!("[LAN] delete failed {:?}: {}", full, error);
                        respond_text(request, 500, "delete failed", &[]);
                    }
                }
            } else if full.is_dir() {
                match fs::remove_dir(&full) {
                    Ok(()) => respond_text(request, 204, "", &[]),
                    Err(_) => respond_text(request, 409, "directory not empty", &[]),
                }
            } else {
                respond_text(request, 404, "not found", &[]);
            }
        }
        _ => respond_text(request, 405, "method not allowed", &[]),
    }
}

fn authorized(request: &Request, expected: &str) -> bool {
    request.headers().iter().any(|header| {
        header
            .field
            .as_str()
            .as_str()
            .eq_ignore_ascii_case("authorization")
            && header.value.as_str() == expected
    })
}

fn decoded_path(url: &str) -> String {
    let raw = url.split('?').next().unwrap_or("/");
    percent_decode_str(raw).decode_utf8_lossy().to_string()
}

fn safe_relative(path: &str) -> Option<PathBuf> {
    let mut relative = PathBuf::new();
    for segment in path.split('/') {
        if segment.is_empty() {
            continue;
        }
        if segment == "." || segment == ".." || segment.contains('\\') || segment.contains('\0') {
            return None;
        }
        relative.push(segment);
    }
    Some(relative)
}

fn file_etag(metadata: &fs::Metadata) -> String {
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("\"{:x}-{:x}\"", metadata.len(), modified)
}

fn add_header<W: std::io::Read>(response: &mut Response<W>, name: &str, value: &str) {
    if let Ok(header) = Header::from_bytes(name.as_bytes(), value.as_bytes()) {
        response.add_header(header);
    }
}

fn respond_text(
    request: Request,
    status: u16,
    text: &str,
    headers: &[(&str, &str)],
) {
    let mut response = Response::from_string(text).with_status_code(StatusCode(status));
    for (name, value) in headers {
        add_header(&mut response, name, value);
    }
    let _ = request.respond(response);
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn free_port() -> u16 {
        use std::net::TcpListener;
        TcpListener::bind(("127.0.0.1", 0))
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(45999)
    }

    #[tokio::test]
    async fn lan_server_roundtrip() {
        let root = std::env::temp_dir().join(format!(
            "woo-lan-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let token = "test-token";
        let port = free_port();
        let actual = start_server_with_root(root.clone(), port, token, "Woo", None).unwrap();
        let base = format!("http://127.0.0.1:{}", actual);
        let client = reqwest::Client::new();
        let propfind = reqwest::Method::from_bytes(b"PROPFIND").unwrap();
        let mkcol = reqwest::Method::from_bytes(b"MKCOL").unwrap();

        // 未认证 → 401
        let response = client
            .request(propfind.clone(), format!("{}/Woo", base))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::UNAUTHORIZED);

        // 建目录 → PUT → GET → HEAD
        let response = client
            .request(mkcol, format!("{}/Woo", base))
            .basic_auth(USERNAME, Some(token))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());

        let payload = r#"{"hello":"world"}"#;
        let response = client
            .put(format!("{}/Woo/test.json", base))
            .basic_auth(USERNAME, Some(token))
            .body(payload)
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());

        let response = client
            .get(format!("{}/Woo/test.json", base))
            .basic_auth(USERNAME, Some(token))
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), payload);

        let response = client
            .head(format!("{}/Woo/test.json", base))
            .basic_auth(USERNAME, Some(token))
            .send()
            .await
            .unwrap();
        assert!(response.headers().get("etag").is_some());

        let response = client
            .delete(format!("{}/Woo/test.json", base))
            .basic_auth(USERNAME, Some(token))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success());

        stop_server();
        let _ = fs::remove_dir_all(&root);
    }

    /// 局域网同步端到端（默认忽略，会切换全局数据库目录）：
    /// cargo test lan_sync_end_to_end --lib -- --ignored --nocapture
    #[tokio::test]
    #[ignore]
    async fn lan_sync_end_to_end() {
        let root = std::env::temp_dir().join(format!(
            "woo-lan-e2e-root-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let device_a = std::env::temp_dir().join(format!(
            "woo-lan-e2e-a-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let device_b = std::env::temp_dir().join(format!(
            "woo-lan-e2e-b-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&device_a).unwrap();
        std::fs::create_dir_all(&device_b).unwrap();
        let token = "e2e-token";
        let port = start_server_with_root(root.clone(), free_port(), token, "Woo", None).unwrap();
        let config = WebdavConfig {
            enabled: true,
            url: format!("http://127.0.0.1:{}", port),
            username: USERNAME.to_string(),
            password: token.to_string(),
            folder: "Woo".to_string(),
            bound_user: None,
        };

        db::set_data_dir(device_a.clone());
        db::close_db();
        db::set_current_user(None);
        db::with_db(|conn| {
            conn.execute(
                "INSERT INTO note_folder (id,parent_id,name,sort_order,create_time,update_time,deleted,is_locked)
                 VALUES ('f1',NULL,'局域网目录',0,'2026-11-16T10:00:00','2026-11-16T10:00:00',0,0)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO note_document (id,folder_id,title,content,branch_name,sort_order,create_time,update_time,deleted,is_locked)
                 VALUES ('d1','f1','局域网文稿','<p>来自电脑</p>',NULL,0,'2026-11-16T10:00:00','2026-11-16T10:00:00',0,0)",
                [],
            ).unwrap();
        });
        let pushed = webdav_sync::sync_with_config(&config, &None).await.unwrap();
        assert_eq!(pushed.pushed_count, 2);

        db::close_db();
        db::set_data_dir(device_b.clone());
        db::set_current_user(None);
        let pulled = webdav_sync::sync_with_config(&config, &None).await.unwrap();
        assert_eq!(pulled.pulled_count, 2);
        let content: String = db::with_db(|conn| {
            conn.query_row("SELECT content FROM note_document WHERE id = 'd1'", [], |row| row.get(0)).unwrap()
        });
        assert_eq!(content, "<p>来自电脑</p>");

        stop_server();
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(device_a);
        let _ = fs::remove_dir_all(device_b);
    }
}
