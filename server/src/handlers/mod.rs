// ============================================================
// handlers/mod.rs —— 页面处理器模块入口
// ============================================================
// 【教学说明】
// 从 M1 开始，所有的"页面处理器"（handler）都放在这个目录下，
// 按功能分文件：
//   auth.rs    → 登录/登出/用户管理（M1）
//   phases.rs  → 阶段管理（M2）
//   exercises.rs → 动作库（M2）
//   plan.rs    → 模板 + 当日计划（M3）
//   ...
//
// 为什么 main.rs 里只写 mod handlers; 就能用 handlers::auth::xxx？
// 因为这里（mod.rs）是目录的"入口文件"，里面声明了子模块。
// 这就像一本书的目录页：先翻到目录，再找具体章节。
// ============================================================

pub mod auth;
pub mod backup;
pub mod exercises;
pub mod phases;
pub mod plan;
pub mod record;
pub mod stats;
