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

## 启动指南

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
