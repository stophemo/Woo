# Woo（无我笔记）

Woo（无我笔记）是一款专注写作的 Markdown 笔记软件，提供**云端同步版**和**纯本地版**两种部署方式。

## 版本说明

| 版本 | 目录 | 特点 | 适用场景 |
|------|------|------|---------|
| **云端版** | `app/` + `services/` | 需要 MySQL + Nacos + 后端服务，支持多用户、云同步 | 团队协作、多设备同步 |
| **本地版** | `app-local/` | 纯 Electron + SQLite，无需任何后端，数据完全本地存储 | 个人使用、离线写作、隐私保护 |

## 本地版快速开始（推荐）

**无需安装数据库、无需启动后端服务**，下载即用：

```powershell
cd app-local
npm install
npm run dev            # 开发模式
npm run electron:build # 打包桌面应用
```

打包产物位于 `app-local/release/`，包含：
- `woo-local-{version}-win-x64-setup.exe` — NSIS 安装包
- `woo-local-{version}-win-x64-portable.exe` — 单文件绿色版
- `woo-local-{version}-win-x64.zip` — 压缩包绿色版

数据存储位置：`%APPDATA%/无我笔记/woo.db`

> 详细文档见：[app-local/README.md](app-local/README.md)

---

## 云端版启动指南

### 环境要求

- **Node.js**
- **JDK 17+**
- **Maven 3.8+**
- **MySQL 8.x**
- **Nacos 2.x**

### 后端启动（services）

后端由 `auth-service`、`note-service`、`gateway` 三个微服务组成，均依赖 Nacos 注册中心和 MySQL，**必须按顺序启动**。

#### 1. 准备数据库

```bash
# 登录 MySQL 后执行
mysql -u root -p < services/sql/init.sql
```

脚本会创建 `woo_notes` 数据库及所需表结构。如需修改账号密码，请同步调整各服务 `src/main/resources/application.yml` 中的 `spring.datasource` 配置。

#### 2. 启动 Nacos（先启动，否则业务服务注册失败）

下载 Nacos Server 2.x 后，在其 `bin` 目录执行：

```powershell
# Windows（standalone 模式）
.\startup.cmd -m standalone
```

确认 `http://localhost:8848/nacos` 可访问后再继续。

#### 3. 编译并启动微服务

```bash
cd services
mvn clean install -DskipTests
```

分别在三个终端启动（或使用 IDE 启动各自的 `*Application` 主类）：

```bash
# 终端 1：认证服务（端口 8081）
mvn -pl auth-service spring-boot:run

# 终端 2：笔记服务（端口 8082）
mvn -pl note-service spring-boot:run

# 终端 3：API 网关（端口 8080，对外统一入口）
mvn -pl gateway spring-boot:run
```

三个服务均在 Nacos 注册完成后，通过 `http://localhost:8080/api/**` 访问所有接口。

> 可选：执行 `services/logs/seed.ps1` 可向已启动的服务注入示例账号与测试笔记数据。

### 前端启动（app）

前端依赖后端网关（`http://localhost:8080`），建议先启动后端再启动前端。

```bash
cd app
npm install

# 方式一：仅启动 Web 开发服务器（默认 http://localhost:5173）
npm run dev

# 方式二：启动 Electron 桌面应用（需先运行 npm run dev 或 npm run build）
npm run electron:dev

# 构建生产版本
npm run build

# 打包 Electron 安装包
npm run electron:build
```

### 启动顺序速查

```
MySQL  →  Nacos  →  auth-service / note-service / gateway  →  前端 npm run dev
```

## 项目结构

```
Woo/
├── app/                    # 云端版前端（Vue 3 + Electron + 后端 API）
│   ├── electron/           # Electron 主进程和预加载脚本
│   ├── src/                # Vue 3 源码（components / stores / types 等）
│   ├── docs/               # 前端设计文档
│   └── package.json
├── app-local/              # 本地版前端（Vue 3 + Electron + SQLite）
│   ├── electron/           # Electron 主进程（SQLite 业务逻辑）
│   ├── src/                # Vue 3 源码（无认证、无 userId）
│   ├── build/              # 打包资源（图标等）
│   ├── release/            # 打包产物（gitignore）
│   └── package.json
└── services/               # 云端版后端微服务（Spring Boot）
    ├── common/             # 通用模块（实体、异常、工具类等）
    ├── auth-service/       # 认证服务
    ├── note-service/       # 笔记服务
    ├── gateway/            # API 网关
    ├── sql/                # 数据库初始化脚本
    └── pom.xml
```

## 贡献

欢迎提交 Issue 和 Pull Request 来改进项目。

## 许可证

本项目采用 [LICENSE](LICENSE) 许可证。
