-- ============================================================
-- 0002_sessions.sql —— 会话表（M1 认证）
-- ============================================================
-- 【教学说明】
-- 这张表存"登录状态"：谁登录了、通行证编号是什么、什么时候过期。
-- 登录成功 → 插入一行；登出 → 删除一行；过期 → 验证失败。
--
-- 迁移机制回顾（M0 第八节）：
--   sqlx 检查 _sqlx_migrations 表，0001 已执行过 → 跳过；
--   只执行本文件 0002 → 老数据库自动升级，无需删库。
-- ============================================================

CREATE TABLE IF NOT EXISTS sessions (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL REFERENCES users(id), -- 属于哪个用户
    token      TEXT NOT NULL UNIQUE,                  -- 随机通行证编号（uuid）
    expires_at TEXT NOT NULL,                         -- 过期时间（RFC3339）
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 【教学：索引】
-- 每次请求都要按 token 查这张表（WHERE token = ?），
-- 数据多了以后全表扫描很慢。给 token 建索引 = 给字典建拼音检字表，
-- 查询从"翻遍全书"变成"直接翻到那一页"。
-- 其实 UNIQUE 约束已经自动建了索引，这里显式说明这个意图。
-- 真正的性能提升在 M3+ 用户多了以后体现。
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token);
