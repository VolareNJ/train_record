# 🐛 M4 后 Bug 修复补课笔记（vibe coding 复盘）

> **用途**：M4 之后的实际使用测试与 debug 大多由 AI 直接代写（vibe coding）。
> 本文档从**知识点**角度复盘每个 BUG：遇到什么问题、根因是什么、怎么修、
> 涉及哪些**跨框架通用**的概念（HTML/JS 语法细节跳过——学 iced 用不上）。
>
> **阅读建议**：每个 BUG 花 5-10 分钟，先想"根因是什么"，再看修复。
> 第 8 节是一句话总结表，适合快速回顾。

---

## 1. 动作库筛选选回"全部"后表格消失

### 现象

动作库页面：筛选部位"胸" → 正常；选回"全部" → **表格整个消失**（空表）。

### 根因（Rust/serde 层，会 iced 重演 ⚠️）

下拉框 `<option value="">全部</option>` 提交时 `body_part=`（**空串**）。
serde 解析 `Form<T>` 时：

- `body_part` 字段声明为 `Option<String>`
- 空串 ≠ 缺键：**`body_part=` 被解析成 `Some("")`，不是 `None`**
- 后端 `match Some("")` 走进"按部位筛选"分支 → `WHERE body_part = ''` → 空结果

**核心教训**：在 Web 表单/RPC 里，"字段存在但值为空"和"字段不存在"是两回事。
Rust 的 `Option` 只能表达"有没有"，无法表达"有但是空"。

### 修复

```rust
// 空串当作"全部"：filter 掉空串后，Some("胸") 才走筛选
let part_filter = query.body_part.as_deref().filter(|p| !p.is_empty());
match part_filter
{
    None => /* 查全部 */,
    Some(pt) => /* WHERE body_part = ? */,
}
```

### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| serde 空串 vs None | 空串解析成 `Some("")` 而非 `None` | ✅ 必考（iced 用 serde 解析配置/消息） |
| `filter()` 净化输入 | `Option::filter(谓词)` 把"不满足条件"的值变成 None | ✅ 通用 Rust |
| 输入净化模式 | 用户输入先归一化（空→无），再进业务逻辑 | ✅ 通用思维 |

---

## 2. 表单部位筛选后表格"不连续"（残留空行）

### 现象

模板/计划的创建/编辑页：筛选"胸"后，表格里出现**大量空行**（本应只显示胸动作）。

### 根因（HTML 层，但状态管理思维通用）

原结构每行是：

```html
<label data-part="胸">…</label><br>
```

JS 隐藏时只隐藏了 `<label>`，`<br>` 是 label 的**兄弟节点**（在 label 外），
隐藏 label 后 `<br>` 仍然占位 → 空行。

### 修复

每行改成**块级容器 div 包整行**，隐藏时整行消失无残留：

```html
<div class="ex-row" data-part="胸">
    <label><input type="checkbox"> 卧推</label>
</div>
```

JS 变成按容器显隐：`row.style.display = (part === '' || row.dataset.part === part) ? '' : 'none'`

### 知识点（状态管理思维，iced 必考 ✅）

| 知识点 | 说明 | iced 对应 |
|---|---|---|
| **视图状态单一来源** | 行的显隐只由 `data-part` + 当前筛选值决定，不维护额外状态 | iced 里 `State` 字段 + 消息驱动，**同一思维** |
| 隐藏/显示 = 条件渲染 | "满足条件才渲染/显示" | iced 里 `if` 分支决定是否渲染元素 |
| 容器 vs 文本节点 | 显隐操作应该作用于"完整单元"（容器），而非零散节点 | — |

---

## 3. 编辑计划：编辑框布局优化（方案 b）

### 现象

编辑计划页每个动作后面直接跟 7 个输入框（计重/杆重/休息/组/次/重/要领），
一行塞满，手机上没法看。

### 方案选择（架构思维）

- **方案 a**：每个动作加"编辑详情"按钮 → 弹窗/独立界面
  - 需要数据同步机制（弹窗里的改动要先暂存）或拆保存接口
  - 破坏现有"整体提交、后端先删后插"的事务模型
- **方案 b**（采纳）：**编辑框只在动作勾选时显示**，且每个字段换行
  - 纯前端改动、保持单表单整体提交、未勾选动作不显示编辑框更干净

### 修复

1. 每行动态切换：勾选 → 显示该行编辑详情块；取消勾选 → 隐藏
2. 编辑详情块内每个字段单独一行（手机友好）
3. 已勾选动作的详情块默认可见；未勾选默认 `display:none`

```html
<div class="ex-row" data-part="胸">
    <label><input type="checkbox" onchange="toggleDetail(6)"> 卧推</label>
    <div id="detail-6" style="display:none">
        计重方式 <select>…</select><br>
        组数 <input name="sets_6"> …<br>
        …
    </div>
</div>
```

### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| **UI 状态 = 数据状态的投影** | 详情块显隐完全由 checkbox 的 checked 决定，不额外存一份 | ✅ iced 核心（State → View 的单向数据流） |
| 事件时机：`change` vs `click` | 用 `change`（先切换 checked 再读状态），避免读到旧值 | ✅ 通用（消息顺序） |
| 隐藏输入框仍会提交 | 表单里 `display:none` 的 input 照样随提交发送 → 后端必须按勾选键过滤（白名单） | ✅ 后端防御 |

**⚠️ 隐藏输入框仍会提交** —— 这是本 BUG 最隐蔽的坑：
JS 把详情块 `display:none` 后，里面的 `<input name="sets_6">` 仍然会随表单提交！
但后端 `plan_update` 只认 `exercise_ids()`（勾选的数字键过滤出来的动作），
未勾选动作的 `sets_6` 等键会被忽略——**前端隐藏 + 后端白名单过滤，双层防护**。

---

## 4. 生产环境删除计划报数据库错误（FOREIGN KEY constraint failed）

### 现象

生产环境点"删除计划"→ 页面报数据库错误。日志：

```
ERROR train_record::error: 数据库错误: error returned from database:
(code: 787) FOREIGN KEY constraint failed
```

### 根因（数据库层，iced + 后端照样会遇到 ✅）

`records` 表有一个外键：

```sql
plan_item_id INTEGER REFERENCES plan_items(id)  -- 训练记录关联到"计划动作项"
```

`plan_delete` 原来的事务是：

1. `DELETE FROM plan_items WHERE plan_id = ?` ← **这里炸了**
2. `DELETE FROM plans WHERE id = ?`

当某个计划项**已经被训练过**（records 里有行引用它）时，删除它违反外键约束。

### 修复

删 plan_items 之前，先解除关联（**保留训练历史**，不是删记录）：

```sql
-- 该计划下所有计划项对应的 records，plan_item_id 置 NULL
UPDATE records SET plan_item_id = NULL
WHERE plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?);
```

`plan_update`（编辑计划 = 先删后插）也有同样隐患，一并修复。

### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| **外键约束** | 被引用的行不能直接删，除非先处理引用方 | ✅（配后端时必考） |
| 删除策略三选一 | ① 级联删（`ON DELETE CASCADE`）② 置 NULL（保留数据解除关联）③ 拒绝删 | ✅ 数据建模决策 |
| 本项目选择：置 NULL | 训练记录是用户历史数据，删计划不该连记录一起删 | ✅ 业务决策 |
| 事务顺序 | "先解除关联 → 再删子 → 再删父"必须在一个事务里 | ✅ |
| 错误码 787 | SQLite 外键约束失败 | — |
| 生产排查法 | 看 `app.log` 定位真实错误（页面只显示"数据库错误"） | ✅ 运维技能 |

---

## 5. 生产排查方法（vibe coding 也值得掌握的技能）

这次 debug 走了完整链路：

1. **看日志**：`tail -50 /opt/train_record/app.log` → 拿到真实错误 `FOREIGN KEY constraint failed`
2. **读迁移**：`grep REFERENCES migrations/*.sql` → 发现 `records.plan_item_id → plan_items(id)`
3. **读 handler**：确认 `plan_delete` 没处理 records
4. **本地复现**：8080 造一条记录 → 删除 → 复现/验证修复
5. **数据验证**：删完后用 python 查 DB，确认记录保留、关联已解除

> **核心**：错误信息永远比"页面表现"更接近真相。页面只说"数据库错误"，
> 日志里有具体约束。学会"现象 → 日志 → 表结构 → 代码"的排查链。

---

## 6. 涉及但跳过的知识点（为什么跳过）

| 知识点 | 说明 | 为什么跳过 |
|---|---|---|
| DOM 操作（`querySelectorAll`、`style.display`） | 浏览器特有 | iced 没有 DOM |
| HTML 表单提交（`FormData`、`submit`） | 浏览器特有 | iced 用消息，不用表单 |
| CSS 布局（`<br>`、`div`、`display`） | 浏览器特有 | iced 用布局容器 |
| JS 事件（`onchange`、`addEventListener`） | 浏览器特有 | iced 用消息订阅 |

**结论**：HTML/JS 部分 vibe coding 完全 OK，不必补课。

---

## 7. 需要真正补课的（iced 必考清单）

按优先级排序，学 iced 前务必消化：

1. **serde 空串 vs None**（BUG 1）：`Option<String>` 接不住"空串存在"的情况
2. **UI 状态 = 数据状态投影**（BUG 3）：显隐不额外存状态，从数据推导
3. **事件顺序**（BUG 3）：`change` 先改状态再触发，别读旧值
4. **删除策略**（BUG 4）：外键约束下"级联删 / 置 NULL / 拒绝删"三选一
5. **输入净化链**（BUG 1 + 3）：前端隐藏 ≠ 不提交，后端永远白名单校验
6. **自动计算保护条件**（§9.1）：自动联动写值要区分"初始回显"与"用户输入"，空输入 ≠ 0
7. **迁移设计约束**（§9.2）：SQLite `ADD COLUMN` 只能加 NULL/带默认值列，新列对老数据 = NULL，模型必须 `Option`

---

## 8. 一句话总结表

| # | BUG | 根因（一句话） | 知识点 |
|---|---|---|---|
| 1 | 选回"全部"表格消失 | 空串被解析成 `Some("")`，误入筛选分支 | serde 空串 ≠ None |
| 2 | 筛选后残留空行 | `<br>` 是 label 兄弟节点，隐藏 label 没隐藏它 | 容器级显隐 |
| 3 | 编辑框一行塞满 | 7 个输入框全排一行 | 条件渲染 + 状态投影 |
| 4 | 删除计划报外键错误 | records 引用 plan_items，删被引用的行违反约束 | 外键删除策略 |
| 5 | 自动更新覆盖回显值 | JS 无条件写 weight，把计划回显 60 覆盖成杆重 20 | 自动计算要保护条件 |
| 6 | 改了代码但没生效 | 浏览器缓存旧 JS，curl 却看到新文件 | 版本号破缓存 |

---

## 9. M4 结尾追加：去按钮自动更新 + 动作级备注（vibe coding 复盘）

> M4 结尾的实际使用测试与修 bug 阶段做三件事：① 去掉"填入强度"按钮改自动更新；
> ② 新增动作级备注 `plan_note`；③ AGENTS.md 加编译缓存检查（流程约定，无知识点，不补课）。
> 本文档继续沿用前 8 节的复盘格式：只补**跨框架通用**的知识点。

### 9.1 自动更新的"回显值保护"

#### 现象

编辑计划页去掉"填入强度"按钮、改为 JS 自动写入实际强度后，出现隐蔽问题：
页面加载时，已有计划实际强度（如卧推 60）的行，其"观测强度"输入框是**空的**；
若 JS 无条件执行 `weightInput.value = rowTotal()`，会把 60 覆盖成"纯杆重 20"
（bar 模式空片 = 20kg）。用户什么都没干，回显值就丢了。

#### 根因（交互层思维，iced 会重演 ⚠️）

- 自动计算逻辑（换算器）只认"当前输入值"，不知道值的**来源**
- 页面初始状态有两类数据：**回显值**（数据库来的计划值）和**用户输入**
- "观测强度为空"有两种含义：① 用户还没输入（应保留回显值）② 用户清空了（也不该覆盖）
- 无保护的自动写入 = 把"空输入"当成"0 输入" → 破坏回显

**核心教训**：自动联动 ≠ 无条件覆盖。写值前要问：**这个位置的值，现在是谁的？**

#### 修复

```js
// 只在观测强度有输入时写入——页面加载时 plate 为空，
// 不能覆盖 weight-input 已有的回显值（计划预设/上次记录）。
if (plateInput.value !== '') {
    weightInput.value = roundToHalf(total);
}
```

record_form 的 `weight_converter.js` 与编辑计划页的行级 JS 用同一套保护。

#### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| 自动计算要有写入条件 | 自动联动 ≠ 无条件覆盖，要区分"初始回显"与"用户输入" | ✅ iced 里 State 初始化值 vs 消息更新同样要防 |
| 空输入 ≠ 0 | 输入框为空是"无输入"，不是数值 0 | ✅ 通用 |
| 数据来源意识 | 写值前先问"这个值现在是谁的"（回显？用户？计算？） | ✅ 单向数据流思维 |

---

### 9.2 动作级备注的迁移设计（SQLite ALTER TABLE 限制）

#### 现象

需求：计划中**每个动作**可以写备注（如"深蹲行程深一些"、"硬拉70kg晋级赛"），
区别于整计划的 `plans.note`。`plan_items` 表没有该字段。

#### 设计与根因（数据库层，iced + 后端必考 ✅）

迁移 `0004_plan_items_note.sql`：

```sql
ALTER TABLE plan_items ADD COLUMN plan_note TEXT;
```

SQLite 的 `ALTER TABLE ADD COLUMN` 限制：

- 新列必须是：允许 NULL，或带默认值；**不能**加 UNIQUE/PRIMARY KEY，不能设 NOT NULL 无默认值
- 新列对**老数据行 = NULL**（已存在的计划项没有备注，自动变成 NULL）
- 因此 Rust 模型必须 `Option<String>`：`pub plan_note: Option<String>`

空串归一化与 `plan_key_points` 同模式：表单空串 trim 后 → `None` → 入库 NULL，
保证"没填"和"填了空"一致（与 BUG 1 呼应）。

#### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| SQLite ALTER 限制 | `ADD COLUMN` 只能加 NULL/带默认值列，不能加约束 | ✅ 后端必考 |
| 新列 → 老数据 NULL | 迁移加列后旧行全为 NULL，模型层必须 `Option` | ✅ |
| 空串 vs NULL 归一化 | 表单空串 → None → NULL，与"没填"一致 | ✅ 与 BUG 1 同一思维 |
| 整计划备注 vs 动作备注 | `plans.note`（一条）vs `plan_items.plan_note`（每动作一条），1:N 关系 | ✅ 数据建模 |

---

### 9.3 静态文件缓存：改了代码但没生效

#### 现象

record_form 去掉按钮后，浏览器仍报
`Cannot read properties of null (reading 'addEventListener')`
——旧 JS 里 `fillBtn` 为 null 崩了。curl 验证服务器文件已是新的，
浏览器却仍在执行旧代码。

#### 根因（部署/运维，跨框架通用 ✅）

浏览器 HTTP 缓存了旧 JS。**curl 是全新请求，浏览器是复用本地缓存**——
两个视角看到的文件版本不同。排查时先 curl 服务器确认文件内容，
再怀疑浏览器缓存。

#### 修复

静态资源引用加版本号，URL 变化 → 浏览器强制重新下载：

```html
<script src="/static/weight_converter.js?v=2"></script>
```

以后每次改 JS 递增版本号（v=3、v=4…）。

#### 知识点

| 知识点 | 说明 | iced 会重演吗 |
|---|---|---|
| 缓存 vs 服务器文件 | "代码改了没生效"先 curl 验证服务器，再查浏览器缓存 | ✅ 部署运维通用 |
| 版本号破缓存 | 资源 URL 变化 → 强制重新下载，最简可靠方案 | ✅ |
| 排查顺序 | 现象 → curl 服务器 → 浏览器无痕/版本号 | ✅ 与 §5 排查法呼应 |

