<script setup lang="ts">
import { ref } from 'vue'
import { showToast } from 'vant'

const appVersion = __APP_VERSION__
</script>

<template>
  <div class="settings-page">
    <van-nav-bar title="设置" />

    <van-cell-group title="数据">
      <van-cell title="云同步" value="未登录" is-link />
    </van-cell-group>

    <van-cell-group title="关于">
      <van-cell title="版本" :value="`v${appVersion}`" />
      <van-cell title="项目主页" value="GitHub" is-link @click="showToast('打开链接')" />
    </van-cell-group>
  </div>
</template>
