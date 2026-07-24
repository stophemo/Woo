use crate::commands::CommandResult;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::AppHandle;

const UPDATE_MANIFEST_URL: &str =
    "https://github.com/stophemo/Woo/releases/latest/download/latest.json";
const ANDROID_RELEASE_BASE_URL: &str = "https://github.com/stophemo/Woo/releases/download";

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

/// Android does not use Tauri's desktop installer plugin. It only checks the
/// signed release channel here; the frontend then opens the APK download in
/// the system browser after an explicit user tap.
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

#[cfg(test)]
mod tests {
    use super::{fallback_android_download_url, trusted_android_download_url};
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
}
