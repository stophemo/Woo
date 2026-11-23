use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use woo_sync_core::wire::EncryptedEnvelope;
use woo_sync_core::{SyncData, SyncRequest};

pub const INVITE_HEADER: &str = "X-Woo-Sync-Invite-Code";

#[derive(Debug, Clone)]
pub struct WorkerError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Value,
}

impl WorkerError {
    pub fn is_cursor_ahead(&self) -> bool {
        self.code == "CURSOR_AHEAD"
    }

    pub fn is_cursor_not_found(&self) -> bool {
        self.code == "CURSOR_NOT_FOUND"
    }

    pub fn is_capacity(&self) -> bool {
        self.code == "VAULT_CAPACITY_REACHED"
    }

    pub fn is_unauthorized(&self) -> bool {
        self.status == 401 || self.status == 403
    }

    pub fn is_not_found(&self) -> bool {
        self.status == 404
    }
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.code.is_empty() {
            formatter.write_str(&self.message)
        } else {
            write!(formatter, "{}：{}", self.code, self.message)
        }
    }
}

impl std::error::Error for WorkerError {}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Envelope<T> {
    ok: bool,
    data: Option<T>,
    error: Option<ApiErrorBody>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiErrorBody {
    code: String,
    message: String,
    #[serde(default)]
    details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRegistration {
    pub name: String,
    pub platform: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub public_key: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedDevice {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub token: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedVaultData {
    pub vault_id: String,
    pub device: CreatedDevice,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingCodeData {
    pub pairing_id: String,
    pub pairing_secret: String,
    pub initiator_public_key: String,
    pub expires_at: i64,
    pub server_time: i64,
    pub short_code: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPairingData {
    pub pairing_id: String,
    pub pairing_secret: String,
    pub initiator_public_key: String,
    pub vault_id: String,
    pub expires_at: i64,
    pub server_time: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimInfo {
    pub device_id: String,
    pub name: String,
    pub platform: String,
    pub public_key: String,
    pub claimed_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingStatusData {
    pub pairing_id: String,
    pub status: String,
    pub expires_at: i64,
    pub claim: Option<ClaimInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimData {
    pub pairing_id: String,
    pub status: String,
    pub device_id: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingResultData {
    pub pairing_id: String,
    pub status: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub vault_id: Option<String>,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub initiator_public_key: Option<String>,
    #[serde(default)]
    pub vault_key_envelope: Option<EncryptedEnvelope>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub created_at: i64,
    pub last_seen_at: Option<i64>,
    pub revoked_at: Option<i64>,
    pub is_current: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceListData {
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationData {
    pub operation: woo_sync_core::wire::SyncPulledOperation,
}

pub struct WorkerClient {
    endpoint: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl WorkerClient {
    pub fn new(endpoint: &str, token: Option<&str>) -> Result<Self, String> {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .user_agent("Woo-Notes-Sync/1")
            .build()
            .map_err(|error| format!("初始化同步 HTTP 客户端失败：{error}"))?;
        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_string(),
            token: token.map(ToOwned::to_owned),
            http,
        })
    }

    pub fn with_token(endpoint: &str, token: &str) -> Result<Self, String> {
        Self::new(endpoint, Some(token))
    }

    pub async fn health(&self) -> Result<String, WorkerError> {
        #[derive(Deserialize)]
        struct HealthData {
            service: String,
            version: String,
        }
        let data: HealthData = self.send(reqwest::Method::GET, "/health", None::<&()>, false).await?;
        Ok(format!("{} {}", data.service, data.version))
    }

    pub async fn create_vault(
        &self,
        invite_code: &str,
        device: &DeviceRegistration,
    ) -> Result<CreatedVaultData, WorkerError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            device: &'a DeviceRegistration,
        }
        self.send_with_invite(
            reqwest::Method::POST,
            "/v1/vaults",
            &Body { device },
            invite_code,
        )
        .await
    }

    pub async fn create_pairing_code(
        &self,
        public_key: &str,
    ) -> Result<PairingCodeData, WorkerError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            public_key: &'a str,
        }
        self.send(
            reqwest::Method::POST,
            "/v1/pairings/code",
            Some(&Body { public_key }),
            true,
        )
        .await
    }

    pub async fn resolve_pairing_code(
        &self,
        code: &str,
    ) -> Result<ResolvedPairingData, WorkerError> {
        #[derive(Serialize)]
        struct Body<'a> {
            code: &'a str,
        }
        self.send(
            reqwest::Method::POST,
            "/v1/pairings/resolve",
            Some(&Body { code }),
            false,
        )
        .await
    }

    pub async fn claim_pairing(
        &self,
        pairing_id: &str,
        pairing_secret: &str,
        device_token: &str,
        device: &DeviceRegistration,
    ) -> Result<ClaimData, WorkerError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            pairing_secret: &'a str,
            device_token: &'a str,
            device: &'a DeviceRegistration,
        }
        self.send(
            reqwest::Method::POST,
            &format!("/v1/pairings/{pairing_id}/claim"),
            Some(&Body {
                pairing_secret,
                device_token,
                device,
            }),
            false,
        )
        .await
    }

    pub async fn pairing_status(
        &self,
        pairing_id: &str,
    ) -> Result<PairingStatusData, WorkerError> {
        self.send::<PairingStatusData, ()>(
            reqwest::Method::GET,
            &format!("/v1/pairings/{pairing_id}"),
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn confirm_pairing(
        &self,
        pairing_id: &str,
        vault_key_envelope: &EncryptedEnvelope,
    ) -> Result<ClaimData, WorkerError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            vault_key_envelope: &'a EncryptedEnvelope,
        }
        self.send(
            reqwest::Method::POST,
            &format!("/v1/pairings/{pairing_id}/confirm"),
            Some(&Body {
                vault_key_envelope,
            }),
            true,
        )
        .await
    }

    pub async fn pairing_result(
        &self,
        pairing_id: &str,
        pairing_secret: &str,
        device_token: &str,
    ) -> Result<PairingResultData, WorkerError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Body<'a> {
            pairing_secret: &'a str,
            device_token: &'a str,
        }
        self.send(
            reqwest::Method::POST,
            &format!("/v1/pairings/{pairing_id}/result"),
            Some(&Body {
                pairing_secret,
                device_token,
            }),
            false,
        )
        .await
    }

    pub async fn sync(&self, request: &SyncRequest) -> Result<SyncData, WorkerError> {
        let data: SyncData = self
            .send(reqwest::Method::POST, "/v1/sync", Some(request), true)
            .await?;
        data.validate().map_err(|message| WorkerError {
            status: 200,
            code: "INVALID_RESPONSE".to_string(),
            message,
            details: Value::Null,
        })?;
        Ok(data)
    }

    pub async fn list_devices(&self) -> Result<DeviceListData, WorkerError> {
        self.send::<DeviceListData, ()>(
            reqwest::Method::GET,
            "/v1/devices",
            None::<&()>,
            true,
        )
        .await
    }

    pub async fn revoke_device(&self, device_id: &str) -> Result<(), WorkerError> {
        #[derive(Deserialize)]
        struct RevokeData {
            #[allow(dead_code)]
            revoked: Option<String>,
        }
        let _: RevokeData = self
            .send(
                reqwest::Method::POST,
                &format!("/v1/devices/{device_id}/revoke"),
                Some(&serde_json::json!({})),
                true,
            )
            .await?;
        Ok(())
    }

    pub async fn get_operation(&self, op_id: &str) -> Result<OperationData, WorkerError> {
        self.send::<OperationData, ()>(
            reqwest::Method::GET,
            &format!("/v1/ops/{op_id}"),
            None::<&()>,
            true,
        )
        .await
    }

    async fn send_with_invite<T, B>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: &B,
        invite_code: &str,
    ) -> Result<T, WorkerError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let request = self
            .http
            .request(method, self.url(path))
            .header(INVITE_HEADER, invite_code);
        self.execute(request.json(body), false).await
    }

    async fn send<T, B>(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&B>,
        authenticated: bool,
    ) -> Result<T, WorkerError>
    where
        T: DeserializeOwned,
        B: Serialize + ?Sized,
    {
        let mut request = self.http.request(method, self.url(path));
        if authenticated {
            let token = self.token.as_deref().ok_or_else(|| WorkerError {
                status: 401,
                code: "NOT_AUTHENTICATED".to_string(),
                message: "尚未配置同步空间".to_string(),
                details: Value::Null,
            })?;
            request = request.bearer_auth(token);
        }
        let request = match body {
            Some(value) => request.json(value),
            None => request,
        };
        self.execute(request, authenticated).await
    }

    async fn execute<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
        authenticated: bool,
    ) -> Result<T, WorkerError> {
        let response = request.send().await.map_err(|error| WorkerError {
            status: 0,
            code: "TRANSPORT".to_string(),
            message: format!("同步服务暂时不可达：{error}"),
            details: Value::Null,
        })?;
        let status = response.status().as_u16();
        let text = response.text().await.map_err(|error| WorkerError {
            status,
            code: "TRANSPORT".to_string(),
            message: format!("读取同步服务响应失败：{error}"),
            details: Value::Null,
        })?;
        let envelope: Envelope<T> = serde_json::from_str(&text).map_err(|error| {
            WorkerError {
                status,
                code: "INVALID_RESPONSE".to_string(),
                message: format!(
                    "同步服务返回了无法识别的内容（HTTP {status}）：{error}"
                ),
                details: Value::String(text.chars().take(200).collect()),
            }
        })?;
        if !envelope.ok || !(200..300).contains(&status) {
            let error = envelope.error.unwrap_or(ApiErrorBody {
                code: if authenticated && status == 401 {
                    "UNAUTHORIZED".to_string()
                } else {
                    "SYNC_ERROR".to_string()
                },
                message: format!("同步服务返回 HTTP {status}"),
                details: Value::Null,
            });
            return Err(WorkerError {
                status,
                code: error.code,
                message: error.message,
                details: error.details,
            });
        }
        envelope.data.ok_or_else(|| WorkerError {
            status,
            code: "INVALID_RESPONSE".to_string(),
            message: "同步服务响应缺少 data".to_string(),
            details: Value::Null,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.endpoint, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use woo_sync_core::crypto::base64url_encode;
    use woo_sync_core::wire::{SyncPushOperation, SyncPushSummary};

    #[test]
    fn invite_header_is_woo_specific() {
        assert_eq!(INVITE_HEADER, "X-Woo-Sync-Invite-Code");
    }

    /// 需要先在本机运行 `sync-server: npx wrangler dev --local --port 8787`，
    /// 并使用 `.dev.vars` 中的邀请码。默认跳过，避免 CI 依赖外部服务。
    #[tokio::test]
    #[ignore = "需要本地 wrangler dev"]
    async fn local_worker_round_trip() {
        let endpoint = "http://127.0.0.1:8787";
        let invite = "local-dev-invite-code-123456";
        let client = WorkerClient::new(endpoint, None).unwrap();
        let created = client
            .create_vault(
                invite,
                &DeviceRegistration {
                    name: "Rust 集成测试".to_string(),
                    platform: "macos".to_string(),
                    public_key: None,
                },
            )
            .await
            .unwrap();
        assert!(!created.device.token.is_empty());
        let token = created.device.token.clone();

        let initiator = WorkerClient::new(endpoint, Some(&token)).unwrap();
        let initiator_public = base64url_encode(
            &woo_sync_core::crypto::random_bytes::<32>().unwrap(),
        );
        let pairing = initiator
            .create_pairing_code(&initiator_public)
            .await
            .unwrap();
        assert_eq!(pairing.short_code.len(), 6);
        let resolved = WorkerClient::new(endpoint, None)
            .unwrap()
            .resolve_pairing_code(&pairing.short_code)
            .await
            .unwrap();
        assert_eq!(resolved.vault_id, created.vault_id);
        let joiner_token = base64url_encode(
            &woo_sync_core::crypto::random_bytes::<32>().unwrap(),
        );
        let joiner_public = base64url_encode(
            &woo_sync_core::crypto::random_bytes::<32>().unwrap(),
        );
        let claimed = WorkerClient::new(endpoint, None)
            .unwrap()
            .claim_pairing(
                &pairing.pairing_id,
                &resolved.pairing_secret,
                &joiner_token,
                &DeviceRegistration {
                    name: "Rust 测试手机".to_string(),
                    platform: "android".to_string(),
                    public_key: Some(joiner_public),
                },
            )
            .await
            .unwrap();
        assert_eq!(claimed.status, "claimed");
        let status = initiator
            .pairing_status(&pairing.pairing_id)
            .await
            .unwrap();
        assert_eq!(status.status, "claimed");
        assert_eq!(status.claim.unwrap().name, "Rust 测试手机");
        let envelope = EncryptedEnvelope {
            ciphertext: base64url_encode(&[6u8; 48]),
            nonce: base64url_encode(&[7u8; 12]),
        };
        let confirmed = initiator
            .confirm_pairing(&pairing.pairing_id, &envelope)
            .await
            .unwrap();
        assert_eq!(confirmed.status, "confirmed");
        let result = WorkerClient::new(endpoint, None)
            .unwrap()
            .pairing_result(
                &pairing.pairing_id,
                &resolved.pairing_secret,
                &joiner_token,
            )
            .await
            .unwrap();
        assert_eq!(result.status, "confirmed");
        assert!(result.vault_key_envelope.is_some());

        let operation = SyncPushOperation {
            op_id: format!("op:{}", uuid::Uuid::new_v4()),
            entity_id: "doc:integration".to_string(),
            kind: woo_sync_core::OperationKind::Upsert,
            lamport: 1,
            ciphertext: base64url_encode(&[8u8; 32]),
            nonce: base64url_encode(&[9u8; 12]),
        };
        let data = initiator
            .sync(&SyncRequest {
                cursor: 0,
                ack: 0,
                pull_limit: 100,
                push: vec![operation.clone()],
            })
            .await
            .unwrap();
        assert_eq!(data.push.inserted, 1);
        assert_eq!(data.push.duplicates, 0);
        let cursor = data.cursor;
        let duplicate = WorkerClient::new(endpoint, Some(&joiner_token))
            .unwrap()
            .sync(&SyncRequest {
                cursor,
                ack: cursor,
                pull_limit: 100,
                push: vec![operation.clone()],
            })
            .await
            .unwrap();
        assert_eq!(duplicate.push.inserted, 0);
        assert_eq!(duplicate.push.duplicates, 1);
        let _ = SyncPushSummary {
            received: 1,
            inserted: 0,
            duplicates: 1,
        };
        let fetched = initiator.get_operation(&operation.op_id).await.unwrap();
        assert_eq!(fetched.operation.op_id, operation.op_id);
    }
}
