# 📌 待办与设计决策记录（跨会话）

> 本文件记录**已确认但尚未解决**的设计事项与踩坑记录，每条标注计划在哪个阶段（M 几）解决。
> 用途：Copilot 有上下文长度限制，重要事项写在这里，跨会话可查。
> 约定类内容（如事务纪律、数据隔离）留在 `AGENTS.md`（每次会话自动加载），本文件只放"未来要做的事"。

---

## 一、待解决事项（按优先级）

### 1.1 模板间排序：`templates.sort_order` 真值分配 🔴

- **现状**：`templates.sort_order` 是预留字段，M3 阶段插入时恒为 `0`（占位）。
  M3 的模板列表查询**没有** `ORDER BY sort_order`，不影响功能。
- **计划解决**：**M5 / M7 打磨阶段**
- **方案候选**：
  - 创建时 `MAX(sort_order) + 1` 分配新值
  - 或界面拖拽排序后批量回写
- **注意**：在此之前**不要依赖** `templates.sort_order` 的值，
  也不要因为"全是 0"而改动它。子表 `template_items.sort_order` / `plan_items.sort_order` 从一开始就是真数据（`enumerate()` 生成）。

### 1.2 模板/计划"空动作"校验 ⚠️

- **现状**：创建模板/计划时，用户一个动作都不勾选，`exercise_ids()` 返回空 Vec，
  循环插入 0 次 → 生成一个空壳模板/空计划。
- **计划解决**：**M5 打磨阶段**（M3 先允许，可后续补删）
- **方案候选**：表单校验 `exercise_ids()` 非空，空则返回 400 提示"至少选一个动作"

### 1.3 HashMap 迭代顺序 ≠ 勾选顺序 🟡

- **现状**：`TemplateCreateForm` 用 `#[serde(flatten)]` + `HashMap` 收集勾选的动作 id，
  `HashMap` 迭代顺序**不保证**是表单提交顺序（实测勾选 6→7，落库 sort_order 是 7=0、6=1）。
- **影响**：sort_order 仍连续（0,1,2…），列表按 sort_order 展示顺序正常；
  但"用户勾选先后 = 动作先后"的语义丢失。
- **计划解决**：**M3 编辑页兜底**（编辑时按展示顺序重新分配 sort_order）；
  若需严格勾选顺序，改为 checkbox name 带序号（如 `exercise_ids_0`）或前端排序。

---

## 二、踩坑记录（已解决，供参考）

### 2.1 serde_urlencoded 多选陷阱（M3 第 1 步，已解决 ✅）

- **坑**：axum 的 `Form<T>` 用 `serde_urlencoded` 解析，它是 **map 语义**：
  - 重复键 `exercise_ids=6&exercise_ids=7` → **后值覆盖前值**（只剩 7）
  - `Vec<i64>` 字段 → 实测 422：`invalid type: string "6", expected a sequence`
  - `exercise_ids[]=6&exercise_ids[]=7`（[] 后缀）→ **同样 422**，[] 后缀不生效
- **解法（本项目采用）**：checkbox 的 `name` = 动作 id（唯一键），`value="1"`（勾选标记）；
  结构体 `#[serde(flatten)]` 收进 `HashMap<String, String>`，handler 按"能 parse 成 i64 的键"过滤。
  表单提交形如：`name=推日&6=1&7=1`
- **影响范围**：**M3 第 2 步（当日计划：手动选动作）** 会再遇到，直接复用同一模式。
  M3 第 3 步（从模板复制）不涉及表单多选，不受影响。

### 2.2 事务必须 commit（M3 第 1 步，已解决 ✅）

- **坑**：写多张表（templates + template_items）时 `begin()` 后漏了 `commit()`，
  函数结束 `tx` drop → 全部回滚，数据静默丢失（页面却显示成功）。
- **解法**：`begin()` → 所有 `execute` 用 `&mut *tx` → 结尾 `tx.commit().await?;`

---

## 三、测试数据（M3 阶段 curl 实测用）

- 阶段：`phase id=3`（M3test，start_date=2026-08-08）
- 动作：`exercise id=6`（bench_press）、`id=7`（squat）
- 登录：`admin / admin123`；curl 需 `-c /tmp/ck.txt -b /tmp/ck.txt`
- 服务器：端口 8080；重启后需重新登录再实测
