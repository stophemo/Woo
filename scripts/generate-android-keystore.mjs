#!/usr/bin/env node

import { randomBytes } from 'node:crypto'
import { chmodSync, mkdirSync, existsSync, readFileSync, writeFileSync } from 'node:fs'
import { spawnSync } from 'node:child_process'
import { resolve } from 'node:path'

const signingDir = resolve('.signing')
const keystorePath = resolve(signingDir, 'woo-release.jks')
const envPath = resolve(signingDir, 'android-signing.env')
const base64Path = resolve(signingDir, 'woo-release.jks.base64')
const githubEnvPath = resolve(signingDir, 'github-actions.env')

mkdirSync(signingDir, { recursive: true, mode: 0o700 })

function writeGitHubEnv(password, alias) {
  const githubEnvContent = [
    `ANDROID_KEYSTORE_BASE64=${readFileSync(keystorePath).toString('base64')}`,
    `ANDROID_KEYSTORE_PASSWORD=${password}`,
    `ANDROID_KEY_ALIAS=${alias}`,
    `ANDROID_KEY_PASSWORD=${password}`,
    '',
  ].join('\n')
  writeFileSync(githubEnvPath, githubEnvContent, { mode: 0o600 })
}

if (process.argv.includes('--export-github-env')) {
  if (!existsSync(keystorePath) || !existsSync(envPath)) {
    console.error('未找到现有 Android release keystore 或本地签名变量。')
    process.exit(1)
  }
  const envContent = readFileSync(envPath, 'utf8')
  const readValue = (name) => {
    const match = envContent.match(new RegExp(`^${name}='([^']+)'$`, 'm'))
    if (!match) throw new Error(`本地签名变量缺少 ${name}`)
    return match[1]
  }
  writeGitHubEnv(
    readValue('ANDROID_KEYSTORE_PASSWORD'),
    readValue('ANDROID_KEY_ALIAS'),
  )
  console.log(`GitHub Actions Secret 文件已保存：${githubEnvPath}`)
  process.exit(0)
}

if (existsSync(keystorePath) || existsSync(envPath)) {
  console.error('签名材料已存在，拒绝覆盖。请先备份并确认是否需要沿用旧密钥。')
  process.exit(1)
}

const alias = 'woo-release'
const password = randomBytes(32).toString('base64url')
const keytool = process.env.KEYTOOL || 'keytool'
const result = spawnSync(keytool, [
  '-genkeypair',
  '-keystore', keystorePath,
  '-storetype', 'JKS',
  '-storepass', password,
  '-keypass', password,
  '-alias', alias,
  '-keyalg', 'RSA',
  '-keysize', '4096',
  '-sigalg', 'SHA256withRSA',
  '-validity', '10000',
  '-dname', 'CN=Woo Notes, OU=Release, O=Woo Notes, L=Shanghai, ST=Shanghai, C=CN',
], { stdio: 'inherit' })

if (result.status !== 0) {
  console.error('Android release keystore 生成失败。')
  process.exit(result.status ?? 1)
}

chmodSync(keystorePath, 0o600)

const envContent = [
  `ANDROID_KEYSTORE_PATH='${keystorePath}'`,
  `ANDROID_KEYSTORE_PASSWORD='${password}'`,
  `ANDROID_KEY_ALIAS='${alias}'`,
  `ANDROID_KEY_PASSWORD='${password}'`,
  '',
].join('\n')

writeFileSync(envPath, envContent, { mode: 0o600 })
writeFileSync(base64Path, readFileSync(keystorePath).toString('base64'), { mode: 0o600 })
writeGitHubEnv(password, alias)

console.log(`Android release keystore 已生成：${keystorePath}`)
console.log(`本地签名变量已保存：${envPath}`)
console.log(`GitHub Secret 源文件已保存：${base64Path}`)
console.log(`GitHub Actions Secret 文件已保存：${githubEnvPath}`)
console.log('请将整个 .signing 目录离线备份；丢失此密钥后将无法覆盖升级已安装应用。')
