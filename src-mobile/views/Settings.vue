<script setup lang="ts">
import { ref } from 'vue'
import { showToast } from 'vant'

const supabaseUrl = ref('')
const supabaseKey = ref('')
</script>

<template>
  <div class="settings-page">
    <van-nav-bar title="设置" />

    <van-cell-group title="账户">
      <van-cell title="登录" is-link to="/login" />
    </van-cell-group>

    <van-cell-group title="关于">
      <van-cell title="版本" value="0.5.1" />
    </van-cell-group>
  </div>
</template>
