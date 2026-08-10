-- ============================================================
-- 0003_plan_items_metadata.sql —— 计划项补计重元数据列
-- ============================================================
-- 【教学说明】
-- 需求：编辑计划时就规定好训练计重方式（同 record_form），
--   训练当天 record_form 首先按 plan_item 预填，不用再手设。
--   这样"预设计重信息"发生在计划层，不会产生 records 记录，
--   不会误标"已训练"（已训练判定 = records 表有记录）。
--
-- 只允许在 record_form 填的（感受 feeling / 下次策略 strategy）
-- 不在这张表里 —— 它们属于"训练完的反思"，不属于"训练前的安排"。
--
-- ⚠️ SQLite ALTER TABLE ADD COLUMN 的注意点：
--   已有行的新列值 = NULL（无法加 NOT NULL 默认值约束的旧行）。
--   所以 Rust 侧 PlanItem 新字段必须 Option<T>，
--   否则旧计划项反序列化直接崩（NULL → 非 Option 类型报错）。
--
-- 迁移机制回顾：sqlx 检查 _sqlx_migrations 表，0001/0002 已执行过 → 跳过；
--   只执行本文件 0003 → 老数据库自动升级，无需删库。
-- ============================================================

-- 计重方式（bar/support/std/lb2kg，与 records.mode 同义；空 = 未预设）
ALTER TABLE plan_items ADD COLUMN plan_mode TEXT;

-- 杆重规格（Olympic 20 / Smith 11.3 / 短杠 10 / 双边 0；空 = 用动作默认）
ALTER TABLE plan_items ADD COLUMN plan_bar_weight REAL;

-- 计划休息秒（空 = 未预设，record_form 留空让用户填）
ALTER TABLE plan_items ADD COLUMN plan_rest INTEGER;

-- 计划要领（可选；空 = record_form 落回动作库 key_points）
ALTER TABLE plan_items ADD COLUMN plan_key_points TEXT;
