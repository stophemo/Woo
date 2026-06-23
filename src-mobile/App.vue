<script setup lang="ts">
// 移动端根组件 - 使用 Vant 的 Tabbar 底部导航
import { ref } from 'vue'
import { useRouter, useRoute } from 'vue-router'

const router = useRouter()
const route = useRoute()

const tabRoutes: Record<string, number> = {
  '/': 0,
  '/settings': 2,
}

const active = ref(tabRoutes[route.path] ?? 0)

function onTabChange(index: number) {
  const paths = ['/', '/drafts', '/settings']
  router.push(paths[index])
}
</script>

<template>
  <div class="mobile-app">
    <router-view />
    <van-tabbar v-model="active" @change="onTabChange" safe-area-inset-bottom>
      <van-tabbar-item icon="home-o">笔记</van-tabbar-item>
      <van-tabbar-item icon="edit-o">草稿</van-tabbar-item>
      <van-tabbar-item icon="setting-o">设置</van-tabbar-item>
    </van-tabbar>
  </div>
</template>

<style>
.mobile-app {
  min-height: 100vh;
  background: var(--van-background);
}
</style>
