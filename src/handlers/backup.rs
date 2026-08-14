// ============================================================
// handlers/backup.rs —— 数据备份（导出/导入 + CSV/JSON 导出）
// ============================================================
// 【教学说明】
// 这个文件是 M6 的核心：让训练数据"丢不了、换设备也用得上"。
// 四个 handler 对应四个能力：
//   GET  /admin/backup                    → 备份管理页（backup_page）
//   GET  /admin/backup/download           → 下载数据库文件（backup_download）
//   POST /admin/backup/upload             → 上传 .db 恢复（backup_upload）
//   GET  /admin/backup/export?format=...  → 导出记录 CSV/JSON（export_records）
//
// 全部是【管理员专属】（users 管理同款：handler 内部检查 user.is_admin）。
//
// 📌 阶段要求：M6 你来实现本文件所有函数。
//   实现完成后对照检查（完整实现备份在 docs/learning_path/M6_ref/）。
//
// ⚠️ 接线提醒（本文件写完后）：
//   1. src/handlers/mod.rs 加一行：pub mod backup;
//   2. Cargo.toml：axum features 加 "multipart"（上传文件用）
//   3. src/main.rs 注册 4 条路由 + home 账户区加备份入口
//   4. static/manifest.json + static/sw.js（PWA，老师已写好骨架）
// ============================================================

// 【教学：本文件用到的导入 —— 比之前多了这些】
//   - Multipart：文件上传提取器（axum feature "multipart"）
//   - HeaderMap：构造响应头（Content-Disposition 下载文件）
//   - tokio::fs：异步文件读写（读 .db 文件字节 / 写临时文件）
use axum::{
    extract::{Multipart, Query, State},
    http::{HeaderMap, header},
    response::{Html, Response},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{AppState, error::AppError, handlers::auth::AuthUser};

// ============================================================
// 【教学：M6 核心认知 —— SQLite 单文件 = 备份就是复制】
// ============================================================
// 一个 SQLite 数据库 = 一个文件（train_record.db），所有表都在里面。
// 所以：
//   备份 = 读文件字节 → 发给浏览器下载（backup_download）
//   恢复 = 收上传字节 → 写回数据库文件（backup_upload）
// 不需要任何 SQL 导出语句——文件本身就是完整备份。
//
// 三个安全要点：
//   1. 管理员专属：备份页能下载全部数据，必须 is_admin
//   2. 覆盖前先备份：上传恢复时先把当前库改名为 .bak-时间戳
//      （和部署时手动备份同款，出错了能回退）
//   3. 魔数校验：SQLite 文件前 16 字节固定是 "SQLite format 3\0"，
//      上传的不是数据库文件 → 拒绝（防垃圾文件损坏库）

// ============================================================
// 第一部分：备份管理页（GET /admin/backup）
// ============================================================
/// 备份管理页：下载备份 + 上传恢复 + 导出 CSV/JSON 三个入口
///
/// 【教学：和 admin_users 同款守卫】
/// 非管理员 → Forbidden。页面只是几个按钮/表单，逻辑很简单。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser
/// 2. is_admin 检查（非管理员 → Forbidden 403）
/// 3. 拼 HTML：三个区块
///    - 下载备份：<a href="/admin/backup/download">（GET 链接即可，
///      下载是只读操作，不需要 POST 表单）
///    - 上传恢复：<form method="post" enctype="multipart/form-data">
///      ⚠️ 上传文件必须 enctype="multipart/form-data"！
///      默认表单编码（urlencoded）只传键值对，传不了文件字节。
///      <input type="file" name="db_file" accept=".db">
///    - 导出记录：两个链接（?format=csv / ?format=json）
/// 4. 返回首页链接
pub async fn backup_page(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Html<String>, AppError>
{
    todo!("M6 第 1 步：备份管理页（管理员守卫 + 下载/上传/导出入口）")
}

// ============================================================
// 第二部分：下载数据库文件（GET /admin/backup/download）
// ============================================================
/// 下载完整数据库文件（.db）
///
/// 【教学：浏览器"下载"的本质 —— Content-Disposition 响应头】
/// 返回类型是 Response（不是 Html），要手动组装响应：
///   1. 读文件字节：tokio::fs::read(&state.config.database_path).await
///      → Vec<u8>（数据库二进制，不需要转字符串）
///   2. 响应头：Content-Disposition: attachment; filename="train_record_{日期}.db"
///      - attachment → 浏览器弹出"保存"对话框
///      - filename → 默认保存的文件名
///   3. (头, 字节) 转 Response：axum 对 (HeaderMap, Vec<u8>) 有 IntoResponse 实现
///
/// 【教学：日期从哪来？】
/// 和项目一贯纪律一致——用 SQLite 拿：
///   SELECT date('now', 'localtime')
/// 文件名形如 train_record_2026-08-15.db。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser
/// 2. is_admin 检查
/// 3. 查今天日期（SQLite localtime）
/// 4. 读数据库文件字节
/// 5. 组装 Content-Disposition 头 + 返回 (HeaderMap, bytes)
pub async fn backup_download(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Response, AppError>
{
    todo!("M6 第 1 步：下载数据库文件（Content-Disposition 头）")
}

// ============================================================
// 第三部分：上传恢复（POST /admin/backup/upload）
// ============================================================
/// 上传 .db 文件恢复数据库
///
/// 【教学：Multipart 上传 —— 文件怎么从浏览器到服务器】
/// <form enctype="multipart/form-data"> 提交时，文件内容被编码成
/// multipart 消息体（每个字段一段，文件段带 filename）。
/// axum 的 Multipart 提取器把它拆回字段：
///   while let Some(field) = multipart.next_field().await? { ... }
///   field.name()      → "db_file"（input 的 name 属性）
///   field.file_name() → 上传的文件名（浏览器给的）
///   field.bytes()     → 文件内容 Vec<u8>
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Multipart
/// 2. is_admin 检查
/// 3. 遍历字段找到 name == "db_file" 的，拿 file_name + bytes
///    （没有该字段 → Validation "没有收到文件"）
/// 4. 校验扩展名：file_name 以 ".db" 结尾（不区分大小写）
/// 5. 【魔数校验】前 16 字节 == b"SQLite format 3\0"
///    （不是合法 SQLite 文件 → Validation 拒绝）
/// 6. 先备份当前库：重命名 database_path → database_path.bak-{时间戳}
///    （时间戳：SQLite strftime('%Y%m%d-%H%M%S','now','localtime')）
/// 7. 写上传字节到 database_path（tokio::fs::write）
/// 8. 返回提示页："恢复成功，请重启服务生效"
///    ⚠️ 为什么重启？连接池还握着旧文件句柄，
///    直接覆盖会损坏库；重启 = 干净地重新打开新文件。
///    （热替换连接池需要 Arc/RwLock 包 AppState——M7 打磨项）
pub async fn backup_upload(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    mut multipart: Multipart,
) -> Result<Html<String>, AppError>
{
    todo!("M6 第 1 步：上传恢复（魔数校验 + 先备份再覆盖）")
}

// ============================================================
// 第四部分：导出 CSV/JSON（GET /admin/backup/export?format=...）
// ============================================================
/// 导出全部训练记录为 CSV 或 JSON（只读，不改库）
///
/// 【教学：Query 提取器 + format 参数（M2 exercises.rs 同款）】
/// /admin/backup/export?format=csv → query.format = Some("csv")
/// 不传 format → 默认 csv。
///
/// 【教学：CSV 手工拼写 —— 引号转义规则】
/// CSV 每行 = 逗号分隔的字段。字段值里有逗号/换行/引号时必须处理：
///   值用双引号包裹，内部的 " 翻倍成 ""
/// 教学版简化：拼一个转义辅助函数 escape_csv(s)：
///   if s.contains(',') || s.contains('"') || s.contains('\n')
///       → format!("\"{}\"", s.replace('"', "\"\""))
///   else → s
///
/// 【教学：JSON 序列化 —— serde_json 一步到位】
/// 查出来的行装进 Vec<serde_json::Value>（json! 宏构造），
/// serde_json::to_string(&rows) 自动转义、自动合法。
/// 不需要手工拼——这就是"机器生成合法 JSON"（M4_bugfix_notes §10.3）。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Query<ExportQuery>
/// 2. is_admin 检查
/// 3. 查全部记录 + 动作名：
///    SELECT r.*, e.name FROM records r
///    INNER JOIN exercises e ON r.exercise_id = e.id
///    WHERE e.user_id = ? ORDER BY r.record_date, r.id
/// 4. match format：
///    - "csv" → 手工拼 CSV（列名行 + 数据行），
///      Content-Type: text/csv; charset=utf-8
///    - "json" → serde_json 序列化，Content-Type: application/json
///    - 其他 → Validation "format 只支持 csv/json"
/// 5. 下载头：attachment; filename="records_{日期}.csv/.json"
/// 6. 返回 (HeaderMap, String)
pub async fn export_records(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<ExportQuery>,
) -> Result<Response, AppError>
{
    todo!("M6 第 1 步：CSV/JSON 导出（转义 + serde_json）")
}

// ============================================================
// 【教学：查询参数结构体 + CSV 转义辅助函数】
// ============================================================
/// 导出格式查询参数：?format=csv | ?format=json
#[derive(Deserialize)]
pub struct ExportQuery
{
    pub format: Option<String>,
}

/// CSV 字段转义：值含逗号/引号/换行 → 双引号包裹 + 内部引号翻倍
///
/// 【教学：这是纯函数（输入 String → 输出 String），可以写单元测试】
/// 测试用例：
///   escape_csv("abc")       == "abc"
///   escape_csv("a,b")       == "\"a,b\""
///   escape_csv("say \"hi\"") == "\"say \"\"hi\"\"\""
/// 实现：if 值包含 , 或 " 或 \n → 包裹 + replace，否则原样
fn escape_csv(s: &str) -> String
{
    todo!("M6 第 1 步：CSV 转义（纯函数，含单元测试）")
}
