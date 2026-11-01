package com.nonegonotes.woo.tauri

import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  private val darkThemes = listOf("dark", "ink", "graphite", "deepsea")

  // 主题可以在应用内随时切换，而 WebView 没有反向通知原生的通道；
  // 这里只在应用处于前台时低频轮询一次主题（一次字符串求值，开销可忽略）。
  private val chromeHandler = Handler(Looper.getMainLooper())
  private val chromeSyncTask = object : Runnable {
    override fun run() {
      syncChromeColors()
      chromeHandler.postDelayed(this, CHROME_SYNC_INTERVAL_MS)
    }
  }

  override fun onWebViewCreate(webView: WebView) {
    appWebView = webView
    // 系统字体缩放（MIUI 常见 1.2~1.4x）会被 WebView 直接套到所有文本上，
    // 把按 px 设计好的字号、行高、行距整体撑开，版式随之变形。
    // 界面字号由应用自己负责，因此把 textZoom 固定为 100%。
    webView.settings.textZoom = 100
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
      // WebView 没有 viewport-fit=cover 时 env(safe-area-inset-bottom) 恒为 0，
      // 底部导航条（手势胶囊）会压住 tabbar 文字，因此底部安全区也由原生补齐。
      val bottomInset = windowInsets.getInsets(WindowInsetsCompat.Type.navigationBars()).bottom

      view.setPadding(
        initialPaddingLeft,
        initialPaddingTop + topInset,
        initialPaddingRight,
        initialPaddingBottom + bottomInset
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

      // 避让出来的状态栏区域显示的是窗口背景，必须与当前主题一致，
      // 否则浅色主题下会留一条黑边。
      syncChromeColors()

      windowInsets
    }
    ViewCompat.requestApplyInsets(contentView)

    // WebView 首帧渲染前主题变量还取不到，前后多试几次，并用 onResume 兜底。
    for (delay in longArrayOf(600L, 1_600L, 3_200L)) {
      contentView.postDelayed({ syncChromeColors() }, delay)
    }
  }

  override fun onResume() {
    super.onResume()
    syncChromeColors()
    chromeHandler.removeCallbacks(chromeSyncTask)
    chromeHandler.postDelayed(chromeSyncTask, CHROME_SYNC_INTERVAL_MS)
  }

  override fun onPause() {
    chromeHandler.removeCallbacks(chromeSyncTask)
    super.onPause()
  }

  private fun publishImeInset(pixelInset: Int) {
    val density = resources.displayMetrics.density
    val cssInset = if (pixelInset > 0) (pixelInset / density).toInt() else 0
    val script = "(function(){document.documentElement.style.setProperty('--woo-ime-inset','${cssInset}px');" +
      "window.dispatchEvent(new CustomEvent('woo:ime-inset',{detail:${cssInset}}));})()"
    appWebView?.evaluateJavascript(script, null)
  }

  /**
   * 读取页面当前主题（html[data-theme] 与主题底色），同步状态栏底色与图标明暗。
   * 只能单次取值，因此靠 insets 变化与 onResume 触发，避免常驻轮询。
   */
  private fun syncChromeColors() {
    // 取「状态栏正下方实际显示的颜色」：不同页面底色不同（列表是纸面色、
    // 编辑器是画布色），统一取纸面色会留一条色差。
    val script = """
      (function(){
        function bgOf(el){
          while (el) {
            var c = getComputedStyle(el).backgroundColor;
            if (c && c !== 'rgba(0, 0, 0, 0)' && c !== 'transparent') return c;
            el = el.parentElement;
          }
          return getComputedStyle(document.body).backgroundColor || '';
        }
        var theme = document.documentElement.getAttribute('data-theme') || 'light';
        var probe = document.elementFromPoint(Math.round(window.innerWidth / 2), 4);
        var bg = probe ? bgOf(probe) : '';
        if (!bg) {
          var host = document.querySelector('.van-config-provider') || document.documentElement;
          bg = getComputedStyle(host).getPropertyValue('--van-background').trim();
        }
        return theme + '|' + bg;
      })()
    """.trimIndent().replace("\n", "")
    appWebView?.evaluateJavascript(script) { result ->
      val raw = result?.let { unquote(it.trim()) } ?: return@evaluateJavascript
      val parts = raw.split("|")
      if (parts.size != 2) return@evaluateJavascript
      val isDark = darkThemes.contains(parts[0])
      val color = parseCssColor(parts[1])
      runOnUiThread { applyChromeColors(isDark, color) }
    }
  }

  private companion object {
    const val CHROME_SYNC_INTERVAL_MS = 1_500L
  }

  private fun unquote(value: String): String {
    if (value.length >= 2 && value.startsWith("\"") && value.endsWith("\"")) {
      return value.substring(1, value.length - 1).replace("\\\"", "\"")
    }
    return value
  }

  private fun applyChromeColors(isDark: Boolean, color: Int?) {
    if (color != null) {
      window.setBackgroundDrawable(ColorDrawable(color))
      window.statusBarColor = color
    }
    val controller = WindowInsetsControllerCompat(window, window.decorView)
    controller.isAppearanceLightStatusBars = !isDark
    controller.isAppearanceLightNavigationBars = !isDark
  }

  /** 只处理 #rrggbb / #aarrggbb / rgb() / rgba()，取不到时返回 null（保持原值）。 */
  private fun parseCssColor(value: String): Int? {
    val text = value.trim()
    if (text.isEmpty()) return null

    if (text.startsWith("#")) {
      val hex = text.removePrefix("#")
      return when (hex.length) {
        6 -> runCatching { Color.parseColor("#$hex") }.getOrNull()
        8 -> runCatching {
          Color.argb(
            hex.substring(0, 2).toInt(16),
            hex.substring(2, 4).toInt(16),
            hex.substring(4, 6).toInt(16),
            hex.substring(6, 8).toInt(16),
          )
        }.getOrNull()
        else -> null
      }
    }

    if (!text.startsWith("rgb")) return null
    val numbers = Regex("""[\d.]+""").findAll(text).map { it.value }.toList()
    if (numbers.size < 3) return null
    val red = numbers[0].toFloatOrNull()?.toInt() ?: return null
    val green = numbers[1].toFloatOrNull()?.toInt() ?: return null
    val blue = numbers[2].toFloatOrNull()?.toInt() ?: return null
    val alpha = if (numbers.size >= 4) {
      ((numbers[3].toFloatOrNull() ?: 1f) * 255).toInt()
    } else {
      255
    }
    return Color.argb(alpha, red, green, blue)
  }
}
