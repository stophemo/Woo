# Woo（无我笔记）

Woo（无我笔记）是一款专注写作的 Markdown 桌面笔记软件。

## 特性

- 简洁的 Markdown 编辑体验
- Git 版本管理支持
- 思维导图与大纲视图
- AI 辅助写作能力（规划中）

## 技术栈

- **前端**: Vue 3 + TypeScript + Pinia
- **桌面端**: Electron + Vite
- **后端**: Spring Boot 3 + Spring Cloud Gateway
- **持久化**: MyBatis Plus + MySQL
- **认证**: JWT

## 开发指南

### 前端（app）

```bash
cd app
npm install

# 启动开发服务器
npm run dev

# 构建生产版本
npm run build

# 构建 Electron 应用
npm run electron:build
```

### 后端（services）

```bash
cd services
mvn clean install
```

各微服务（auth-service、note-service、gateway）可独立启动，数据库初始化脚本见 `services/sql/init.sql`。

## 项目结构

```
Woo/
├── app/                    # 前端应用（Vue 3 + Electron）
│   ├── electron/           # Electron 主进程和预加载脚本
│   ├── src/                # Vue 3 源码（components / stores / types 等）
│   ├── docs/               # 前端设计文档
│   └── package.json
└── services/               # 后端微服务（Spring Boot）
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
