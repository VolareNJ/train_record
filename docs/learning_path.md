# 🗺️ 开发路径图（从零到能跑）

> 这份文档告诉你：**从哪个文件开始写、按什么顺序、每个文件该写什么**。
> 配合 `structure.md`（设计稿）和每个 `.rs` 文件里的【教学注释】一起看。

---

## 一、项目地图：每个文件是干什么的

```
train_record/
├── Cargo.toml          # 依赖清单（相当于"购物单"）
├── migrations/
│   └── 0001_init.sql   # 数据库表结构（7 张表）
├── src/
│   ├── main.rs         # 入口：组装一切，启动服务器 ← 你先读这个
│   ├── config.rs       # 配置：端口/数据库路径/密钥
│   ├── error.rs        # 统一错误类型（把各种错误翻译成 HTTP 状态码）
│   ├── db.rs           # 数据库连接池 + 迁移
│   ├── models.rs       # 数据模型：User/Phase/Exercise/Template/Plan/Record
│   └── (以后会有)
│       ├── auth.rs     # M1 登录注册
│       ├── handlers/   # M2+ 各种页面处理器
│       └── ...
├── templates/          # (M2+) HTML 模板（askama）
├── static/             # (M2+) CSS/JS
└── docs/
    ├── proposal.md     # 项目背景和动机
    └── structure.md    # 完整设计文档（需求+表结构+页面+开发计划）
```

---

## 二、M0 阶段：先让服务器跑起来（已完成 ✅）

**顺序很重要，因为它体现了依赖关系**（下面的文件被上面的文件使用）：

```
第1步  Cargo.toml        先声明依赖，才能用 cargo add 装包
   ↓
第2步  src/config.rs     不依赖任何人，先写它
   ↓
第3步  src/error.rs      不依赖 config，但所有模块都要用它
   ↓
第4步  src/models.rs     用 sqlx 的 FromRow，给表定数据结构
   ↓
第5步  migrations/0001_init.sql  定义 7 张表的字段
   ↓
第6步  src/db.rs         连接数据库 + 自动执行迁移
   ↓
第7步  src/main.rs       把以上全部组装起来，启动服务器 ★ 最高层
```

**为什么是这个顺序？** Rust 编译器按 `main.rs` 里的 `mod` 声明找文件。
`main.rs` 是"指挥塔"，它依赖下面所有模块；所以越底层（config）越先写，
越顶层（main）越后写。

**M0 验收标准**（已通过）：
- [x] `cargo check` 无错误
- [x] `cargo run` 启动，日志显示"数据库迁移完成"
- [x] 访问 `http://服务器IP:8080/` 显示"训练记录系统 M0 脚手架运行成功"

---

## 三、M0 → M7 里程碑路线（来自 structure.md）

```
M0 脚手架（已完成）✅
   ↓ 能启动服务器、连上数据库
M1 认证
   ├── 注册/登录/登出（cookie session）
   ├── 管理员邀请注册（生成邀请码）
   └── 路由守卫（没登录不能进）
   ↓ 有了"你是谁"
M2 基础数据
   ├── 阶段管理（增删改、归档/启用）
   ├── 动作库（增删改查 + 部位分组）
   └── 坚持天数（今天 − 阶段开始日）
   ↓ 有了"练什么"
M3 计划
   ├── 模板（A/B 分化，绑定阶段）
   ├── 计划（按日生成，从模板复制动作）
   └── 今日页（状态徽标 + 动作列表）
   ↓ 有了"今天练什么"
M4 训练记录
   ├── 记录录入（组/次/重量/感觉/策略/要领）
   ├── 重量换算器（bar/support/std/lb2kg）
   ├── 即时保存（无"归档"按钮）
   └── 杆重按动作配置（olympic=20 等）
   ↓ 有了"练得怎么样"
M5 历史回顾
   ├── 日历视图 + 日期列表
   ├── 动作详情（表格 + 折线图 + 1RM）
   └── 策略提示（自由文本，计划页显示上次）
   ↓ 有了"怎么改进"
M6 备份与体验
   ├── 导出/导入（JSON）
   ├── CSV/JSON 导出
   └── PWA（手机添加到主屏幕）
   ↓ 数据安全 + 体验
M7 打磨
   ├── 界面美化（CSS）
   ├── 响应式（手机友好）
   └── 错误处理完善
```

---

## 四、你现在该做什么

M0 已完成，**下一步是 M1 认证**。但先别急着写代码，建议：

### 1. 读懂 M0 的代码（半天）
打开 `src/main.rs`，从【教学注释】最多的文件开始，顺着 `main()` 的执行流走一遍：
`main → AppConfig::from_env() → db::init_pool() → Router::new() → axum::serve()`
每个【教学】注释都是一个知识点，看完 `main.rs` 再看 `db.rs`，最后看 `error.rs`。

### 2. 动手改点东西（检验理解）
试着做这几个小改动，能编译通过就说明你懂了：
- 改首页欢迎文字
- 在首页多显示一个数：`SELECT COUNT(*) FROM phases` 的阶段数量
- 把端口默认值从 8080 改成 3000（在 config.rs）

### 3. 开始 M1 前先读 design 相关章节
打开 `docs/structure.md`，重点看：
- §3 数据库设计里 `users` 表
- §5 功能规格里"认证与权限"一节
- §8 开发计划里 M1 的任务分解

### 4. 开工 M1 的顺序（到时候我会带你写）
```
第1步  Cargo.toml       添加 askama（模板）+ cookie 相关库
第2步  migrations/0002_auth.sql    users 表补充字段（如果需要）
第3步  src/auth.rs      密码哈希 + session 创建/校验
第4步  templates/login.html  登录页
第5步  main.rs          注册登录/注册路由
第6步  src/middleware.rs  登录检查（没登录跳转到 /login）
```

---

## 五、常见坑（写代码时注意）

| 坑 | 解决办法 |
|---|---|
| `port` 变量不存在 | 用 `config.port`，配置在 config.rs 里 |
| `.parse()` 报类型不明确 | 显式标注类型：`let addr: SocketAddr = ...` |
| struct 没实现 Clone | 加 `#[derive(Clone)]` |
| sqlx 查询需要类型标注 | 写 `let n: i64 = ...` |
| 大括号换行 | 项目约定：函数体左大括号换行（Allman 风格） |

---

## 六、学习路径（技术栈 → 你需要的知识）

| 技术 | 你现在只需要知道 |
|---|---|
| Rust 基础 | struct/impl、Result/?、match、迭代器 |
| tokio | 只要会用 `#[tokio::main]` 和 `.await` |
| axum | Router、handler、提取器（State/Path/Form） |
| sqlx | query/query_scalar/fetch_one、FromRow |
| askama | 模板语法 `{{ }}`、`{% if %}`（M1 学） |
| argon2 | 密码哈希（M1 学） |

> 记住一条心法：**遇到不懂的，先照着【教学注释】抄一遍，跑通了再问为什么。**
> 抄三遍，自然就懂了。
