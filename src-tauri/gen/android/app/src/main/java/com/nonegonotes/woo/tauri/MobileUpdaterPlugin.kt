package com.nonegonotes.woo.tauri

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.activity.result.ActivityResult
import androidx.core.content.FileProvider
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File

@InvokeArg
class InstallApkArgs {
  lateinit var apkPath: String
}

@TauriPlugin
class MobileUpdaterPlugin(private val activity: Activity) : Plugin(activity) {
  @Command
  fun installApk(invoke: Invoke) {
    try {
      val apkFile = resolveApkFile(invoke)

      if (
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
        !activity.packageManager.canRequestPackageInstalls()
      ) {
        openInstallPermissionSettings(invoke)
        return
      }

      launchInstaller(apkFile)
      resolveStatus(invoke, "installStarted")
    } catch (error: Exception) {
      invoke.reject(error.message ?: "无法打开系统安装器")
    }
  }

  @ActivityCallback
  fun installPermissionResult(invoke: Invoke, _result: ActivityResult) {
    try {
      if (
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
        !activity.packageManager.canRequestPackageInstalls()
      ) {
        resolveStatus(invoke, "permissionRequired")
        return
      }

      launchInstaller(resolveApkFile(invoke))
      resolveStatus(invoke, "installStarted")
    } catch (error: Exception) {
      invoke.reject(error.message ?: "无法打开系统安装器")
    }
  }

  private fun resolveApkFile(invoke: Invoke): File {
    val args = invoke.parseArgs(InstallApkArgs::class.java)
    val apkFile = File(args.apkPath).canonicalFile
    val cacheDirectory = activity.cacheDir.canonicalFile
    val cachePrefix = cacheDirectory.path + File.separator

    require(apkFile.path.startsWith(cachePrefix) && apkFile.extension.lowercase() == "apk") {
      "安装包路径无效"
    }
    require(apkFile.isFile && apkFile.length() > 0L) { "安装包不存在" }
    return apkFile
  }

  private fun launchInstaller(apkFile: File) {
    val apkUri = FileProvider.getUriForFile(
      activity,
      "${activity.packageName}.fileprovider",
      apkFile
    )
    val installIntent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(apkUri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    activity.startActivity(installIntent)
  }

  private fun openInstallPermissionSettings(invoke: Invoke) {
    val packageSettings = Intent(
      Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
      Uri.parse("package:${activity.packageName}")
    )
    val intent = if (packageSettings.resolveActivity(activity.packageManager) != null) {
      packageSettings
    } else {
      Intent(Settings.ACTION_SECURITY_SETTINGS)
    }
    startActivityForResult(invoke, intent, "installPermissionResult")
  }

  private fun resolveStatus(invoke: Invoke, status: String) {
    val result = JSObject()
    result.put("status", status)
    invoke.resolve(result)
  }
}
