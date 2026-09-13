# train_record REST API 接口文档（v1）

> 面向桌面客户端开发。最后更新：2026-08-31，与 commit `59e0b98` 对应。
>
> 【M9 补充】本文件描述的是 **REST 出口**（`/api/v1`，实现于 `src/api/rest/`）。
> M9 另开了一个 **gRPC 出口**（`proto/train_record.proto`，实现于 `src/api/grpc/`，
> 默认端口 `GRPC_PORT=50051`）：两者共用同一套业务函数（rest 层的
> `pub(crate)` 共享函数），但契约形式不同（JSON vs protobuf、无流式 vs 四种流式）。
> 写客户端时二选一即可；需要流式（图表/导出/实时回执）就选 gRPC。

---

## 目录

1. [通用约定](#1-通用约定)
2. [认证 API](#2-认证-api)
3. [阶段 API](#3-阶段-api)
4. [动作库 API](#4-动作库-api)
5. [模板 + 计划 API](#5-模板--计划-api)
6. [记录 API](#6-记录-api)
7. [统计 API](#7-统计-api)
8. [iced 客户端接入建议](#8-iced-客户端接入建议)

---

## 1. 通用约定

### 1.1 Base URL 与协议

- 协议：HTTP（生产同域部署在 80 端口；本地开发默认 8080，见 `PORT` 环境变量）
- 前缀：所有端点都在 `/api/v1/` 下
- 编码：请求体 `Content-Type: application/json`，响应体 `application/json`；全部 UTF-8

### 1.2 认证方式（重要）

本项目认证走 **session cookie**，不是 Bearer token：

1. `POST /api/v1/login` 成功后：
   - 响应头 `Set-Cookie: session=<token>; Path=/; HttpOnly; SameSite=Lax; Max-Age=2592000`（30 天）
   - 响应体 JSON 里**冗余返回** `"token"` 字段（专为非浏览器客户端设计）
2. iced 客户端（reqwest 等）**不会自动存 cookie**，必须自己保存 token，
   之后每个请求手动带请求头：
   ```
   Cookie: session=<token>
   ```
3. 未登录 / session 失效 → `401 {"error": "未登录"}`。

> 说明：`Authorization: Bearer` 头认证留到 M9 再做（见 `docs/todo.md` 扩展点）。
> 当前所有 `ApiAuthUser` 守卫的端点都认 `Cookie: session=<token>`。

### 1.3 错误响应格式

所有错误统一 JSON：

```json
{ "error": "错误消息" }
```

| HTTP 状态码 | 含义 | 触发场景 |
|---|---|---|
| `400` | 参数不合法 | 名称空、日期格式错、负数、重复名等（消息可展示给用户） |
| `401` | 未登录 | 无 Cookie / session 失效 / 用户不存在 |
| `403` | 无权限 | 操作归档阶段（"归档阶段不可编辑"） |
| `404` | 资源不存在 | id 不存在或不属于当前用户 |
| `500` | 服务器内部错误 | 数据库错误（消息统一为"数据库错误"，细节只进日志） |

**iced 客户端建议**：`401` → 跳到登录页清空 token；`4xx` 其余 → 把 `error` 文案展示给用户；`5xx` → 提示"服务器开小差了"。

### 1.4 数据模型 ID 关系速览

```
phases (阶段) 1──N templates (模板) 1──N template_items (模板项)
phases (阶段) 1──N plans (计划)     1──N plan_items (计划项) 1──N records (训练记录)
exercises (动作库) 1──N records (训练记录，按 exercise_id)
```

- 所有列表/详情查询均按当前登录用户隔离（`WHERE ... user_id = ?`）
- 日期格式一律 `YYYY-MM-DD`（如 `2026-08-31`）
- 数字字段：JSON 直接 `f64`/`i64`（无页面层空串问题）

### 1.5 PATCH 语义

PATCH = 部分更新：**不传的字段保持旧值**（null 和缺字段都视为"不改"）。

---

## 2. 认证 API

### 2.1 登录 `POST /api/v1/login`

请求：

```json
{
  "username": "admin",
  "password": "admin123"
}
```

成功 `200`：

```json
{
  "user": {
    "id": 1,
    "username": "admin",
    "is_admin": true,
    "body_weight": 75.5
  },
  "token": "550e8400-e29b-41d4-a716-446655440000"
}
```

响应头同时带 `Set-Cookie`（浏览器可用）。iced 客户端用 `token` 字段即可。

失败：`400 {"error": "用户名不存在或密码错误"}`（用户名错和密码错**同一消息**，防枚举）。

### 2.2 登出 `POST /api/v1/logout`

- 无需登录（没 token 也返回成功）
- 请求体无；可带 `Cookie` 头（有则销毁对应 session）
- 成功 `200`：`{"ok": true}`，响应头 `Set-Cookie: session=; Max-Age=0`（清除 cookie）
- 客户端收到 200 后应**丢弃本地 token**

### 2.3 当前用户 `GET /api/v1/me`

- 需要登录
- 成功 `200`：`UserOut`（同 2.1 的 `user` 对象）
- 失败 `401 {"error": "未登录"}`

**用途**：iced 启动时用本地 token 调一次 `/me` 判断登录态是否有效。

### 2.4 `UserOut` 结构

| 字段 | 类型 | 说明 |
|---|---|---|
| `id` | i64 | 用户 id |
| `username` | String | 登录名 |
| `is_admin` | bool | 是否管理员 |
| `body_weight` | f64 \| null | 体重 kg（未设置 → null） |

---

## 3. 阶段 API

阶段是"时间容器"：计划、记录都挂在阶段下。归档 = 只读软删除。

### 3.1 阶段列表 `GET /api/v1/phases`

- 需要登录
- 排序：进行中在前、归档在后，各自按创建时间倒序
- 成功 `200`：`PhaseOut[]`

```json
[
  {
    "id": 3,
    "name": "增肌期",
    "note": "冬训增肌",
    "start_date": "2026-08-01",
    "archived": false,
    "days": 30
  }
]
```

`days` = 今天 − `start_date` 的自然日差（`start_date` 为 null → 0）。

### 3.2 创建阶段 `POST /api/v1/phases`

请求：

```json
{
  "name": "增肌期",
  "note": "冬训增肌",
  "start_date": "2026-08-01"
}
```

- `start_date` 可省略或 `null`（未设置）；传空串 `""` 也视为未设置
- 失败：`400` 名称空 / 重名（"阶段名已存在"）
- 成功 `200`：完整 `PhaseOut`（含新 id）

### 3.3 阶段详情 `GET /api/v1/phases/{id}`

- 成功 `200`：`PhaseOut`
- 失败 `404`（不存在或不属于当前用户）

### 3.4 更新阶段 `PATCH /api/v1/phases/{id}`

请求（全部可选，部分更新）：

```json
{
  "name": "新名字",
  "note": "新备注",
  "start_date": "2026-08-15"
}
```

- 不传字段保持旧值；`start_date` 传空串 → 清空
- 失败：`400` 名称空；`404` 不存在
- 成功 `200`：更新后的 `PhaseOut`

### 3.5 归档 `POST /api/v1/phases/{id}/archive`

- 成功 `200`：更新后的 `PhaseOut`（`archived: true`）
- 失败 `404`

### 3.6 启用 `POST /api/v1/phases/{id}/unarchive`

- 同上，`archived` 置回 `false`

> ⚠️ 归档阶段是只读的：其下模板/计划/记录的**写操作**会返回 `403 {"error": "归档阶段不可编辑"}`。

---

## 4. 动作库 API

动作库是"动作字典"，计划项、记录都引用 `exercise_id`。

### 4.1 动作列表 `GET /api/v1/exercises?body_part=胸`

- `body_part` 可选（空串视为不筛选）
- 排序：不筛选 → 按 `body_part, sort_order, id`；筛选 → 按 `sort_order, id`
- 成功 `200`：`ExerciseOut[]`

```json
[
  {
    "id": 6,
    "name": "平板杠铃卧推",
    "body_part": "胸",
    "default_mode": "bar",
    "bar_weight": 20.0,
    "default_unit": "kg",
    "default_sets": 3,
    "default_reps": 8,
    "key_points": "沉肩、肩胛后缩",
    "last_record_date": "2026-08-30",
    "best_1rm": 97.5
  }
]
```

| 字段 | 类型 | 说明 |
|---|---|---|
| `default_mode` | String | 计重方式：`bar`（含杆）/`support`（辅助，如引体）/`std`（自重） |
| `bar_weight` | f64 | 默认杆重 kg（bar 模式用） |
| `default_unit` | String | `kg` / `lb`（只影响展示，实际重量始终存 kg） |
| `default_sets/reps` | i64 | 建计划时的预填默认值 |
| `last_record_date` | String \| null | 最近一次训练日期（从未练过 → null） |
| `best_1rm` | f64 \| null | 历史最高 1RM（Epley 公式实时算；无记录 → null） |

### 4.2 创建动作 `POST /api/v1/exercises`

请求（除 name/body_part 外都有服务端默认值）：

```json
{
  "name": "深蹲",
  "body_part": "腿",
  "default_mode": "bar",
  "bar_weight": 20.0,
  "default_unit": "kg",
  "default_sets": 3,
  "default_reps": 8,
  "key_points": "核心收紧，膝盖与脚尖同向"
}
```

- 默认值：`default_mode="bar"`、`bar_weight=20.0`、`default_unit="kg"`、`default_sets=3`、`default_reps=8`、`key_points=""`
- 失败：`400` 名称或部位空 / 重名
- 成功 `200`：完整 `ExerciseOut`

### 4.3 动作详情 `GET /api/v1/exercises/{id}`

- 成功 `200`：`ExerciseOut`（含最近训练 + 最高 1RM）
- 失败 `404`

### 4.4 更新动作 `PATCH /api/v1/exercises/{id}`

请求（全部可选，部分更新）：

```json
{
  "name": "平板杠铃卧推",
  "body_part": "胸",
  "default_mode": "bar",
  "bar_weight": 20.0,
  "default_unit": "kg",
  "default_sets": 4,
  "default_reps": 8,
  "key_points": "新要领"
}
```

- 成功 `200`：更新后的 `ExerciseOut`；失败 `400`/`404`

### 4.5 删除动作 `DELETE /api/v1/exercises/{id}`

- 成功 `200`：`{"ok": true}`
- 失败 `404`
- ⚠️ 若动作已被模板/计划/记录引用，SQLite 外键约束会报 `500`（与页面层一致，引用检查是未来增强点）

---

## 5. 模板 + 计划 API

模板/计划都是"父表 + items 子表"结构。模板属于阶段；计划 = 某阶段某天的训练单。

### 5.1 模板列表 `GET /api/v1/phases/{phase_id}/templates`

- 归档阶段**可查看**（只验证归属，不要求未归档）
- 排序：按 `sort_order, id`
- 成功 `200`：`TemplateOut[]`

```json
[
  {
    "id": 2,
    "phase_id": 3,
    "name": "推日",
    "items": [
      { "id": 10, "exercise_id": 6, "exercise_name": "平板杠铃卧推", "plan_sets": 4, "plan_reps": 8 }
    ]
  }
]
```

### 5.2 创建模板 `POST /api/v1/phases/{phase_id}/templates`

请求：

```json
{
  "name": "推日",
  "items": [
    { "exercise_id": 6 },
    { "exercise_id": 7 }
  ]
}
```

- `items` 数组顺序 = 动作顺序（`sort_order` 自动按 enumerate 生成）
- `items` 不能为空；`exercise_id` 必须已存在于动作库（否则外键约束 → `500`）
- 归档阶段 → `403`
- 成功 `200`：完整 `TemplateOut`

### 5.3 更新模板 `PATCH /api/v1/templates/{id}`

- 请求体同 5.2（全量提交 name + items，**先删后插**）
- 归档阶段 → `403`；成功 `200`：更新后的 `TemplateOut`

### 5.4 删除模板 `DELETE /api/v1/templates/{id}`

- 成功 `200`：`{"ok": true}`；失败 `404`
- 注意：**不校验归档**（与页面层一致），但归档阶段下模板一般不该删——客户端自己控制入口

### 5.5 计划列表 `GET /api/v1/phases/{phase_id}/plans?date=2026-08-31`

- `date` 可选：不传 → 该阶段全部计划（按 `date DESC, id DESC`）；传了 → 只查那天
- 归档阶段**可查看**
- 成功 `200`：`PlanOut[]`

```json
[
  {
    "id": 5,
    "phase_id": 3,
    "date": "2026-08-31",
    "note": "推日，冲重量",
    "items": [
      {
        "id": 21,
        "exercise_id": 6,
        "exercise_name": "平板杠铃卧推",
        "body_part": "胸",
        "plan_sets": 4,
        "plan_reps": 8,
        "plan_weight": 60.0,
        "plan_rest": 90,
        "plan_key_points": "最后一组力竭",
        "plan_note": "本周第二次卧推"
      }
    ]
  }
]
```

`PlanItemOut` 的 `plan_*` 均可为 `null`（未预设）。

### 5.6 创建计划 `POST /api/v1/phases/{phase_id}/plans`

请求：

```json
{
  "date": "2026-08-31",
  "note": "推日，冲重量",
  "items": [
    {
      "exercise_id": 6,
      "plan_sets": 4,
      "plan_reps": 8,
      "plan_weight": 60.0,
      "plan_rest": 90,
      "plan_key_points": "最后一组力竭",
      "plan_note": null
    }
  ]
}
```

- `date` 必填且 `YYYY-MM-DD`；`note` 可空串；`items` 非空
- ⚠️ 同一阶段同一日期**唯一**（DB `UNIQUE(phase_id, date)`）——重复日期会 500，客户端应先查 5.5 判断
- 成功 `200`：完整 `PlanOut`

### 5.7 计划详情 `GET /api/v1/plans/{id}`

- 成功 `200`：`PlanOut`；失败 `404`

### 5.8 更新计划 `PATCH /api/v1/plans/{id}`

- 请求体同 5.6（全量提交 date + note + items，先删后插）
- 服务端已处理训练记录关联：
  备份记录 → 解除 `plan_item_id` → 删旧项 → 重插 → 按备份还原记录关联
- 归档阶段 → `403`
- 成功 `200`：更新后的 `PlanOut`

### 5.9 删除计划 `DELETE /api/v1/plans/{id}`

- 服务端会先解除 records 关联（保留训练历史）再删
- 成功 `200`：`{"ok": true}`；失败 `404`

---

## 6. 记录 API

一个计划项一天最多一条记录（upsert 语义）。

### 6.1 今日训练卡片 `GET /api/v1/today`

iced 客户端的**主屏数据源**。成功 `200`：

```json
{
  "phase": {
    "id": 3,
    "name": "增肌期",
    "days": 30
  },
  "date": "2026-08-31",
  "plan": {
    "id": 5,
    "note": "推日，冲重量",
    "items": [
      {
        "id": 21,
        "exercise_id": 6,
        "exercise_name": "平板杠铃卧推",
        "body_part": "胸",
        "plan_sets": 4,
        "plan_reps": 8,
        "plan_weight": 60.0,
        "plan_rest": 90,
        "plan_key_points": "最后一组力竭",
        "last_record": {
          "id": 88,
          "weight": 60.0,
          "sets": 4,
          "reps": 8,
          "rest": 90,
          "feeling": "状态不错",
          "strategy": "下次加 2.5kg"
        }
      }
    ]
  }
}
```

| 空态 | 含义 |
|---|---|
| `phase: null` | 没有进行中的阶段（或全部归档）→ 客户端提示"先去建阶段" |
| `plan: null` | 今天没有计划 → 客户端提示"今天没安排"并给"创建计划"入口 |
| `items[i].last_record: null` | 该动作今天还没练 → 显示"未训练"，保存时走 INSERT |
| `items[i].last_record` 有值 | 已练过 → 显示"✅ 已完成"（`completed` 由保存时的字段决定） |

### 6.2 记录 upsert `POST /api/v1/plans/{plan_id}/items/{item_id}/records`

保存今天某计划项的训练记录。**有最近记录 → 更新同一行；无 → 插入新行（日期 = 服务器当天）**。

请求：

```json
{
  "weight": 60.0,
  "sets": 4,
  "reps": 8,
  "rest": 90,
  "feeling": "状态不错",
  "strategy": "下次加 2.5kg",
  "key_points": "",
  "completed": true
}
```

- `weight/sets/reps` 必填；`rest/feeling/strategy/key_points` 缺省 `0`/空串；`completed` 缺省 `false`
- 负数 → `400 "重量/组数/次数/休息不能为负数"`
- 归档阶段 → `403`；计划/计划项不存在 → `404`
- ⚠️ 计划项验证是**双条件**（`item_id` 必须属于 `plan_id`），URL 里两个 id 都要传对
- 成功 `200`：保存后的 `RecordOut`

```json
{
  "id": 88,
  "exercise_id": 6,
  "exercise_name": "平板杠铃卧推",
  "record_date": "2026-08-31",
  "weight": 60.0,
  "sets": 4,
  "reps": 8,
  "rest": 90,
  "feeling": "状态不错",
  "strategy": "下次加 2.5kg",
  "key_points": "",
  "mode": "bar",
  "completed": true
}
```

> 注意：与页面层不同，API 保存**不回写**动作库（要领/计重配置/默认组次都不动）。
> 若 iced 客户端需要"保存时同步要领"等功能，这是 M9 扩展点。

### 6.3 按日期查记录 `GET /api/v1/records?date=2026-08-31`

- `date` 必填 `YYYY-MM-DD`
- 成功 `200`：当天该用户**全部动作**的 `RecordOut[]`（按 `exercise_id` 排序）
- 无记录 → `[]`（不是 404）

### 6.4 更新记录 `PATCH /api/v1/records/{id}`

请求（全部可选，部分更新）：

```json
{
  "weight": 62.5,
  "sets": 4,
  "reps": 8,
  "rest": 90,
  "feeling": "状态不错",
  "strategy": "下次加 2.5kg",
  "key_points": "",
  "completed": true
}
```

- 成功 `200`：更新后的 `RecordOut`；负数 → `400`；不存在 → `404`

### 6.5 删除记录 `DELETE /api/v1/records/{id}`

- 成功 `200`：`{"ok": true}`；失败 `404`

---

## 7. 统计 API

只读端点，全部需要登录。

### 7.1 历史日历 `GET /api/v1/history?year=2026&month=08`

- 参数可选：缺省 → 服务器当前年月
- `year` 必须 4 位数字、`month` 必须 2 位数字（如 `"08"`，注意**前导零**）
- 成功 `200`：

```json
{
  "year": "2026",
  "month": "08",
  "train_days": ["2026-08-01", "2026-08-15", "2026-08-30"]
}
```

- 客户端拿 `train_days` 自己画日历（iced 场景）

### 7.2 某天详情 `GET /api/v1/history/{date}`

- `date` = `YYYY-MM-DD`
- 成功 `200`：`DayRecordOut[]`

```json
[
  {
    "id": 88,
    "exercise_id": 6,
    "exercise_name": "平板杠铃卧推",
    "body_part": "胸",
    "mode": "bar",
    "weight": 60.0,
    "sets": 4,
    "reps": 8,
    "rest": 90,
    "feeling": "状态不错",
    "strategy": "下次加 2.5kg",
    "key_points": "",
    "1rm": 75.0
  }
]
```

> ⚠️ JSON 键名是 **`"1rm"`**（小写，Rust 标识符不能以数字开头，serde 用 rename）。

### 7.3 动作统计 `GET /api/v1/exercises/{id}/stats`

- 成功 `200`：

```json
{
  "exercise": { "id": 6, "name": "平板杠铃卧推", "body_part": "胸" },
  "records": [
    { "date": "2026-08-01", "weight": 55.0, "sets": 3, "reps": 10, "1rm": 73.3 },
    { "date": "2026-08-15", "weight": 60.0, "sets": 4, "reps": 8, "1rm": 75.0 }
  ],
  "best_1rm": 75.0
}
```

- `records` 按日期升序（趋势图直接可用）；`1rm` 每条实时算（Epley）
- 失败：`404` 动作不存在

---

## 8. iced 客户端接入建议

### 8.1 HTTP 客户端选型

推荐 **reqwest**（异步）+ **serde/serde_json**，与项目 Cargo.toml 一致。iced 的 `Subscription` 可以跑异步任务：

```rust
// Cargo.toml 增加
// reqwest = { version = "0.12", features = ["json", "rustls-tls"], default-features = false }
// serde = { version = "1", features = ["derive"] }
// serde_json = "1"
```

### 8.2 认证流程

```rust
// ① 登录：拿到 token 存本地
let resp = client.post("http://host/api/v1/login")
    .json(&json!({"username": u, "password": p}))
    .send().await?;
let body: serde_json::Value = resp.json().await?;
let token = body["token"].as_str().unwrap().to_string(); // 持久化（如本地文件）

// ② 后续每个请求手动带 Cookie 头
let resp = client.get("http://host/api/v1/today")
    .header("Cookie", format!("session={}", token))
    .send().await?;

// ③ 启动自检：GET /api/v1/me
//    200 → 登录态有效；401 → 清 token 跳登录
```

### 8.3 错误处理模式

```rust
match resp.status().as_u16() {
    200 => { /* 反序列化数据 */ }
    401 => { /* 清 token，切到登录页 */ }
    400 | 403 | 404 => {
        // 解析 {"error": "..."}，把消息直接展示给用户
        let err = resp.json::<serde_json::Value>().await?;
        show_error(err["error"].as_str().unwrap_or("未知错误"));
    }
    _ => show_error("服务器错误，稍后再试"),
}
```

### 8.4 数据流建议（对应页面）

| iced 页面 | 数据源 |
|---|---|
| 登录页 | `POST /login` → 存 token |
| 主屏（今日训练） | `GET /today`；保存动作 → `POST /plans/{plan_id}/items/{item_id}/records` |
| 阶段列表/编辑 | `GET /phases` → 增删改 `/phases...` |
| 动作库 | `GET /exercises?body_part=` → CRUD |
| 模板管理 | `GET /phases/{id}/templates` → CRUD |
| 计划编排 | `GET /phases/{id}/plans?date=` → 创建/更新计划 |
| 历史回顾 | `GET /history?year=&month=` → 点某天 → `GET /history/{date}` |
| 动作趋势 | `GET /exercises/{id}/stats` |

### 8.5 已知边界（写客户端时注意）

1. **日期唯一性**：创建计划前先查 `GET /phases/{phase_id}/plans?date=今天`，已存在则引导用户改日期或去编辑，否则 500
2. **归档只读**：归档阶段下的所有写操作（建模板/计划、存记录）会 403，客户端应隐藏或禁用入口
3. **删除动作**：被引用的动作删除会 500，客户端删除前应提示"该动作有训练记录"
4. **记录日期**：upsert 的 `record_date` 由**服务器时钟**决定（`date('now','localtime')`），客户端不传
5. **1RM 键名**：`"1rm"` 小写；`ExerciseOut` 里是 `"best_1rm"`
6. **体重**：`UserOut.body_weight` 为 `null` 表示未设置；API 暂无修改体重的端点（页面层有，M9 可补）
7. **Cookie 头格式**：`Cookie: session=<token>`，`token` 是 UUID 字符串（登录响应里的 `token` 字段）

---

## 附：端点速查表

| 方法 | 路径 | 说明 | 需登录 |
|---|---|---|---|
| POST | `/api/v1/login` | 登录，返回 user + token | ✗ |
| POST | `/api/v1/logout` | 登出 | ✗ |
| GET | `/api/v1/me` | 当前用户 | ✓ |
| GET | `/api/v1/phases` | 阶段列表 | ✓ |
| POST | `/api/v1/phases` | 创建阶段 | ✓ |
| GET | `/api/v1/phases/{id}` | 阶段详情 | ✓ |
| PATCH | `/api/v1/phases/{id}` | 更新阶段 | ✓ |
| POST | `/api/v1/phases/{id}/archive` | 归档阶段 | ✓ |
| POST | `/api/v1/phases/{id}/unarchive` | 启用阶段 | ✓ |
| GET | `/api/v1/exercises?body_part=` | 动作列表 | ✓ |
| POST | `/api/v1/exercises` | 创建动作 | ✓ |
| GET | `/api/v1/exercises/{id}` | 动作详情（含 1RM） | ✓ |
| PATCH | `/api/v1/exercises/{id}` | 更新动作 | ✓ |
| DELETE | `/api/v1/exercises/{id}` | 删除动作 | ✓ |
| GET | `/api/v1/phases/{phase_id}/templates` | 模板列表 | ✓ |
| POST | `/api/v1/phases/{phase_id}/templates` | 创建模板 | ✓ |
| PATCH | `/api/v1/templates/{id}` | 更新模板 | ✓ |
| DELETE | `/api/v1/templates/{id}` | 删除模板 | ✓ |
| GET | `/api/v1/phases/{phase_id}/plans?date=` | 计划列表 | ✓ |
| POST | `/api/v1/phases/{phase_id}/plans` | 创建计划 | ✓ |
| GET | `/api/v1/plans/{id}` | 计划详情 | ✓ |
| PATCH | `/api/v1/plans/{id}` | 更新计划 | ✓ |
| DELETE | `/api/v1/plans/{id}` | 删除计划 | ✓ |
| GET | `/api/v1/today` | 今日训练卡片 | ✓ |
| POST | `/api/v1/plans/{plan_id}/items/{item_id}/records` | 记录 upsert | ✓ |
| GET | `/api/v1/records?date=` | 按日期查记录 | ✓ |
| PATCH | `/api/v1/records/{id}` | 更新记录 | ✓ |
| DELETE | `/api/v1/records/{id}` | 删除记录 | ✓ |
| GET | `/api/v1/history?year=&month=` | 历史日历 | ✓ |
| GET | `/api/v1/history/{date}` | 某天详情 | ✓ |
| GET | `/api/v1/exercises/{id}/stats` | 动作统计 | ✓ |
