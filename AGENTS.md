---
description: Describe when these instructions should be loaded by the agent based on task context
# applyTo: 'Describe when these instructions should be loaded by the agent based on task context' # when provided, instructions will automatically be added to the request context when the pattern matches an attached file
---

<!-- Tip: Use /create-instructions in chat to generate content with agent assistance -->

当前是一个Ubuntu 24.04环境
请使用Rust语言开发，多使用函数式，多使用迭代器和适配器，大括号换行
禁止使用unsafe

## 协作模式与教学约定

- 学习模式：老师（Copilot）写定义与教学注释，学生（用户）写实现
- 每个阶段验收包含【理解验证】，统一使用**填空题**形式（不提供选项），
  避免选择题的"答案提示效应"无法检验真实理解
- 填空答案需学生独立写出后，再对照代码或参考答案检查