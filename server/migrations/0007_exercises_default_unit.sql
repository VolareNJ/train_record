-- M5 修订：动作默认计重单位（kg/lb）
-- 只影响"观测强度"下拉框的预填与展示串单位，不影响实际强度（实际强度始终存 kg）。
ALTER TABLE exercises ADD COLUMN default_unit TEXT NOT NULL DEFAULT 'kg';
