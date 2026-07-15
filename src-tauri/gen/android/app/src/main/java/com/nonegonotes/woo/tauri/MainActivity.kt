package com.nonegonotes.woo.tauri

import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
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
      windowInsets
    }
    ViewCompat.requestApplyInsets(contentView)
  }
}
