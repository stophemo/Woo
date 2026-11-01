package com.nonegonotes.woo.tauri

import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  override fun onWebViewCreate(webView: WebView) {
    appWebView = webView
  }

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    val contentView = findViewById<View>(android.R.id.content)
    val initialPaddingLeft = contentView.paddingLeft
    val initialPaddingTop = contentView.paddingTop
    val initialPaddingRight = contentView.paddingRight
    val initialPaddingBottom = contentView.paddingBottom

    ViewCompat.setOnApplyWindowInsetsListener(contentView) { view, windowInsets ->
      val topInset = windowInsets.getInsets(
        WindowInsetsCompat.Type.statusBars() or WindowInsetsCompat.Type.displayCutout()
      ).top

      // 顶部由原生层统一避让；底部继续交给 Vant safe-area 处理，避免重复 padding。
      view.setPadding(
        initialPaddingLeft,
        initialPaddingTop + topInset,
        initialPaddingRight,
        initialPaddingBottom
      )

      // 应用启用了 edge-to-edge，窗口不会因输入法弹出而 resize，
      // 所以 WebView 里的 visualViewport 看不到键盘，固定定位的底部工具条会被键盘盖住。
      // 这里把输入法遮挡高度（CSS px）直接推给前端，由前端自行上推底部工具条。
      val imeVisible = windowInsets.isVisible(WindowInsetsCompat.Type.ime())
      val imeBottom = if (imeVisible) {
        windowInsets.getInsets(WindowInsetsCompat.Type.ime()).bottom
      } else {
        0
      }
      publishImeInset(imeBottom)

      windowInsets
    }
    ViewCompat.requestApplyInsets(contentView)
  }

  private fun publishImeInset(pixelInset: Int) {
    val density = resources.displayMetrics.density
    val cssInset = if (pixelInset > 0) (pixelInset / density).toInt() else 0
    val script = "(function(){document.documentElement.style.setProperty('--woo-ime-inset','${cssInset}px');" +
      "window.dispatchEvent(new CustomEvent('woo:ime-inset',{detail:${cssInset}}));})()"
    appWebView?.evaluateJavascript(script, null)
  }
}
