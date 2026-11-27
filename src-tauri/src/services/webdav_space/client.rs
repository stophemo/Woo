//! 最小 WebDAV 客户端：Basic Auth、HTTPS、禁止重定向、退避重试、
//! PROPFIND/MKCOL/GET/PUT/DELETE。

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use reqwest::{Method, StatusCode};

use super::account::normalize_endpoint_with_options;

/// 同一账号相邻请求的最小间隔，避免突发流量触发供应商限流。
const MIN_REQUEST_INTERVAL: Duration = Duration::from_millis(120);
/// 429 冷却上限：超过这个时间本轮直接结束，交给下一轮同步重试。
const MAX_COOLDOWN: Duration = Duration::from_secs(120);

struct Throttle {
    /// 同一账号的请求串行执行，避免并发打满供应商配额。
    gate: tokio::sync::Mutex<()>,
    last: StdMutex<Option<Instant>>,
    cooldown_until: StdMutex<Option<Instant>>,
}

impl Throttle {
    fn new() -> Self {
        Self {
            gate: tokio::sync::Mutex::new(()),
            last: StdMutex::new(None),
            cooldown_until: StdMutex::new(None),
        }
    }

    async fn wait_turn(&self) {
        loop {
            let now = Instant::now();
            let cooldown = *self.cooldown_until.lock().unwrap();
            if let Some(until) = cooldown {
                if now < until {
                    tokio::time::sleep(until - now).await;
                    continue;
                }
            }
            let last = *self.last.lock().unwrap();
            if let Some(last) = last {
                let elapsed = now.saturating_duration_since(last);
                if elapsed < MIN_REQUEST_INTERVAL {
                    tokio::time::sleep(MIN_REQUEST_INTERVAL - elapsed).await;
                    continue;
                }
            }
            *self.last.lock().unwrap() = Some(Instant::now());
            return;
        }
    }

    fn cooldown(&self, duration: Duration) {
        let duration = duration.min(MAX_COOLDOWN);
        let until = Instant::now() + duration;
        let mut guard = self.cooldown_until.lock().unwrap();
        if guard.map(|current| current < until).unwrap_or(true) {
            *guard = Some(until);
        }
    }
}

static THROTTLES: Lazy<StdMutex<HashMap<String, Arc<Throttle>>>> =
    Lazy::new(|| StdMutex::new(HashMap::new()));

fn throttle_for(key: &str) -> Arc<Throttle> {
    let mut guard = THROTTLES.lock().unwrap();
    guard
        .entry(key.to_string())
        .or_insert_with(|| Arc::new(Throttle::new()))
        .clone()
}

fn retry_after(response: &reqwest::Response) -> Option<Duration> {
    let value = response.headers().get("retry-after")?.to_str().ok()?;
    value
        .trim()
        .parse::<u64>()
        .ok()
        .map(Duration::from_secs)
        .map(|duration| duration.min(MAX_COOLDOWN))
}

const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop><d:resourcetype/><d:getetag/><d:getlastmodified/></d:prop>
</d:propfind>"#;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteError {
    Conflict,
    NotFound,
    Auth,
    Redirect,
    Other(String),
}

impl fmt::Display for RemoteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(formatter, "远端文件已存在"),
            Self::NotFound => write!(formatter, "远端路径不存在"),
            Self::Auth => write!(
                formatter,
                "WebDAV 认证失败：请确认用户名和应用密码；坚果云等供应商需要使用应用密码"
            ),
            Self::Redirect => write!(
                formatter,
                "WebDAV 地址发生重定向，请直接填写最终地址（不接受重定向）"
            ),
            Self::Other(message) => write!(formatter, "{message}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
}

#[async_trait]
pub trait RemoteStore: Send + Sync {
    async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>, RemoteError>;
    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>, RemoteError>;
    async fn put(
        &self,
        path: &str,
        bytes: &[u8],
        if_none_match: bool,
    ) -> Result<(), RemoteError>;
    async fn delete(&self, path: &str) -> Result<(), RemoteError>;
    async fn mkcol(&self, path: &str) -> Result<(), RemoteError>;
}

pub async fn ensure_collection(
    store: &dyn RemoteStore,
    path: &str,
) -> Result<(), RemoteError> {
    let mut current = String::new();
    for segment in path.split('/').filter(|segment| !segment.is_empty()) {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(segment);
        store.mkcol(&current).await?;
    }
    Ok(())
}

pub struct WebdavClient {
    base: reqwest::Url,
    username: String,
    password: String,
    http: reqwest::Client,
    retry_base: Duration,
}

impl WebdavClient {
    pub fn new(
        endpoint: &str,
        username: &str,
        password: &str,
        allow_loopback_http: bool,
        retry_base: Duration,
    ) -> Result<Self, String> {
        let endpoint = normalize_endpoint_with_options(endpoint, allow_loopback_http)?;
        let base = reqwest::Url::parse(&endpoint)
            .map_err(|_| "WebDAV 地址格式不正确".to_string())?;
        let http = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Woo/", env!("CARGO_PKG_VERSION"), " WebDAV"))
            .connect_timeout(Duration::from_secs(15))
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| format!("初始化 WebDAV 客户端失败：{error}"))?;
        Ok(Self {
            base,
            username: username.to_string(),
            password: password.to_string(),
            http,
            retry_base,
        })
    }

    fn url_for(&self, path: &str, collection: bool) -> Result<reqwest::Url, RemoteError> {
        let mut url = self.base.clone();
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| RemoteError::Other("WebDAV 地址不能作为目录".to_string()))?;
            for segment in path.split('/') {
                if segment.is_empty() || segment == "." {
                    continue;
                }
                if segment == ".." {
                    return Err(RemoteError::Other("WebDAV 路径不能包含 ..".to_string()));
                }
                segments.push(segment);
            }
            // 集合路径带尾斜杠，避免部分服务器用 301 跳转补斜杠；
            // 客户端明确禁止重定向，因此必须直接请求最终地址。
            if collection {
                segments.push("");
            }
        }
        Ok(url)
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        content_type: Option<&str>,
        depth: Option<&str>,
        if_none_match: bool,
        collection: bool,
    ) -> Result<(StatusCode, Vec<u8>, String), RemoteError> {
        let url = self.url_for(path, collection)?;
        // 同一账号串行请求：WebDAV 供应商通常按并发/请求数限流，
        // 后台同步、设置页和设备列表同时请求很容易触发 429。
        let throttle = throttle_for(&format!("{}|{}", self.base, self.username));
        let _gate = throttle.gate.lock().await;
        throttle.wait_turn().await;
        let mut last_error: Option<RemoteError> = None;
        for attempt in 0..4u32 {
            if attempt > 0 {
                let delay = self.retry_base * (1u32 << (attempt - 1));
                if !delay.is_zero() {
                    tokio::time::sleep(delay).await;
                }
            }
            let mut request = self.http.request(method.clone(), url.clone());
            if !self.username.is_empty() {
                request = request.basic_auth(&self.username, Some(&self.password));
            }
            if let Some(value) = body.clone() {
                request = request.body(value);
            }
            if let Some(value) = content_type {
                request = request.header("Content-Type", value);
            }
            if let Some(value) = depth {
                request = request.header("Depth", value);
            }
            if if_none_match {
                request = request.header("If-None-Match", "*");
            }
            match request.send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_redirection() {
                        return Err(RemoteError::Redirect);
                    }
                    if status == StatusCode::TOO_MANY_REQUESTS {
                        // 不在本次同步里硬等：记录冷却时间，让本轮失败并在下一轮自动重试。
                        let wait = retry_after(&response).unwrap_or(Duration::from_secs(30));
                        throttle.cooldown(wait);
                        return Err(RemoteError::Other(format!(
                            "云盘请求过于频繁，已暂停本轮同步，约 {} 秒后自动重试",
                            wait.as_secs().max(1)
                        )));
                    }
                    if status.is_server_error() {
                        throttle.cooldown(Duration::from_secs(2));
                        last_error = Some(RemoteError::Other(format!(
                            "WebDAV 暂时不可用（HTTP {}）",
                            status.as_u16()
                        )));
                        continue;
                    }
                    let location = response
                        .headers()
                        .get("location")
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or_default()
                        .to_string();
                    let body = response
                        .bytes()
                        .await
                        .map_err(|error| RemoteError::Other(format!("读取 WebDAV 响应失败：{error}")))?
                        .to_vec();
                    return Ok((status, body, location));
                }
                Err(error) => {
                    throttle.cooldown(Duration::from_secs(3));
                    last_error = Some(RemoteError::Other(format!("网络错误：{error}")));
                }
            }
        }
        Err(last_error.unwrap_or_else(|| RemoteError::Other("WebDAV 请求失败".to_string())))
    }

    fn map_status(action: &str, status: StatusCode) -> RemoteError {
        match status {
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => RemoteError::Auth,
            StatusCode::NOT_FOUND => RemoteError::NotFound,
            StatusCode::CONFLICT => RemoteError::Conflict,
            StatusCode::PRECONDITION_FAILED => RemoteError::Conflict,
            StatusCode::INSUFFICIENT_STORAGE => RemoteError::Other(
                "云盘空间不足，请清理空间或更换 WebDAV 账号".to_string(),
            ),
            _ => RemoteError::Other(format!(
                "WebDAV 请求失败（{}）：HTTP {}",
                action,
                status.as_u16()
            )),
        }
    }

    fn parse_propfind(&self, requested_path: &str, xml: &str) -> Result<Vec<RemoteEntry>, RemoteError> {
        let document = roxmltree::Document::parse(xml)
            .map_err(|error| RemoteError::Other(format!("解析 WebDAV 目录失败：{error}")))?;
        let requested_name = requested_path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .next_back()
            .unwrap_or_default();
        let mut entries = Vec::new();
        for response in document
            .descendants()
            .filter(|node| node.tag_name().name() == "response")
        {
            let Some(href) = response
                .descendants()
                .find(|node| node.tag_name().name() == "href")
                .and_then(|node| node.text())
            else {
                continue;
            };
            let href = href.trim();
            if href.is_empty() {
                continue;
            }
            let is_dir = href.ends_with('/')
                || response
                    .descendants()
                    .any(|node| node.tag_name().name() == "collection");
            let decoded = percent_encoding::percent_decode_str(href)
                .decode_utf8_lossy()
                .to_string();
            let name = decoded
                .trim_end_matches('/')
                .rsplit('/')
                .next()
                .unwrap_or_default()
                .to_string();
            if name.is_empty() || name == requested_name {
                continue;
            }
            entries.push(RemoteEntry { name, is_dir });
        }
        Ok(entries)
    }
}

#[async_trait]
impl RemoteStore for WebdavClient {
    async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>, RemoteError> {
        let (status, body, _) = self
            .send(
                Method::from_bytes(b"PROPFIND").expect("PROPFIND"),
                path,
                Some(PROPFIND_BODY.as_bytes().to_vec()),
                Some("application/xml; charset=utf-8"),
                Some("1"),
                false,
                true,
            )
            .await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        if !status.is_success() {
            return Err(Self::map_status("列出目录", status));
        }
        let text = String::from_utf8_lossy(&body);
        self.parse_propfind(path, &text)
    }

    async fn get(&self, path: &str) -> Result<Option<Vec<u8>>, RemoteError> {
        let (status, body, _) = self
            .send(Method::GET, path, None, None, None, false, false)
            .await?;
        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            return Err(Self::map_status("读取文件", status));
        }
        Ok(Some(body))
    }

    async fn put(
        &self,
        path: &str,
        bytes: &[u8],
        if_none_match: bool,
    ) -> Result<(), RemoteError> {
        let (status, _, _) = self
            .send(
                Method::PUT,
                path,
                Some(bytes.to_vec()),
                Some("application/octet-stream"),
                None,
                if_none_match,
                false,
            )
            .await?;
        if status.is_success() {
            return Ok(());
        }
        Err(Self::map_status("写入文件", status))
    }

    async fn delete(&self, path: &str) -> Result<(), RemoteError> {
        let (status, _, _) = self
            .send(Method::DELETE, path, None, None, None, false, false)
            .await?;
        if status.is_success() || status == StatusCode::NOT_FOUND {
            return Ok(());
        }
        Err(Self::map_status("删除文件", status))
    }

    async fn mkcol(&self, path: &str) -> Result<(), RemoteError> {
        let (status, _, _) = self
            .send(
                Method::from_bytes(b"MKCOL").expect("MKCOL"),
                path,
                None,
                None,
                None,
                false,
                true,
            )
            .await?;
        if status.is_success() || status == StatusCode::METHOD_NOT_ALLOWED {
            return Ok(());
        }
        Err(Self::map_status("创建目录", status))
    }
}

impl WebdavClient {
    /// 测试连接：确保 v1 可创建，再写入/删除一个探针文件。
    pub async fn test_connection(&self) -> Result<String, String> {
        ensure_collection(self, "v1").await.map_err(|error| error.to_string())?;
        let path = format!("v1/woo-write-test-{}.json", uuid::Uuid::new_v4());
        self.put(&path, br#"{"ok":true}"#, false)
            .await
            .map_err(|error| error.to_string())?;
        let _ = self.delete(&path).await;
        Ok("连接成功，可读写".to_string())
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    pub struct MemoryRemote {
        pub files: Mutex<HashMap<String, Vec<u8>>>,
        pub dirs: Mutex<HashSet<String>>,
        /// 模拟写入失败次数；用于验证重试/失败路径。
        pub fail_puts: Mutex<usize>,
    }

    impl MemoryRemote {
        pub fn new() -> Self {
            let remote = Self::default();
            remote.dirs.lock().unwrap().insert(String::new());
            remote
        }

        pub fn insert_file(&self, path: &str, bytes: &[u8]) {
            let mut dirs = self.dirs.lock().unwrap();
            self.files
                .lock()
                .unwrap()
                .insert(path.trim_matches('/').to_string(), bytes.to_vec());
            // 确保父目录存在。
            if let Some((parent, _)) = path.trim_matches('/').rsplit_once('/') {
                let mut prefix = String::new();
                for segment in parent.split('/') {
                    if !prefix.is_empty() {
                        prefix.push('/');
                    }
                    prefix.push_str(segment);
                    dirs.insert(prefix.clone());
                }
            }
        }
    }

    #[async_trait]
    impl RemoteStore for MemoryRemote {
        async fn list(&self, path: &str) -> Result<Vec<RemoteEntry>, RemoteError> {
            let path = path.trim_matches('/');
            let prefix = if path.is_empty() {
                String::new()
            } else {
                format!("{path}/")
            };
            let mut names = HashMap::<String, bool>::new();
            for file in self.files.lock().unwrap().keys() {
                if let Some(rest) = file.strip_prefix(&prefix) {
                    if rest.contains('/') {
                        let dir = rest.split('/').next().unwrap();
                        names.insert(dir.to_string(), true);
                    } else {
                        names.insert(rest.to_string(), false);
                    }
                }
            }
            for dir in self.dirs.lock().unwrap().iter() {
                if let Some(rest) = dir.strip_prefix(&prefix) {
                    if !rest.is_empty() && !rest.contains('/') {
                        names.insert(rest.to_string(), true);
                    }
                }
            }
            Ok(names
                .into_iter()
                .map(|(name, is_dir)| RemoteEntry { name, is_dir })
                .collect())
        }

        async fn get(&self, path: &str) -> Result<Option<Vec<u8>>, RemoteError> {
            Ok(self
                .files
                .lock()
                .unwrap()
                .get(path.trim_matches('/'))
                .cloned())
        }

        async fn put(
            &self,
            path: &str,
            bytes: &[u8],
            if_none_match: bool,
        ) -> Result<(), RemoteError> {
            let path = path.trim_matches('/').to_string();
            let mut failures = self.fail_puts.lock().unwrap();
            if *failures > 0 {
                *failures -= 1;
                return Err(RemoteError::Other("模拟写入失败".to_string()));
            }
            drop(failures);
            if if_none_match && self.files.lock().unwrap().contains_key(&path) {
                return Err(RemoteError::Conflict);
            }
            self.insert_file(&path, bytes);
            Ok(())
        }

        async fn delete(&self, path: &str) -> Result<(), RemoteError> {
            self.files
                .lock()
                .unwrap()
                .remove(path.trim_matches('/'));
            Ok(())
        }

        async fn mkcol(&self, path: &str) -> Result<(), RemoteError> {
            self.dirs
                .lock()
                .unwrap()
                .insert(path.trim_matches('/').to_string());
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn preserves_provider_base_paths_when_appending_sync_paths() {
        let cases = [
            ("https://dav.jianguoyun.com/dav/", "/dav/v1/vault-a/ops/"),
            (
                "https://cloud.example.com/remote.php/dav/files/alice/",
                "/remote.php/dav/files/alice/v1/vault-a/ops/",
            ),
            ("https://nas.example.com:5006/woo", "/woo/v1/vault-a/ops/"),
            ("https://dav.example.com", "/v1/vault-a/ops/"),
        ];
        for (endpoint, expected) in cases {
            let client = WebdavClient::new(
                endpoint,
                "user",
                "password",
                false,
                Duration::ZERO,
            )
            .unwrap();
            let url = client.url_for("v1/vault-a/ops", true).unwrap();
            assert_eq!(url.path(), expected, "endpoint={endpoint}");
        }
    }

    #[test]
    fn parses_propfind_hrefs_and_collections() {
        let client = WebdavClient::new(
            "https://dav.example.com/remote.php/dav/files/user",
            "user",
            "password",
            false,
            Duration::ZERO,
        )
        .unwrap();
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:href>/remote.php/dav/files/user/v1/vault/ops/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/user/v1/vault/ops/0a/</d:href>
    <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat>
  </d:response>
  <d:response>
    <d:href>/remote.php/dav/files/user/v1/vault/ops/0a/abc.json</d:href>
    <d:propstat><d:prop><d:resourcetype/></d:prop></d:propstat>
  </d:response>
</d:multistatus>"#;
        let entries = client.parse_propfind("v1/vault/ops", xml).unwrap();
        assert!(entries
            .iter()
            .any(|entry| entry.name == "0a" && entry.is_dir));
        assert!(entries
            .iter()
            .any(|entry| entry.name == "abc.json" && !entry.is_dir));
    }

    #[tokio::test]
    async fn rejects_redirects_without_following_them() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let handle = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            request
                .respond(
                    tiny_http::Response::empty(302).with_header(
                        tiny_http::Header::from_bytes(&b"Location"[..], &b"https://evil.example/"[..])
                            .unwrap(),
                    ),
                )
                .unwrap();
        });
        let client = WebdavClient::new(
            &format!("http://{address}"),
            "user",
            "password",
            true,
            Duration::ZERO,
        )
        .unwrap();
        let error = client.list("v1").await.unwrap_err();
        assert_eq!(error, RemoteError::Redirect);
        handle.join().unwrap();
    }

    #[tokio::test]
    async fn retries_server_errors() {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let address = server.server_addr().to_ip().unwrap();
        let handle = std::thread::spawn(move || {
            for status in [500u16, 207] {
                let request = server.recv().unwrap();
                let body = if status == 207 {
                    r#"<d:multistatus xmlns:d="DAV:"><d:response><d:href>/v1/shard/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response></d:multistatus>"#
                } else {
                    ""
                };
                request
                    .respond(
                        tiny_http::Response::from_string(body).with_status_code(status),
                    )
                    .unwrap();
            }
        });
        let client = WebdavClient::new(
            &format!("http://{address}"),
            "user",
            "password",
            true,
            Duration::ZERO,
        )
        .unwrap();
        let entries = client.list("v1").await.unwrap();
        assert!(entries.iter().any(|entry| entry.is_dir));
        handle.join().unwrap();
    }
}
