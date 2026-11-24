import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

const cases = [
  {
    file: 'src-mobile/views/SyncSpace.vue',
    color: 'var(--van-text-color)',
    background: 'var(--van-background-2)',
  },
  {
    file: 'src/components/settings/SyncSpaceSettings.vue',
    color: 'var(--text-primary',
    background: null,
  },
]

test('六位码在深色/浅色主题下使用前景色而不是主题强调色', () => {
  for (const item of cases) {
    const source = readFileSync(new URL(`../${item.file}`, import.meta.url), 'utf8')
    const rule = source.match(/\.pairing-code\s*\{([^}]*)\}/s)?.[1]
    assert.ok(rule, `${item.file} 缺少 .pairing-code 样式`)
    assert.ok(!rule.includes('--van-primary-color'), `${item.file} 不应使用低对比度主色`)
    assert.ok(rule.includes(item.color), `${item.file} 应使用 ${item.color}`)
    if (item.background) {
      assert.ok(rule.includes(`background: ${item.background}`), `${item.file} 应有明确的深色/浅色背景`)
    }
  }
})
