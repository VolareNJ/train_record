# 🏋️ 训练记录系统 (train_record)

> 用 Rust 重写的健身训练记录系统 —— 从 OneNote + Excel + CSV 的繁琐流程，升级为**服务器部署、手机/电脑随时访问、训练即时落库**的现代化 Web 应用。

原 Python 版工作流（繁琐）：OneNote 规划 → 训练时手动记录 → 回家复制粘贴到 Excel → pandas 导入 CSV 归档。

新系统愿景：浏览器打开即可用，训练时边练边记，**即时保存到数据库**，历史随时回看。

---

## ✨ 核心特性（规划）

| 特性 | 说明 |
|------|------|
| 👥 多用户 | 账号系统，数据互相隔离；**管理员邀请制**注册，避免陌生人注册 |
| 🗂️ 训练阶段 (Phase) | 一段训练期归档为一个阶段，可只读回看、重新启用 |
| 📋 计划与模板 | A/B 分化模板绑定阶段，每日人工制定计划 |
| ✅ 今日页 | 训练动作列表 + 状态徽标，点击任意动作即可查看/编辑 |
| ⚡ 即时保存 | 训练记录自动落库，无需"归档"按钮 |
| 📊 历史回顾 | 日历视图 + 动作详情（表格/折线图/1RM） |
| 🔁 重量换算 | 界面内置换算器（bar/support/std/lb2kg），杆重按动作可配置 |
| 🔒 数据安全 | 导出/导入 + CSV/JSON 导出，随时备份 |
| 📱 移动友好 | 手机浏览器访问（规划 PWA 添加到主屏幕） |

---

## 🛠️ 技术栈

| 层 | 技术 | 说明 |
|----|------|------|
| 语言 | **Rust** (edition 2024) | 函数式风格，迭代器/适配器 |
| Web 框架 | **Axum 0.8** | 异步、高性能 |
| 异步运行时 | **Tokio 1.x** | 全特性 |
| 数据库 | **SQLite** + **sqlx** | 单文件零配置，编译期检查 SQL |
| 模板引擎 | **Askama 0.16** | 服务端渲染 HTML |
| 认证 | **argon2** + cookie Session | 密码哈希、登录会话 |
| 前端 | 原生 HTML/CSS/JS | 无重型框架，轻量 |

完整依赖清单见 [`Cargo.toml`](Cargo.toml)。

---

## 🚀 快速开始

### 环境要求

- Rust **stable 1.95+**（编译）
- Rust **nightly**（仅 rustfmt 需要，用于大括号换行格式）

> 国内网络提示：本项目已配置 crates.io 国内镜像（`~/.cargo/config.toml`），rustup 源见 `~/.bashrc`。

### 安装与运行

```bash
# 1. 克隆项目
git clone <your-repo-url> train_record
cd train_record

# 2. 构建（首次会自动下载依赖，耗时较长）
cargo build

# 3. 运行（自动创建 SQLite 数据库并执行迁移）
cargo run

# 4. 浏览器访问
#    本机:   http://127.0.0.1:8080
#    局域网: http://<服务器IP>:8080
```

### 配置（环境变量）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `8080` | 监听端口 |
| `DATABASE_PATH` | `train_record.db` | SQLite 数据库文件路径（自动创建） |
| `SESSION_SECRET` | （内置默认） | 会话签名密钥，**生产环境必须设置！** |

示例：

```bash
PORT=3000 DATABASE_PATH=/data/train.db SESSION_SECRET=your-secret cargo run
```

### 代码格式化（大括号换行）

项目使用 nightly rustfmt 实现 **Allman 风格**（大括号换行）：

```bash
cargo fmt          # 格式化（rust-toolchain.toml 已固定 nightly）
cargo fmt --check  # 检查是否合规
```

---

## 📁 项目结构

```
train_record/
├── Cargo.toml              # 依赖清单
├── rust-toolchain.toml     # 固定 nightly 工具链（rustfmt 需要）
├── rustfmt.toml            # 格式化配置（大括号换行）
├── migrations/
│   └── 0001_init.sql       # 数据库表结构（7 张表，自动迁移）
├── src/
│   ├── main.rs             # 入口：组装一切，启动服务器
│   ├── config.rs           # 配置（端口/数据库路径/密钥）
│   ├── error.rs            # 统一错误类型 → HTTP 状态码
│   ├── db.rs               # 数据库连接池 + 迁移
│   └── models.rs           # 数据模型（User/Phase/Exercise/...）
├── docs/
│   ├── proposal.md         # 项目背景与动机
│   ├── structure.md        # 完整设计文档（需求/表结构/页面/计划）
│   └── learning_path/      # 🗺️ 分阶段开发路径图
│       ├── M0.md           # M0 脚手架路径图（已完成 ✅）
│       └── M1.md ~ M7.md   # 后续阶段（开工时创建）
├── Python_pkg/             # 原 Python 版（历史数据与参考）
│   ├── sys.py              # 原记录程序
│   ├── table.xlsx          # 原 Excel 记录表
│   └── all_data/           # 原 CSV 数据（按训练阶段组织）
└── train_record.db         # SQLite 数据库（运行时自动生成）
```

---

## 🗺️ 开发进度

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **M0** | 脚手架：配置/错误/模型/数据库迁移/服务器启动 | ✅ 已完成 |
| **M1** | 认证：注册/登录/登出、管理员邀请制、路由守卫 | 📝 定义完成，待实现 |
| **M2** | 基础数据：阶段管理、动作库、坚持天数 | ⬜ 未开始 |
| **M3** | 计划：模板（A/B 分化）、按日计划、今日页 | ⬜ 未开始 |
| **M4** | 训练记录：录入、重量换算器、即时保存 | ⬜ 未开始 |
| **M5** | 历史回顾：日历视图、动作详情、折线图/1RM | ⬜ 未开始 |
| **M6** | 备份：导出/导入、CSV/JSON 导出、PWA | ⬜ 未开始 |
| **M7** | 打磨：界面美化、响应式、错误处理 | ⬜ 未开始 |

> 开发是**边写边学**模式：每个文件都带有【教学注释】，从 [`docs/learning_path/M0.md`](docs/learning_path/M0.md) 开始阅读。

---

## 📚 文档导航

- [`docs/proposal.md`](docs/proposal.md) —— 项目背景：为什么重写
- [`docs/structure.md`](docs/structure.md) —— **设计地基**：完整需求结论、数据库 DDL、页面规格、开发计划
- [`docs/learning_path/M0.md`](docs/learning_path/M0.md) —— **开发路径图**：文件依赖顺序、M0 里程碑、常见坑（M1~M7 开工时各自创建）

---

## 🤝 协作约定

- **代码风格**：函数式优先（迭代器/适配器）、大括号换行（Allman）、中文注释、**禁止 unsafe**
- **文档先行**：需求变更先改 `docs/structure.md`，再改代码
- **教学注释**：关键代码带【教学说明】，边开发边学习

---

## 📄 License

私有项目，仅供个人使用。
