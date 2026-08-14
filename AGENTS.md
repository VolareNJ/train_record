---
description: Describe when these instructions should be loaded by the agent based on task context
# applyTo: 'Describe when these instructions should be loaded by the agent based on task context' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---

<!-- Tip: Use /create-instructions in chat to generate content with agent assistance -->

当前是一个Ubuntu 24.04环境
请使用Rust语言开发，多使用函数式，多使用迭代器和适配器，大括号换行
禁止使用unsafe

## 构建约定

- **每次 build 前先检查编译缓存大小**：`du -sm target | cut -f1`
  - 超过 5GB → `cargo clean` 后再 `cargo build` / `cargo run`
  - 未超过 → 直接增量编译（省时间）
  原因：云服务器磁盘空间有限，增量编译会累积大量 target 缓存，
  clean 后重新编译可释放磁盘空间（代价是编译时间变长），
  所以只在缓存膨胀时 clean，避免每次都全量重编
- 验证命令顺序：`cargo +nightly fmt --check` → `cargo check` → `cargo test`
- 格式不符合时用 `cargo +nightly fmt` 自动修正

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

### 事务纪律

- 写多张表的 handler 必须 `begin()` + `commit()`，**遗漏 commit 会全部回滚**
  （数据静默丢失，页面却显示成功——最难排查的 bug）
- 事务示例：`let mut tx = state.pool.begin().await?;` → 所有 `execute` 用 `&mut *tx` → `tx.commit().await?;`

### 数据隔离

- 所有按 id 查询必须带 user_id 条件：`WHERE id = ? AND user_id = ?`，不能只按 phase_id/模板 id 查

### 表单多选（checkbox）陷阱

- axum 的 `Form<T>` 用 `serde_urlencoded` 解析（**map 语义**）：
  重复键后值覆盖前值，`Vec<i64>` 会 422，`[]` 后缀也不生效
- ✅ 正确模式：checkbox `name` = 动作 id（唯一键）、`value="1"`，
  结构体 `#[serde(flatten)]` 收进 `HashMap<String, String>`，handler 按数字键过滤
- 详细踩坑记录见 `docs/todo.md` §2.1

### 字段约定

- `template_items.sort_order` / `plan_items.sort_order`：**实际字段**（`enumerate()` 生成，决定动作顺序）
- `templates.sort_order`：**预留字段**（暂恒为 0），模板间排序是未来待办 → `docs/todo.md` §1.1

### 前端文案约定

- 组数×次数等**乘号统一用 ASCII `*`**（如 `2组 * 15次`），
  不用 `×`（U+00D7）——部分设备/字体渲染不一致
- 每个 HTML 页面**必须带移动端 viewport head**（手机浏览器训练场景）：
  `<head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>`
  拼 HTML 时第一行就写，别忘