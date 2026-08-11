use crate::commands::CommandResult;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::AppHandle;

#[cfg(target_os = "android")]
use std::path::{Path, PathBuf};
#[cfg(target_os = "android")]
use tauri::{Emitter, Manager};
#[cfg(target_os = "android")]
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const UPDATE_MANIFEST_URL: &str =
    "https://github.com/stophemo/Woo/releases/latest/download/latest.json";
const ANDROID_RELEASE_BASE_URL: &str = "https://github.com/stophemo/Woo/releases/download";
#[cfg(target_os = "android")]
const MOBILE_UPDATE_PROGRESS_EVENT: &str = "mobile-update-download-progress";
#[cfg(target_os = "android")]
const MIN_ANDROID_APK_BYTES: u64 = 1024 * 1024;
#[cfg(target_os = "android")]
const MAX_ANDROID_APK_BYTES: u64 = 250 * 1024 * 1024;

#[cfg(target_os = "android")]
static MOBILE_UPDATE_DOWNLOAD_LOCK: once_cell::sync::Lazy<tokio::sync::Mutex<()>> =
    once_cell::sync::Lazy::new(|| tokio::sync::Mutex::new(()));

#[derive(Debug, Deserialize)]
struct UpdateManifest {
    version: String,
    notes: Option<String>,
    #[serde(rename = "pub_date")]
    pub_date: Option<String>,
    android: Option<AndroidDownload>,
}

#[derive(Debug, Deserialize)]
struct AndroidDownload {
    url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileUpdateInfo {
    current_version: String,
    version: String,
    notes: Option<String>,
    pub_date: Option<String>,
    download_url: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileUpdateDownload {
    apk_path: String,
    cached: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileInstallResult {
    status: String,
}

#[cfg(target_os = "android")]
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MobileUpdateProgress {
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    download_finished: bool,
}

fn trusted_android_download_url(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let trusted = parsed.scheme() == "https"
        && parsed.host_str() == Some("github.com")
        && parsed
            .path()
            .starts_with("/stophemo/Woo/releases/download/")
        && parsed.path().ends_with("/Woo_android-arm64-v8a.apk");
    trusted.then(|| parsed.to_string())
}

fn fallback_android_download_url(version: &Version) -> String {
    format!("{ANDROID_RELEASE_BASE_URL}/v{version}/Woo_android-arm64-v8a.apk")
}

#[cfg(any(target_os = "android", test))]
fn trusted_android_download_url_for_version(url: &str, version: &Version) -> Option<String> {
    let trusted_url = trusted_android_download_url(url)?;
    let parsed = reqwest::Url::parse(&trusted_url).ok()?;
    let expected_path =
        format!("/stophemo/Woo/releases/download/v{version}/Woo_android-arm64-v8a.apk");
    (parsed.path() == expected_path).then_some(trusted_url)
}

#[cfg(target_os = "android")]
fn emit_download_progress(
    app: &AppHandle,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    download_finished: bool,
) {
    let _ = app.emit(
        MOBILE_UPDATE_PROGRESS_EVENT,
        MobileUpdateProgress {
            downloaded_bytes,
            total_bytes,
            download_finished,
        },
    );
}

#[cfg(target_os = "android")]
async fn is_valid_cached_apk(path: &Path) -> bool {
    let Ok(metadata) = tokio::fs::metadata(path).await else {
        return false;
    };
    if !metadata.is_file()
        || metadata.len() < MIN_ANDROID_APK_BYTES
        || metadata.len() > MAX_ANDROID_APK_BYTES
    {
        return false;
    }

    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return false;
    };
    let mut magic = [0_u8; 4];
    file.read_exact(&mut magic).await.is_ok() && magic == *b"PK\x03\x04"
}

#[cfg(target_os = "android")]
async fn download_mobile_update(
    app: &AppHandle,
    version: &Version,
    download_url: &str,
) -> Result<MobileUpdateDownload, String> {
    let trusted_url = trusted_android_download_url_for_version(download_url, version)
        .ok_or_else(|| "更新下载地址不受信任".to_string())?;
    let _download_guard = MOBILE_UPDATE_DOWNLOAD_LOCK.lock().await;

    let update_dir = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法访问更新缓存目录: {error}"))?
        .join("updates");
    tokio::fs::create_dir_all(&update_dir)
        .await
        .map_err(|error| format!("无法创建更新缓存目录: {error}"))?;

    let apk_path = update_dir.join(format!("Woo-{version}-arm64-v8a.apk"));
    if is_valid_cached_apk(&apk_path).await {
        let size = tokio::fs::metadata(&apk_path)
            .await
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        emit_download_progress(app, size, Some(size), true);
        return Ok(MobileUpdateDownload {
            apk_path: apk_path.to_string_lossy().into_owned(),
            cached: true,
        });
    }

    let _ = tokio::fs::remove_file(&apk_path).await;
    let partial_path = PathBuf::from(format!("{}.part", apk_path.to_string_lossy()));
    let _ = tokio::fs::remove_file(&partial_path).await;

    let result = async {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(20))
            .timeout(Duration::from_secs(15 * 60))
            .user_agent(format!(
                "Woo/{} Android Updater",
                app.package_info().version
            ))
            .build()
            .map_err(|error| format!("无法启动更新下载: {error}"))?;
        let mut response = client
            .get(trusted_url)
            .send()
            .await
            .map_err(|error| format!("无法连接更新服务器: {error}"))?
            .error_for_status()
            .map_err(|error| format!("更新服务器返回异常状态: {error}"))?;

        let total_bytes = response.content_length();
        if total_bytes.is_some_and(|size| size > MAX_ANDROID_APK_BYTES) {
            return Err("更新安装包超过大小限制".to_string());
        }

        let mut file = tokio::fs::File::create(&partial_path)
            .await
            .map_err(|error| format!("无法创建更新安装包: {error}"))?;
        let mut downloaded_bytes = 0_u64;
        let mut last_emitted_bytes = 0_u64;
        emit_download_progress(app, 0, total_bytes, false);

        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("更新安装包下载中断: {error}"))?
        {
            downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as u64);
            if downloaded_bytes > MAX_ANDROID_APK_BYTES {
                return Err("更新安装包超过大小限制".to_string());
            }
            file.write_all(&chunk)
                .await
                .map_err(|error| format!("无法写入更新安装包: {error}"))?;

            if downloaded_bytes.saturating_sub(last_emitted_bytes) >= 256 * 1024
                || total_bytes == Some(downloaded_bytes)
            {
                emit_download_progress(app, downloaded_bytes, total_bytes, false);
                last_emitted_bytes = downloaded_bytes;
            }
        }

        file.flush()
            .await
            .map_err(|error| format!("无法保存更新安装包: {error}"))?;
        file.sync_all()
            .await
            .map_err(|error| format!("无法保存更新安装包: {error}"))?;
        drop(file);

        if !is_valid_cached_apk(&partial_path).await {
            return Err("下载的更新安装包无效".to_string());
        }
        tokio::fs::rename(&partial_path, &apk_path)
            .await
            .map_err(|error| format!("无法完成更新安装包写入: {error}"))?;
        emit_download_progress(app, downloaded_bytes, total_bytes, true);

        Ok(MobileUpdateDownload {
            apk_path: apk_path.to_string_lossy().into_owned(),
            cached: false,
        })
    }
    .await;

    if result.is_err() {
        let _ = tokio::fs::remove_file(&partial_path).await;
    }
    result
}

/// Android uses a dedicated APK update flow, while this command shares the
/// release manifest and version comparison rules with the desktop updater.
#[tauri::command(rename_all = "camelCase", rename = "appCheckMobileUpdate")]
pub async fn app_check_mobile_update(app: AppHandle) -> CommandResult<Option<MobileUpdateInfo>> {
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(45))
        .user_agent(format!("Woo/{}", app.package_info().version))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            log::warn!("[Updater][Mobile] 无法创建网络客户端: {error}");
            return CommandResult::error("无法启动更新检查");
        }
    };

    let response = match client.get(UPDATE_MANIFEST_URL).send().await {
        Ok(response) => response,
        Err(error) => {
            log::warn!("[Updater][Mobile] 请求更新信息失败: {error}");
            return CommandResult::error("无法连接更新服务器，请稍后重试");
        }
    };

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(error) => {
            log::warn!("[Updater][Mobile] 更新服务器返回异常状态: {error}");
            return CommandResult::error("更新服务器暂时不可用");
        }
    };

    let manifest = match response.json::<UpdateManifest>().await {
        Ok(manifest) => manifest,
        Err(error) => {
            log::warn!("[Updater][Mobile] 更新信息格式无效: {error}");
            return CommandResult::error("更新信息格式无效");
        }
    };

    let version_text = manifest.version.trim().trim_start_matches('v');
    let latest_version = match Version::parse(version_text) {
        Ok(version) => version,
        Err(error) => {
            log::warn!(
                "[Updater][Mobile] 远端版本号无效 '{}': {error}",
                manifest.version
            );
            return CommandResult::error("更新版本号无效");
        }
    };
    let current_version = app.package_info().version.clone();

    if latest_version <= current_version {
        return CommandResult::success(None);
    }

    let download_url = manifest
        .android
        .and_then(|download| trusted_android_download_url(&download.url))
        .unwrap_or_else(|| fallback_android_download_url(&latest_version));

    CommandResult::success(Some(MobileUpdateInfo {
        current_version: current_version.to_string(),
        version: latest_version.to_string(),
        notes: manifest.notes,
        pub_date: manifest.pub_date,
        download_url,
    }))
}

#[tauri::command(rename_all = "camelCase", rename = "appDownloadMobileUpdate")]
pub async fn app_download_mobile_update(
    app: AppHandle,
    version: String,
    download_url: String,
) -> CommandResult<MobileUpdateDownload> {
    #[cfg(target_os = "android")]
    {
        let version = match Version::parse(version.trim().trim_start_matches('v')) {
            Ok(version) => version,
            Err(error) => {
                log::warn!("[Updater][Mobile] 下载版本号无效 '{version}': {error}");
                return CommandResult::error("更新版本号无效");
            }
        };

        match download_mobile_update(&app, &version, &download_url).await {
            Ok(download) => CommandResult::success(download),
            Err(error) => {
                log::warn!("[Updater][Mobile] APK 下载失败: {error}");
                CommandResult::error(&error)
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, version, download_url);
        CommandResult::error("当前平台不支持 APK 应用内更新")
    }
}

#[tauri::command(rename_all = "camelCase", rename = "appInstallMobileUpdate")]
pub async fn app_install_mobile_update(
    app: AppHandle,
    apk_path: String,
) -> CommandResult<MobileInstallResult> {
    #[cfg(target_os = "android")]
    {
        match android_updater::install(&app, apk_path).await {
            Ok(result) => CommandResult::success(result),
            Err(error) => {
                log::warn!("[Updater][Mobile] 无法启动 APK 安装: {error}");
                CommandResult::error(&format!("无法启动系统安装器: {error}"))
            }
        }
    }

    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, apk_path);
        CommandResult::error("当前平台不支持 APK 安装")
    }
}

#[cfg(target_os = "android")]
pub(crate) mod android_updater {
    use super::MobileInstallResult;
    use serde::Serialize;
    use tauri::{
        plugin::{Builder, PluginHandle, TauriPlugin},
        AppHandle, Manager, Runtime,
    };

    const PLUGIN_IDENTIFIER: &str = "com.nonegonotes.woo.tauri";

    #[derive(Debug)]
    struct AndroidUpdater<R: Runtime>(PluginHandle<R>);

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct InstallApkArgs {
        apk_path: String,
    }

    pub(crate) fn init<R: Runtime>() -> TauriPlugin<R> {
        Builder::<R>::new("mobile-updater")
            .setup(|app, api| {
                let handle =
                    api.register_android_plugin(PLUGIN_IDENTIFIER, "MobileUpdaterPlugin")?;
                app.manage(AndroidUpdater(handle));
                Ok(())
            })
            .build()
    }

    pub(super) async fn install<R: Runtime>(
        app: &AppHandle<R>,
        apk_path: String,
    ) -> Result<MobileInstallResult, String> {
        let handle = app.state::<AndroidUpdater<R>>().0.clone();
        handle
            .run_mobile_plugin_async("installApk", InstallApkArgs { apk_path })
            .await
            .map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        fallback_android_download_url, trusted_android_download_url,
        trusted_android_download_url_for_version,
    };
    use semver::Version;

    #[test]
    fn accepts_only_the_stable_android_release_asset() {
        let trusted =
            "https://github.com/stophemo/Woo/releases/download/v0.6.5/Woo_android-arm64-v8a.apk";
        assert_eq!(
            trusted_android_download_url(trusted).as_deref(),
            Some(trusted)
        );

        assert!(trusted_android_download_url(
            "http://github.com/stophemo/Woo/releases/download/v0.6.5/Woo_android-arm64-v8a.apk"
        )
        .is_none());
        assert!(
            trusted_android_download_url("https://example.com/Woo_android-arm64-v8a.apk").is_none()
        );
        assert!(trusted_android_download_url(
            "https://github.com/stophemo/Woo/releases/download/v0.6.5/other.apk"
        )
        .is_none());
    }

    #[test]
    fn builds_a_stable_fallback_download_url() {
        let version = Version::parse("0.6.5").unwrap();
        assert_eq!(
            fallback_android_download_url(&version),
            "https://github.com/stophemo/Woo/releases/download/v0.6.5/Woo_android-arm64-v8a.apk"
        );
    }

    #[test]
    fn binds_the_download_asset_to_the_requested_version() {
        let version = Version::parse("0.6.5").unwrap();
        let matching =
            "https://github.com/stophemo/Woo/releases/download/v0.6.5/Woo_android-arm64-v8a.apk";
        let mismatched =
            "https://github.com/stophemo/Woo/releases/download/v0.6.4/Woo_android-arm64-v8a.apk";

        assert_eq!(
            trusted_android_download_url_for_version(matching, &version).as_deref(),
            Some(matching)
        );
        assert!(trusted_android_download_url_for_version(mismatched, &version).is_none());
    }
}
