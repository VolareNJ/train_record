-- ============================================================
-- 0001_init.sql —— 初始数据库结构
-- ============================================================
-- 【教学说明】
-- 这是 sqlx 迁移文件。文件名规则：序号_描述.sql（如 0001_init.sql）
-- sqlx 启动时会按序号顺序执行，并把执行记录存到 _sqlx_migrations 表，
-- 已执行的不会重复执行（幂等）。
--
-- SQLite 类型：INTEGER(整数) TEXT(文本) REAL(浮点)
-- 【教学：IF NOT EXISTS】确保表不存在时才创建，防止重复执行报错
-- ============================================================

-- 用户表
CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,  -- 自增主键
    username      TEXT NOT NULL UNIQUE,               -- 登录名，唯一
    password_hash TEXT NOT NULL,                      -- argon2 哈希
    display_name  TEXT NOT NULL DEFAULT '',           -- 显示名
    is_admin      INTEGER NOT NULL DEFAULT 0,         -- 0=普通 1=管理员
    created_at    TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 训练阶段表
-- 【教学】PRIMARY KEY = 主键（唯一标识一行）；AUTOINCREMENT = 自动递增
-- FOREIGN KEY = 外键，关联 users 表；REFERENCES = 引用哪张表的哪列
CREATE TABLE IF NOT EXISTS phases (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL REFERENCES users(id), -- 属于哪个用户
    name       TEXT NOT NULL,                         -- 阶段名
    note       TEXT NOT NULL DEFAULT '',              -- 备注
    start_date TEXT,                                  -- 开始日期(可空)
    archived   INTEGER NOT NULL DEFAULT 0,            -- 0=进行中 1=已归档
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- 动作库表
CREATE TABLE IF NOT EXISTS exercises (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES users(id),
    name         TEXT NOT NULL,                       -- 动作名
    body_part    TEXT NOT NULL DEFAULT '',            -- 部位分组
    default_mode TEXT NOT NULL DEFAULT 'bar',         -- 默认模式
    bar_weight   REAL NOT NULL DEFAULT 20.0,          -- 默认杆重
    default_sets INTEGER NOT NULL DEFAULT 3,          -- 默认组数
    default_reps INTEGER NOT NULL DEFAULT 8,          -- 默认次数
    key_points   TEXT NOT NULL DEFAULT '',            -- 要领
    created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE(user_id, name)                             -- 同用户下动作名唯一
);

-- 训练模板表（绑定阶段）
CREATE TABLE IF NOT EXISTS templates (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    phase_id   INTEGER NOT NULL REFERENCES phases(id),
    name       TEXT NOT NULL,                         -- 模板名
    sort_order INTEGER NOT NULL DEFAULT 0             -- 排序
);

-- 模板动作项
CREATE TABLE IF NOT EXISTS template_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    template_id INTEGER NOT NULL REFERENCES templates(id),
    exercise_id INTEGER NOT NULL REFERENCES exercises(id),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    plan_sets   INTEGER,                              -- 可空=用默认
    plan_reps   INTEGER
);

-- 当日计划表
CREATE TABLE IF NOT EXISTS plans (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    phase_id   INTEGER NOT NULL REFERENCES phases(id),
    date       TEXT NOT NULL,                         -- 'YYYY-MM-DD'
    note       TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now', 'localtime')),
    UNIQUE(phase_id, date)                            -- 同阶段同日期只有一个计划
);

-- 计划动作项
CREATE TABLE IF NOT EXISTS plan_items (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_id     INTEGER NOT NULL REFERENCES plans(id),
    exercise_id INTEGER NOT NULL REFERENCES exercises(id),
    sort_order  INTEGER NOT NULL DEFAULT 0,
    plan_sets   INTEGER,
    plan_reps   INTEGER,
    plan_weight REAL
);

-- 训练记录表
CREATE TABLE IF NOT EXISTS records (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    plan_item_id INTEGER REFERENCES plan_items(id),   -- 可空：非计划录入
    phase_id     INTEGER NOT NULL REFERENCES phases(id),
    exercise_id  INTEGER NOT NULL REFERENCES exercises(id),
    record_date  TEXT NOT NULL,                       -- 'YYYY-MM-DD'
    weight       REAL NOT NULL,                       -- 实际总重 kg
    sets         INTEGER NOT NULL,
    reps         INTEGER NOT NULL,
    rest         INTEGER NOT NULL DEFAULT 0,          -- 休息秒
    feeling      TEXT NOT NULL DEFAULT '',            -- 感受
    strategy     TEXT NOT NULL DEFAULT '',            -- 策略
    key_points   TEXT NOT NULL DEFAULT '',            -- 要领
    mode         TEXT NOT NULL DEFAULT 'bar',         -- 录入模式
    created_at   TEXT NOT NULL DEFAULT (datetime('now', 'localtime'))
);

-- ============================================================
-- 【教学：索引(INDEX)】
-- 索引 = 书的目录，让按某列查询变快。
-- 我们经常按 phase_id / exercise_id / record_date 查询，
-- 所以给它们建索引。数据量大时才明显，但提前建好无坏处。
-- ============================================================
CREATE INDEX IF NOT EXISTS idx_records_phase    ON records(phase_id);
CREATE INDEX IF NOT EXISTS idx_records_exercise ON records(exercise_id);
CREATE INDEX IF NOT EXISTS idx_records_date     ON records(record_date);
CREATE INDEX IF NOT EXISTS idx_plans_phase      ON plans(phase_id);
