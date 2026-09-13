---
description: Describe when these instructions should be loaded by the agent based on task context
# applyTo: 'Describe when these instructions should be loaded by the agent based on task context' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---

<!-- Tip: Use /create-instructions in chat to generate content with agent assistance -->

当前是一个Ubuntu 24.04环境
请使用Rust语言开发，多使用函数式，多使用迭代器和适配器，大括号换行
禁止使用unsafe
尽量不要使用clone/dyn trait，避免不必要的运行时开销

## 背景（每次对话前必读）

You are a helpful software engineer assistant. When you thought, thought in ENGLISH, start with "We need..."

## 构建约定

- **工具链：默认 stable**（项目不锁工具链、不用 nightly 编译）
  - `cargo build/check/test/run` 全部走 stable（本机 rustup default）
  - **只有格式化需要 nightly**：`cargo +nightly fmt`（rustfmt.toml 里的
    `brace_style` 等选项是 unstable 特性，stable rustfmt 会忽略它们）
  - 所以命令一律写 `+nightly fmt`，**不要**用裸 `cargo fmt`
- **每次 build 前先检查编译缓存大小**：`du -sm target | cut -f1`
  - 超过 5GB → `cargo clean` 后再 `cargo build` / `cargo run`
  - 未超过 → 直接增量编译（省时间）
  原因：云服务器磁盘空间有限，增量编译会累积大量 target 缓存，
  clean 后重新编译可释放磁盘空间（代价是编译时间变长），
  所以只在缓存膨胀时 clean，避免每次都全量重编
- 验证命令顺序：`cargo +nightly fmt --check` → `cargo check` → `cargo test`
- 格式不符合时用 `cargo +nightly fmt` 自动修正

### 0 warning 纪律（M9 起，硬指标）

- **交付前必须 0 警告**：`cargo check --all-targets` 与 `cargo test` 的输出里
  任何一个 `warning:` 都不允许（包括测试目标、未使用变量、未使用类型……）。
  自查：`cargo check --all-targets 2>&1 | grep -c "^warning"` → 必须是 0
  （`cargo test` 同理）。
- **唯一例外：挖空练习的未实现代码**（教学场景）：
  `todo!()` / `unimplemented!()` 占位时，参数/返回值确实用不到，可以先加
  `#[allow(unused)]` / `#[allow(unused_variables)]` 顶住警告，但：
  1. 必须紧贴该函数写注释说明“挖空期间允许，实现后删”
  2. **实现完成后要删掉这些 allow**，并确认警告仍然为 0
    （否则 allow 会把真正的警告永久掩盖——这是最隐蔽的债）
  3. 已实现的方法/函数一律不允许保留 allow
- 新增代码前先看基线：改之前跑一次 `cargo check`，改之后对比，
  不能“能跑就行，警告留着”。
- clippy（Zed 保存时检查用的就是它）：**新增代码不得引入新的 clippy 警告**。
  编辑器配置在 `.zed/settings.json`（原 `.vscode/settings.json` 已删：Zed 不读它）。
  现状基线是 0 条（M9 收尾清完 347 条，历史见 `docs/todo.md` §1.7）。
  对比方法：`cargo clippy --all-targets 2>&1 | grep -c "^warning"` → 必须是 0。

## 协作模式与教学约定

- 学习模式：老师（Copilot）写定义与教学注释，学生（用户）写实现
- 每个阶段验收包含【理解验证】，统一使用**填空题**形式（不提供选项），
  避免选择题的"答案提示效应"无法检验真实理解
- 填空答案需学生独立写出后，再对照代码或参考答案检查
- 老师的完整实现会备份在 `docs/learning_path/<阶段>_ref/` 目录，
  学生实现完成后再对照，不要提前查看

## 跨会话备忘

- **待办与设计决策记录** → `docs/todo.md`（每条标注未来哪个 M 解决）
- **设计稿** → `docs/structure.md`；**阶段指南** → `docs/learning_path/<M>.md`

## 代码约定

### 路径与导入（M9 起，全路径优先）

- **所有类型/函数都写全路径**，不用 `use` 做便利导入：
   `sqlx::query_as::<_, crate::models::User>(...)`、`axum::extract::State<crate::AppState>`
   `crate::api::rest::records::record_upsert(...)`
   文件头 `use sqlx::{query_as, SqlitePool};` + 正文写短名
  理由三条：① 读代码不必回文件头查“这个名字是哪来的”；
  ② 新增/移动文件不牵动一整块 import（M8→M9 目录搬迁时就吃过这个苦）；
  ③ 避免“局部变量名与导入名撞车”（迁移时真实踩到：`let header = ...` 撞 `use axum::http::header`）
- **唯一例外：trait 方法调用**——trait 必须在作用域里，按“最小引用”原则
  **一个 trait 一行**（不写 `use xxx::*`，不顺手多导）：
  `use axum::response::IntoResponse;`（为了 `.into_response()`）
  `use argon2::PasswordHasher;`（为了 `.hash_password()`）
  `use sqlx::ConnectOptions;`（为了 `.log_statements()`）
- 能不用导入就不导入：需要 `std::str::FromStr` 的写法可换成 str 自带的
  `"...".parse::<SqliteConnectOptions>()`（内部就是 FromStr，零导入）
- 检查命令（应只剩个位数的 trait 导入）：`grep -rn "^use " src/`
- 全路径下不再需要“兼容转发”：`pub use rest::ApiError;` 已删除，
  调用方直接写 `crate::api::rest::ApiError`（一条路径一个出处）

### 禁止 emoji（M9 起）

- **代码、注释、文档、提交信息里一律不用 emoji**
  （包括 U+2705 对勾、U+274C 叉、U+26A0 警告符、U+1F4CC 图钉、
  以及 U+2605 五角星、U+2713/U+2717 勾叉这类“看起来像符号”的字符）
  为什么：① 终端/编辑器/字体对 emoji 宽度与变体选择符（U+FE0F）处理不一致，
  排版容易错位；② grep/日志里搜不到、不好自动化；③ 纯文本表达更精确。
  连“举例”也不要直接写字符（否则自查命令会命中本条文档）——写码点即可。
- 替代写法：
  状态用文字（“已完成 / 待实现 / 未开始”）、清单用 `- [x]` / `- [ ]`、
  重点用“注意： / 重要：”、等级用 `1. 2. 3.`。
- 自查（**不含 migrations/**，原因见下）：
  `grep -rlP "[\x{1F300}-\x{1FAFF}\x{2600}-\x{27BF}]" src docs README.md AGENTS.md proto build.rs static sw.js` 应为空。
- **例外：`migrations/*.sql` 不要改**（哪怕只是改注释/去 emoji）：
  sqlx 会校验迁移文件的 checksum，**已执行过的迁移被改动后，
  下一次启动会直接报错**（previously applied but has been modified）。
  迁移文件一旦提交就视为**不可变**：要改表结构就新增迁移文件；
  注释写错了也只能在后续迁移里补说明（或保留原样）。
  （M9 清理时误改过 4 个迁移文件，已回退——见 `docs/todo.md` §2.5）

### 模块树（M9 起 lib + bin 分离）

- **模块树在 `src/lib.rs`**（`pub mod api/auth/calc/config/db/error/handlers/models/page;`），
  共享状态 `AppState` 也定义在那里；`src/main.rs` 只负责“读配置 → 起服务器 → 组装路由”。
- bin 里引用库里的东西要**以包名开头**：`train_record::api::rest::router()`、`train_record::AppState`
  （bin 是另一个编译目标，`crate::` 会指向 bin 自己）。
- 好处：测试/示例/未来其他 crate 都能引用应用本体；`cargo test` 会分别跑 lib 与 bin 的测试。

### 事务纪律

- 写多张表的 handler 必须 `begin()` + `commit()`，**遗漏 commit 会全部回滚**
  （数据静默丢失，页面却显示成功——最难排查的 bug）
- 事务示例：`let mut tx = state.pool.begin().await?;` → 所有 `execute` 用 `&mut *tx` → `tx.commit().await?;`

### 数据隔离

- 所有按 id 查询必须带 user_id 条件：`WHERE id = ? AND user_id = ?`，不能只按 phase_id/模板 id 查

### 表单多选（checkbox）陷阱

- axum 的 `Form<T>` 用 `serde_urlencoded` 解析（**map 语义**）：
  重复键后值覆盖前值，`Vec<i64>` 会 422，`[]` 后缀也不生效
- 正确模式：checkbox `name` = 动作 id（唯一键）、`value="1"`，
  结构体 `#[serde(flatten)]` 收进 `HashMap<String, String>`，handler 按数字键过滤
- 详细踩坑记录见 `docs/todo.md` §2.1

### 字段约定

- `template_items.sort_order` / `plan_items.sort_order`：**实际字段**（`enumerate()` 生成，决定动作顺序）
- `templates.sort_order`：**预留字段**（暂恒为 0），模板间排序是未来待办 → `docs/todo.md` §1.1

### 前端文案约定

- 组数×次数等**乘号统一用 ASCII `*`，且不带空格**（如 `3*8`），
  不用 `×`（U+00D7）——部分设备/字体渲染不一致
- 每个 HTML 页面**必须带移动端 viewport head**（手机浏览器训练场景）：
  `<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>`
  拼 HTML 时第一行就写，别忘