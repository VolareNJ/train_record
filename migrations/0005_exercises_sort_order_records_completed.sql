-- ============================================================
-- 0005_exercises_sort_order_records_completed.sql
--   M4 修订：动作库排序 + 记录"已完成"标记
-- ============================================================
-- 【M4 修订：需求来源】
--   1. exercises 加 sort_order：同一 body_part 内从 1 开始排序，
--      动作库列表按部位内顺序展示，可自由调整。
--   2. records 加 completed：只有勾选"已完成"的记录才在今日页
--      显示 ✅已完成（训练做到一半/未完成不算完成）。
--
-- ⚠️ SQLite ALTER TABLE ADD COLUMN 注意点：
--   已有行的新列自动填默认值（DEFAULT 0），无需手动回填，
--   所以 Rust 侧可以直接用非 Option 类型（i64 / bool）。
-- ============================================================

-- 动作库排序字段（同一 body_part 内从 1 开始）
ALTER TABLE exercises ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0;

-- 老数据初始化：同一 user_id + body_part 内按 id 升序编号（1, 2, 3…）
-- 相关子查询：数一数"同部位里 id <= 我"的动作有多少个，就是我的序号。
-- （SQLite 3.25+ 也支持窗口函数，但相关子查询更直白、兼容性最好。）
UPDATE exercises SET sort_order = (
    SELECT COUNT(*) FROM exercises e2
    WHERE e2.user_id = exercises.user_id
      AND e2.body_part = exercises.body_part
      AND e2.id <= exercises.id
);

-- 记录"已完成"标记（0=未完成 1=已完成；默认 0）
ALTER TABLE records ADD COLUMN completed INTEGER NOT NULL DEFAULT 0;
