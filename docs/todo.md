# 📌 待办与设计决策记录（跨会话）

> 本文件记录**已确认但尚未解决**的设计事项与踩坑记录，每条标注计划在哪个阶段（M 几）解决。
> 用途：Copilot 有上下文长度限制，重要事项写在这里，跨会话可查。
> 约定类内容（如事务纪律、数据隔离）留在 `AGENTS.md`（每次会话自动加载），本文件只放"未来要做的事"。

---

## 〇、阶段路线（2026-08-21 更新）

- ✅ M0-M5 全部完成（M5 含理解验证，2026-08-14 收官）
- ✅ M6 备份与体验（原计划：数据备份/导出）
- ✅ M7 打磨（热替换连接池/未登录跳转/排序真值/美化/PWA 离线/部署文档）
  （含理解验证，2026-08-21 收官；§1.1/§1.3/§1.4 三项待办已解决，见下）
- ✅ **M8 REST API 层**（新增）：给 train_record 加 JSON API，为 iced GUI 客户端铺路
  - 详见 `docs/learning_path/M5_roadmap_notes.md` §3 路线图与"GUI 技术栈决策"
  - 阶段文档 `docs/learning_path/M8.md`；完整实现已备份 `docs/learning_path/M8_ref/`
  - 认证方案：M8 复用 session cookie（ApiAuthUser 守卫），login 返回 `{"user", "token"}`；
    `Authorization: Bearer token` 头认证是扩展点（iced 客户端若需要再加）
  - 已实测：登录/登出/me、阶段/动作/模板/计划 CRUD、today/upsert/记录列表/更新/删除、
    history 日历/exercise stats、跨用户数据隔离（401/404）、未登录 401 JSON

---

## 一、待解决事项（按优先级）

### 1.1 模板间排序：`templates.sort_order` 真值分配 ✅（M7 第 3 步已解决，4d2231a 之前）

- **现状**：`templates.sort_order` 是预留字段，M3 阶段插入时恒为 `0`（占位）。
  M3 的模板列表查询**没有** `ORDER BY sort_order`，不影响功能。
- **解法（M7）**：创建时 `COALESCE(MAX(sort_order), -1) + 1` 分配新值（新模板排最后），
  列表查询加 `ORDER BY sort_order, id`。
- **注意**：`template_items.sort_order` / `plan_items.sort_order` 从一开始就是真数据（`enumerate()` 生成）。

### 1.2 模板/计划"空动作"校验 ✅（M5 第 6 步已解决，8d6434f 之前）

- **解法**：`template_create` / `template_update` / `plan_create` 三处
  在事务 begin 前校验：`exercise_ids().is_empty()` → 422"至少选择一个动作"
  （plan_create 只在未选模板时校验——模板自身已有校验）
- **实测**：curl 空模板/空计划均 422；选模板正常创建不误伤

### 1.3 HashMap 迭代顺序 ≠ 勾选顺序 ✅（M7 第 3 步已解决）

- **坑**：`TemplateCreateForm` 用 `#[serde(flatten)]` + `HashMap` 收集勾选的动作 id，
  `HashMap` 迭代顺序**不保证**是表单提交顺序（实测勾选 6→7，落库 sort_order 是 7=0、6=1）。
- **解法（M7）**：编辑页每行加隐藏序号 `order_{ex_id}`（渲染时=展示顺序），
  JS `addRow` 给新行赋"当前最大序号+1"；后端从 flatten HashMap 过滤 `order_` 前缀键
  按 order 排序后再 `enumerate()` 分配 sort_order（先删后插时用）。
- **效果**：编辑计划/模板打乱顺序保存后，刷新顺序与编辑时一致。

### 1.4 未登录访问返回 401 JSON 而非重定向到 /login ✅（M7 第 2 步已解决）

- **现状**：`AuthUser` 守卫失败返回 `AppError::Unauthorized` → error.rs 输出
  **401 JSON**（`{"error": "请先登录"}`），浏览器显示 JSON 文本，
  而不是 303 重定向到登录页。
- **解法（M7）**：error.rs 的 `IntoResponse` 里 Unauthorized → `Redirect::to("/login")`（302，全局统一跳转）。
- **注意**：M8 的 REST API 需要 401 JSON（程序要判断"没登录"），
  届时 API 用自己的 `ApiError` 类型，不受这里影响。

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

### 2.3 plan_update 先删后插导致记录关联断裂（已解决 ✅，78bfb2d）

- **坑**：编辑计划 = 先删后插（§2.1 外键策略之一：置 NULL 解除关联 → DELETE → 重新 INSERT）。
  重建的 plan_items 是**新 id**，但 records.plan_item_id 没重新挂回去 → 全变 NULL。
- **现象**：数据库里训练记录都在，但 today 页 / record_form（都按 plan_item_id 查）
  显示"未训练"；plan_detail 的"上次训练提示"按 exercise_id 查 → 正常。
- **解法**：解除关联前先备份 `(exercise_id → record_id 列表)`，
  重建后按备份清单精确还原关联（逐条 UPDATE，不依赖 JSON1 扩展）。
  生产数据已手动修复（今天的记录全部重新挂回）。
- **详细复盘**：`docs/learning_path/M4_bugfix_notes.md` §11

### 2.4 静态资源无 Cache-Control → 启发式缓存坑（M7 第 5 步，已解决 ✅）

- **坑**：ServeDir 响应不带 `Cache-Control` 头时，浏览器按
  `(now - Last-Modified) * 10%` 猜测新鲜度（启发式缓存）。
  实测：更新 manifest.json（加了 icons）后，浏览器 HTTP 缓存仍持有
  7 天前的旧文件；SW install 的 `addAll` 也命中这份陈旧响应 →
  新 SW 预缓存里还是旧 manifest（无 icons），PWA 图标验证不通过。
- **现象**：curl 服务器返回 460B 新 manifest，浏览器 fetch 返回 183B 旧版；
  `cache: 'no-store'` 无效（被 SW cache-first 拦截命中 SW 缓存）；
  CDP `Network.setCacheDisabled` 也不影响 SW install 的 fetch。
- **解法**：`.nest_service("/static", ServiceBuilder::new()
  .layer(SetResponseHeaderLayer::overriding(header::CACHE_CONTROL,
  HeaderValue::from_static("no-cache"))).service(ServeDir::new("static")))`
  → 浏览器每次带 ETag revalidate，文件变了才重新下载。
- **注意**：不要图快改 `max-age` 长缓存，会复发此坑。

---

## 三、测试数据（M3/M4 阶段 curl 实测用）

- 阶段：`phase id=3`（备赛期，start_date=2026-08-08）
- 动作：`exercise id=6`（bench_press）、`id=7`（squat）
- 计划：`plan id=7`（2026-08-10），计划项 `item id=16`（bench_press 4×8 60kg）、`17`（squat 5×5）
- 登录：`admin / admin123`；curl 需 `-c /tmp/ck.txt -b /tmp/ck.txt`
- 服务器：端口 8080；重启后需重新登录再实测

### 3.1 计划预设计重信息（已实现 ✅，commit b79b66d）

- **需求**：编辑计划时规定计重方式等信息（同 record_form），record_form 按 plan_item 预填；
  "感受/策略"仍只能在 record_form 填。解决"改 record_form 就标已训练"的矛盾。
- **实现**：
  - `plan_items` 新列：`plan_mode`/`plan_bar_weight`/`plan_rest`/`plan_key_points`（迁移 0003）
  - 编辑计划页每动作行：计重方式下拉 + 杆重下拉 + 休息 + 要领（前缀键 `mode_{id}`/`bar_weight_{id}`/`rest_{id}`/`key_points_{id}`）
  - record_form 预填链：**计划预设 → 最近记录 → 动作库默认**
  - 预设不产生 records 记录 → 不误标"已训练"
- **注意**：`plan_update` 是"先删后插"，编辑计划会重建 plan_item id（record_form URL 里的 item_id 会变）——目前编辑仅限当天未训练，符合 M3 限制。
- 记录页：GET `/plans/7/record/16`；保存 POST `/plans/7/record/16/save`
  （表单字段：weight/sets/reps/rest/feeling/strategy/key_points/mode，**缺字段会 422**）
