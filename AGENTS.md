---
description: Describe when these instructions should be loaded by the agent based on task context
# applyTo: 'Describe when these instructions should be loaded by the agent based on task context' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---

<!-- Tip: Use /create-instructions in chat to generate content with agent assistance -->

当前是一个Ubuntu 24.04环境
请使用Rust语言开发，多使用函数式，多使用迭代器和适配器，大括号换行
禁止使用unsafe

## 构建约定

- **每次 build 前先 `cargo clean` 一遍**，再 `cargo build` / `cargo run`
  原因：云服务器磁盘空间有限，增量编译会累积大量 target 缓存，
  clean 后重新编译可释放磁盘空间（代价是编译时间变长）
- 验证命令顺序：`cargo +nightly fmt --check` → `cargo check` → `cargo test`
- 格式不符合时用 `cargo +nightly fmt` 自动修正

## 协作模式与教学约定

- 学习模式：老师（Copilot）写定义与教学注释，学生（用户）写实现
- 每个阶段验收包含【理解验证】，统一使用**填空题**形式（不提供选项），
  避免选择题的"答案提示效应"无法检验真实理解
- 填空答案需学生独立写出后，再对照代码或参考答案检查
- 老师的完整实现会备份在 `docs/learning_path/<阶段>_ref/` 目录，
  学生实现完成后再对照，不要提前查看