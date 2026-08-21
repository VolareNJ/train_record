# 🏋️ 训练记录系统 (train_record)

> 用 Rust 重写的健身训练记录系统 —— 从 OneNote + Excel + CSV 的繁琐流程，升级为**服务器部署、手机/电脑随时访问、训练即时落库**的现代化 Web 应用。

原 Python 版工作流（繁琐）：OneNote 规划 → 训练时手动记录 → 回家复制粘贴到 Excel → pandas 导入 CSV 归档。

新系统愿景：浏览器打开即可用，训练时边练边记，**即时保存到数据库**，历史随时回看。

---

## ✨ 核心特性

| 特性 | 说明 |
|------|------|
| 👥 多用户 | 账号系统，数据互相隔离；**管理员邀请制**注册，避免陌生人注册 |
| 🗂️ 训练阶段 (Phase) | 一段训练期归档为一个阶段，可只读回看、重新启用 |
| 📋 计划与模板 | A/B 分化模板绑定阶段，每日人工制定计划 |
| ✅ 今日页 | 训练动作列表 + 整行状态色（绿=完成/黄=记录未完成/灰=未训练），点击任意动作即可记录/编辑 |
| ⚡ 即时保存 | 训练记录自动落库，无需"归档"按钮 |
| 📊 历史回顾 | 年月导航日历 + 按动作查看 + 当天详情（按部位分组）+ 动作详情（1RM/2RM/3RM 表格 + Chart.js 折线图） |
| 🔁 重量换算 | 界面内置换算器（bar/support/std），观测强度 + 计重方式展示串，杆重/单位按动作可配置 |
| 🔒 数据安全 | 网页下载 .db 备份 + 上传恢复 + CSV/JSON 导出 |
| 📱 PWA | 手机可"添加到主屏幕"全屏使用，静态资源离线缓存 |

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

### 部署（生产环境）

> 本节是**实测过的部署流程**（Linux，Ubuntu 24.04）。部署与开发可**同时进行、互不干扰**：80 供实际使用，8080 供开发调试。

#### 编译后：拿什么、清什么

Rust 的 `target/` 目录是编译缓存，体积巨大（可达 1.5GB+），分为三块：

| 内容 | 大小 | 说明 |
|------|------|------|
| `target/release/train_record` | ~8M | ✅ **唯一要部署的产物**（单文件，迁移已编译进二进制，自带建表） |
| `target/release/` 其余（.rlib/.d 等） | ~540M | ✅ 可删（依赖的中间产物，重编会再生成） |
| `target/debug/` | ~900M | ✅ 可删（开发调试版） |

```bash
cargo clean        # 删除整个 target/，释放全部空间（下次 build 重新编译）
cargo build --release   # 只编译优化版，不碰 debug
```

> 实际部署只需要**两个东西**：`target/release/train_record` + `static/` 目录（CSS/JS）。
> 数据库文件由程序启动时自动创建，无需手动建。

#### 部署位置（Linux 惯例）

| 项目 | 路径 | 说明 |
|------|------|------|
| 程序 | `/opt/train_record/train_record` | `/opt` = 第三方软件目录 |
| 静态文件 | `/opt/train_record/static/` | 与程序同目录（代码里是相对路径 `static/`，必须在该目录启动） |
| 数据库 | `/var/lib/train_record/train_record.db` | `/var/lib` = 应用数据目录 |
| 运行日志 | `/opt/train_record/app.log` | 启动输出 |

#### 后台拉起程序

```bash
# 1. 编译 release 版
cargo clean && cargo build --release

# 2. 部署
mkdir -p /opt/train_record/static /var/lib/train_record
cp target/release/train_record /opt/train_record/
cp static/* /opt/train_record/static/

# 3. 后台启动（nohup = 关终端不杀进程；& = 放后台）
cd /opt/train_record
PORT=80 DATABASE_PATH=/var/lib/train_record/train_record.db \
ADMIN_USERNAME=admin ADMIN_PASSWORD=admin123 \
nohup ./train_record > app.log 2>&1 &

# 4. 验证
curl -s http://127.0.0.1:80/login   # 200 = 成功
tail -f /opt/train_record/app.log   # 查看启动日志
```

> ⚠️ 80 端口需要 root 权限。生产环境建议后续改用 systemd 托管（开机自启 + 崩溃重启），模板见 `docs/structure.md` §7。

#### 部署与开发同时运行（不冲突）

| 端口 | 模式 | 数据库 | 用途 |
|------|------|--------|------|
| 80 | release 部署版 | `/var/lib/train_record/train_record.db`（空库） | 实际使用 |
| 8080 | debug 开发版 | `./train_record.db`（测试数据） | 开发调试 |

两者**完全独立**：端口不同（出入口不同）、数据库不同（数据隔离）、静态文件各归各。
以后更新部署版：`cargo build --release` → 复制新二进制到 `/opt/train_record/` → 杀掉旧进程重启，开发版 8080 可一直开着。

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
├── Cargo.toml              # 依赖清单（axum 含 multipart）
├── rust-toolchain.toml     # 固定 nightly 工具链（rustfmt 需要）
├── rustfmt.toml            # 格式化配置（大括号换行）
├── migrations/             # SQLite 迁移（自动执行，幂等）
│   ├── 0001_init.sql       # 8 张基础表
│   ├── 0002_sessions.sql   # 会话表
│   ├── 0003_plan_items_metadata.sql  # 计划项计重元数据
│   ├── 0004_plan_items_note.sql      # 计划项备注
│   ├── 0005_exercises_sort_order_records_completed.sql
│   ├── 0006_users_body_weight.sql    # 全局体重
│   └── 0007_exercises_default_unit.sql  # 默认计重单位
├── src/
│   ├── main.rs             # 入口：路由注册 + 首页
│   ├── config.rs           # 配置（端口/数据库路径/密钥/部位顺序）
│   ├── error.rs            # 统一错误类型 → HTTP 状态码
│   ├── db.rs               # 数据库连接池 + 迁移
│   ├── models.rs           # 数据模型（User/Phase/Exercise/...）
│   ├── calc.rs             # 1RM 纯函数（Epley/Wathan）+ 单元测试
│   ├── handlers/
│       ├── auth.rs         # 登录/登出/用户管理/体重维护
│       ├── phases.rs       # 阶段管理
│       ├── exercises.rs    # 动作库 CRUD
│       ├── plan.rs         # 模板 + 当日计划
│       ├── record.rs       # 今日页 + 记录表单 + 保存 + 计重展示串
│       ├── stats.rs        # 历史回顾（日历/当天详情/动作详情/图表）
│       └── backup.rs       # 备份（下载/上传恢复/CSV+JSON 导出）
│   └── api/                # M8：REST API 层（/api/v1，为 iced GUI 客户端铺路）
│       ├── mod.rs          # ApiError + 全部路由注册
│       ├── auth.rs         # ApiAuthUser 守卫 + login/logout/me
│       ├── phases.rs       # 阶段 CRUD + 归档
│       ├── exercises.rs    # 动作 CRUD + 筛选 + 1RM
│       ├── plans.rs        # 模板/计划全 CRUD + 事务
│       ├── records.rs      # today/upsert/列表/更新/删除
│       └── stats.rs        # calendar/history_day/exercise_stats
├── static/
│   ├── manifest.json       # PWA 清单
│   ├── sw.js               # Service Worker（静态资源离线缓存）
│   └── weight_converter.js # 重量换算器
├── sw.js                   # Service Worker（M6 移到根目录，作用域才覆盖全站）
├── docs/
│   ├── proposal.md         # 项目背景与动机
│   ├── structure.md        # 完整设计文档（需求/表结构/页面/计划）
│   ├── todo.md             # 待办与设计决策（跨会话）
│   └── learning_path/      # 🗺️ 分阶段开发路径图
│       ├── M0.md ~ M8.md   # 各阶段路径图（M0-M8 ✅ 全部完成）
│       ├── M1_ref/ M4_ref/ M8_ref/ # 参考答案
│       ├── M4_bugfix_notes.md      # M4 后 Bug 复盘
│       └── M5_roadmap_notes.md     # M5 前路线复盘（含 GUI 决策）
├── Python_pkg/             # 原 Python 版（历史数据与参考）
│   ├── sys.py              # 原记录程序
│   └── all_data/           # 原 CSV 数据（按训练阶段组织）
└── train_record.db         # SQLite 数据库（运行时自动生成）
```

---

## 🗺️ 开发进度

| 里程碑 | 内容 | 状态 |
|--------|------|------|
| **M0** | 脚手架：配置/错误/模型/数据库迁移/服务器启动 | ✅ 已完成 |
| **M1** | 认证：注册/登录/登出、管理员邀请制、路由守卫 | ✅ 已完成（6 项实测通过） |
| **M2** | 基础数据：阶段管理、动作库、坚持天数 | ✅ 已完成（CRUD + 理解验证通过） |
| **M3** | 计划：模板（A/B 分化）、按日计划 | ✅ 已完成（模板 + 当日计划 + 理解验证通过） |
| **M4** | 训练记录：今日页、记录/编辑、重量换算器、即时保存 | ✅ 已完成（Upsert 落库 + 理解验证通过） |
| **M5** | 历史回顾：日历导航、当天详情、动作详情折线图/1RM | ✅ 已完成（含理解验证，2026-08-14 收官） |
| **M6** | 备份与体验：.db 下载/上传恢复、CSV/JSON 导出、PWA | ✅ 已完成（含理解验证，2026-08-18 收官） |
| **M7** | 打磨：热替换连接池、未登录跳转、排序、美化、离线、部署 | ✅ 已完成（含理解验证，2026-08-21 收官） |
| **M8** | REST API 层（为 iced GUI 客户端铺路） | ✅ 已完成（认证/阶段/动作/计划/记录/统计 + 数据隔离实测，2026-08-21 收官） |

> 开发是**边写边学**模式：每个文件都带有【教学注释】，从 [`docs/learning_path/M0.md`](docs/learning_path/M0.md) 开始阅读。

---

## 📚 文档导航

- [`docs/proposal.md`](docs/proposal.md) —— 项目背景：为什么重写
- [`docs/structure.md`](docs/structure.md) —— **设计地基**：完整需求结论、数据库 DDL、页面规格、开发计划
- [`docs/todo.md`](docs/todo.md) —— 待办与设计决策（跨会话备忘）
- [`docs/learning_path/M0.md`](docs/learning_path/M0.md) ~ [`M8.md`](docs/learning_path/M8.md) —— **分阶段开发路径图**（M0-M8 已完成）
- [`docs/learning_path/M4_bugfix_notes.md`](docs/learning_path/M4_bugfix_notes.md) —— M4 后 Bug 修复复盘（iced 必考清单）
- [`docs/learning_path/M5_roadmap_notes.md`](docs/learning_path/M5_roadmap_notes.md) —— M5 前能力评估与 GUI 技术栈决策

> 💡 部署相关（编译产物取舍、目录位置、后台运行、80/8080 共存）见上方「🚀 快速开始 → 部署（生产环境）」。

---

## 🤝 协作约定

- **代码风格**：函数式优先（迭代器/适配器）、大括号换行（Allman）、中文注释、**禁止 unsafe**
- **文档先行**：需求变更先改 `docs/structure.md`，再改代码
- **教学注释**：关键代码带【教学说明】，边开发边学习

---

## 📄 License

私有项目，仅供个人使用。
