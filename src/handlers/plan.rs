// ============================================================
// handlers/plan.rs —— 训练计划（模板 Template + 当日计划 Plan）的 HTTP 处理器
// ============================================================
// 【教学说明】
// 这个文件处理"与训练计划相关的 HTTP 请求"，分两大块：
//
// 一、模板（Template）—— 可复用的动作配方
//   GET  /phases/{phase_id}/templates        → 模板列表（list_templates）
//   GET  /phases/{phase_id}/templates/new    → 新建模板表单（template_create_form）
//   POST /phases/{phase_id}/templates        → 创建模板（template_create）
//   GET  /templates/{id}/edit                → 编辑模板表单（template_edit_form）
//   POST /templates/{id}/edit                → 更新模板（template_update）
//   POST /templates/{id}/delete              → 删除模板（template_delete）
//
// 二、当日计划（Plan）—— 某一天的训练安排
//   GET  /phases/{phase_id}/plans            → 计划列表（list_plans）
//   GET  /phases/{phase_id}/plans/new        → 新建计划表单（plan_create_form）
//   POST /phases/{phase_id}/plans            → 创建计划（plan_create）
//   GET  /plans/{id}                         → 计划详情（plan_detail）
//   GET  /plans/{id}/edit                    → 编辑计划表单（plan_edit_form）
//   POST /plans/{id}/edit                    → 更新计划（plan_update）
//   POST /plans/{id}/delete                  → 删除计划（plan_delete）
//
// 📌 阶段要求：M3 你来实现本文件所有函数。
//   实现完成后对照检查（完整实现备份在 docs/learning_path/M3_ref/）。
// ============================================================

// 【教学：本文件用到的导入】
// 比 M2 的 phases.rs 多了几个东西：
//   - Query（可空查询参数）：计划列表按日期筛选时用
//   - Template / TemplateItem / Plan / PlanItem：models.rs 里 M2 就建好的模型
//   - 注意：本文件同时用到"阶段下查模板"（带 phase_id）和
//     "按模板 id 操作"（不带 phase_id）两种路由，参数来源不同
use axum::{
    extract::{Form, Path, Query, State},
    response::{Html, Redirect},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    AppState,
    error::AppError,
    handlers::auth::AuthUser,
    models::{Exercise, Phase, Plan, PlanItem, Template, TemplateItem},
};

// ============================================================
// 【教学：从"单表 CRUD"到"父子表 CRUD"的跨越】★ 本阶段核心
// ============================================================
// M2 的阶段/动作都是"单表"：一个 handler 只操作一张表。
// M3 的模板/计划都是"父子表"：
//   模板(templates) ─┬─ 模板项(template_items)：父表的"孩子"
//   计划(plans) ─────┴─ 计划项(plan_items)：父表的"孩子"
//
// 父表行被子表引用时的两个铁律：
//   1. 【查询】要拿父表的"孩子"，先查父表拿 id，再按父 id 查子表
//      或者用 JOIN（M5 再深入，M3 先用两次查询，更直白）
//   2. 【删除】先删子表（所有引用父的行），再删父表——"先子后父"
//      否则父表删了，子表还挂着不存在的父 id（孤儿数据）
//
// 这两个铁律会反复出现在本文件的每个函数里，先记住它们。

// ============================================================
// 【教学：事务（Transaction）—— 多步操作要么全成、要么全败】
// ============================================================
// M3 有两个场景必须用事务（否则会留"半截数据"）：
//   场景 A：创建模板 = INSERT 模板 + 批量 INSERT 模板项
//     如果模板项插到一半出错，模板已插入——数据库里多了个"空壳模板"
//   场景 B：从模板复制生成计划 = INSERT 计划 + 批量 INSERT 计划项
//     同理，复制到一半出错，计划就残了
//
// 事务 = 把多个 SQL 包成一个"原子操作"：
//   要么全部成功（commit 提交），要么全部回滚（rollback，像没发生过）
//
// 用法：
//   let mut tx = state.pool.begin().await.map_err(AppError::Database)?;
//   // ... 在 tx 上执行多条 SQL（和 pool 用法一样，只是把 pool 换成 &mut *tx）
//   tx.commit().await.map_err(AppError::Database)?;  // 全成功才提交
//   出错时提前 return Err(...)，tx 被 drop 自动回滚（不用手动 rollback）
//
// 注意：tx 执行 SQL 时要写成 &mut *tx（Begin 的 DerefMut），
// 或者干脆 tx.execute(...)（方法自动解引用，看 sqlx 版本）。

// ============================================================
// 【教学：多对多关系怎么建？—— 用"中间表"（这里叫 item 表）】
// ============================================================
// 一个模板包含多个动作，一个动作可出现在多个模板——
// 这是"多对多"关系。SQL 里的标准解法是建中间表（template_items）：
//
//   templates        template_items         exercises
//   ┌─────┐         ┌──────────────┐       ┌────────┐
//   │  id │──1:N───>│ template_id  │<──N:1──│  id    │
//   │ name│         │ exercise_id  │        │ name   │
//   └─────┘         │ plan_sets    │        └────────┘
//                   │ plan_reps    │
//                   └──────────────┘
//
// 中间表一行 = "模板 X 里有动作 Y，计划组数 N 次 M"。
// 这样不用改任何一张原始表，就表达了任意组合关系。
// 这也是为什么 models.rs 里 TemplateItem 有 template_id + exercise_id 两个外键。

// ============================================================
// 第一部分：模板（Template）
// ============================================================

// ============================================================
// 模板列表（GET /phases/{phase_id}/templates）
// ============================================================
/// 显示某阶段下的所有模板
///
/// 【教学：两级验证】
/// M2 的动作列表只验"登录了没"（AuthUser）。
/// 这里要**两级验证**：URL 里的 phase_id 必须是**当前用户**的阶段。
/// 为什么？因为如果只按 phase_id 查模板，黑客可以访问
/// /phases/{别人的阶段}/templates 看到别人的模板——数据隔离漏洞！
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 查阶段是否存在且属于当前用户：
///    SELECT * FROM phases WHERE id = ? AND user_id = ?
///    → fetch_optional → None 则 Err(NotFound)（阶段不存在或不是你的）
/// 3. 查模板：SELECT * FROM templates WHERE phase_id = ?
/// 4. 拼 HTML：表格列出模板名 + 操作链接（编辑/删除/查看计划入口）
pub async fn list_templates(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示：先把"阶段属于当前用户"查出来（user 变量已解构）
    //   阶段不属于当前用户 → Err(NotFound)
    //   阶段已归档（archived=1）→ 列表页顶部加提示"已归档，只读"
    //   然后查模板列表，每行：模板名 + 编辑/删除链接
    //   最后加"新建模板"链接：/phases/{phase_id}/templates/new
    let phase_ret = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("Phase not found".to_string()))?;

    let template_ret = sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE phase_id = ?")
        .bind(&phase_ret.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|item| {
            format!(
                "<tr><td>{tmp_id}</td><td>{pha_id}</td><td>{tmp_name}</td></tr>",
                tmp_id = item.id,
                pha_id = item.phase_id,
                tmp_name = item.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    Ok(Html(format!(
        r#"
                <h2>训练模板</h2>
                    <table border="1"><tr><th>ID</th><th>阶段ID</th><th>名称</th></tr>
                        {tmp_content}
                    </table>
                <p><a href="/phases/{phase_id}/templates/new">创建训练模板</a></p>
                <p><a href="/">返回首页</a></p>"
            "#,
        tmp_content = template_ret,
        phase_id = phase_ret.id
    )))
    // unimplemented!("M3 学生实现：模板列表")
}

// ============================================================
// 新建模板表单页（GET /phases/{phase_id}/templates/new）
// ============================================================
/// 显示新建模板的表单（模板名 + 多选动作）
///
/// 【教学：多选（checkbox）表单】
/// 一个模板包含多个动作，表单里用 <input type="checkbox" name="exercise_ids" value="1">
/// 每个动作一个勾选框，name 都叫 exercise_ids（重复 name = 数组）。
/// axum 端用 Form<Vec<i64>> 或自定义结构体接收：
///   struct TemplateCreateForm { name: String, exercise_ids: Vec<i64> }
/// 浏览器会提交 exercise_ids=1&exercise_ids=2&exercise_ids=3
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 验证阶段属于当前用户（同 list_templates）
/// 3. 查全部动作：SELECT * FROM exercises WHERE user_id = ?（供勾选）
/// 4. 拼 HTML：表单 + 动作勾选列表（checkbox）+ 提交按钮
pub async fn template_create_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示：动作列表用 exercises 表的 map 生成 checkbox 行
    //   <label><input type="checkbox" name="exercise_ids" value="{id}"> {name}</label>
    // 表单 action = /phases/{phase_id}/templates
    let phase_ret = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("Phase not found".to_string()))?;

    let checkbox_rows = sqlx::query_as::<_, Exercise>
    ("SELECT * FROM exercises WHERE user_id = ?")
    .bind(&user.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .iter()
    .map(|ex| format!(
            r#"<label><input type="checkbox" name="exercise_ids" value="{id}"> {name}</label><br>"#,
            id = ex.id,
            name = ex.name
        ))
    .collect::<Vec::<String>>()
    .join("\n");

    Ok(Html(format!(
        r#"
    <h2>创建训练模板</h2>
    <form method="post" action="/phases/{phase_id}/templates">
        模板名：<input name="name"><br>
        {checkbox_rows}
        <button type="submit">创建</button>
    </form>
    <p><a href="/phases/{phase_id}/templates">返回模板列表</a></p>
    "#,
        phase_id = phase_ret.id,
        checkbox_rows = checkbox_rows,
    )))
    // unimplemented!("M3 学生实现：新建模板表单")
}

// ============================================================
// 创建模板（POST /phases/{phase_id}/templates）
// ============================================================
/// 处理新建模板表单提交（模板名 + 多个动作）
///
/// 【教学：事务 —— 一次写两张表】
/// 创建模板要写两张表：templates（父）+ template_items（子）。
/// 必须用事务包裹，否则子表插入失败会留空壳模板。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id) + Form(form)
/// 2. 验证阶段属于当前用户 + 未归档
/// 3. begin 事务
/// 4. INSERT INTO templates (phase_id, name, sort_order) VALUES (?, ?, ?)
///    → query_scalar::<_, i64> 拿回模板 id（last_insert_rowid）
/// 5. 遍历 form.exercise_ids，逐个 INSERT template_items（带 sort_order 递增）
/// 6. commit → 重定向回模板列表
///
/// 【教学：表单结构体】
/// #[derive(Deserialize)]
/// struct TemplateCreateForm {
///     name: String,
///     exercise_ids: Vec<i64>,   // checkbox 同名 → Vec
/// }
pub async fn template_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
    Form(form): Form<TemplateCreateForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示：INSERT 父表后用 query_scalar 拿新 id：
    //   let template_id: i64 = sqlx::query_scalar("INSERT INTO templates (...) VALUES (...) RETURNING id")
    //     .bind(...).fetch_one(&mut *tx).await.map_err(AppError::Database)?;
    //   （SQLite 3.35+ 支持 RETURNING，sqlx 的 SQLite 驱动可用）
    // 然后 .bind 循环插入子表。
    // 成功 → Ok(Redirect::to(&format!("/phases/{phase_id}/templates")))
    unimplemented!("M3 学生实现：创建模板")
}

// ============================================================
// 编辑模板表单页（GET /templates/{id}/edit）
// ============================================================
/// 显示编辑模板的表单（模板名 + 动作多选，已选的勾上）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 查模板 + 验证归属（JOIN phases 或先查 phase 再验 user_id）
///    SELECT t.*, p.user_id FROM templates t JOIN phases p ON t.phase_id = p.id
///    WHERE t.id = ? → 检查 p.user_id == user.id
/// 3. 查模板已有的动作：SELECT exercise_id FROM template_items WHERE template_id = ?
///    → 得到 HashSet<i64>（已选集合，判断 checkbox 勾选状态）
/// 4. 查全部动作（同 create_form）
/// 5. 拼表单：已选的 checkbox 加 checked 属性
pub async fn template_edit_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示：判断"已选"用 HashSet：
    //   let selected: HashSet<i64> = template_items.iter().map(|t| t.exercise_id).collect();
    //   checkbox 行：if selected.contains(&ex.id) { " checked" } else { "" }
    unimplemented!("M3 学生实现：编辑模板表单")
}

// ============================================================
// 更新模板（POST /templates/{id}/edit）
// ============================================================
/// 处理编辑模板表单提交（改名 + 换动作集合）
///
/// 【教学：更新子表集合的标准套路 —— "先删后插"】
/// 模板的动作集合可能增、减、换顺序。最简单可靠的做法：
///   1. 更新父表（改名）：UPDATE templates SET name = ? WHERE id = ?
///   2. 删掉所有旧子表行：DELETE FROM template_items WHERE template_id = ?
///   3. 重新插入所有勾选的动作（和 create 一样的循环）
/// 三步在一个事务里 → 不会出现"删了没插上"的半截状态。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id) + Form(form)
/// 2. 验证模板归属当前用户（同 edit_form 的 JOIN 验证）
/// 3. begin 事务 → UPDATE 父表 → DELETE 子表 → 循环 INSERT
/// 4. commit → 重定向回模板列表
pub async fn template_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Form(form): Form<TemplateCreateForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示："先删后插"三步都在事务里，顺序不能乱
    unimplemented!("M3 学生实现：更新模板")
}

// ============================================================
// 删除模板（POST /templates/{id}/delete）
// ============================================================
/// 删除模板（连同它的所有模板项）
///
/// 【教学：先子后父】
/// DELETE template_items（孩子）→ DELETE templates（父亲）
/// 必须这个顺序，否则删父后子表留孤儿数据。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 验证归属（JOIN phases 查 user_id）
/// 3. 事务：DELETE FROM template_items WHERE template_id = ?
///        → DELETE FROM templates WHERE id = ?
/// 4. commit → 重定向回模板列表
pub async fn template_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 1 步): 学生实现（步骤见上方注释）
    // 提示：删除父表前先确认存在（fetch_optional 检查），
    // 不存在 → Err(NotFound)；然后事务里先删子后删父
    unimplemented!("M3 学生实现：删除模板")
}

// ============================================================
// 第二部分：当日计划（Plan）
// ============================================================

// ============================================================
// 计划列表（GET /phases/{phase_id}/plans）
// ============================================================
/// 显示某阶段下的所有当日计划（按日期倒序）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 验证阶段属于当前用户（同 list_templates）
/// 3. 查计划：SELECT * FROM plans WHERE phase_id = ? ORDER BY date DESC
/// 4. 拼 HTML：表格列出日期 + 备注 + 操作（详情/编辑/删除）
pub async fn list_plans(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    unimplemented!("M3 学生实现：计划列表")
}

// ============================================================
// 新建计划表单页（GET /phases/{phase_id}/plans/new）
// ============================================================
/// 显示新建计划的表单（日期 + 可选模板 + 可选手动加动作）
///
/// 【教学：三种新建方式】
/// 1. 选模板复制（推荐）：下拉框选模板 → 提交后把模板项复制成计划项
/// 2. 手动选动作：checkbox 多选动作（不选模板时用）
/// 3. 两者都有：选模板 + 手动加（M3 先做 1 和 2，3 后续再扩展）
///
/// 表单结构体：
/// struct PlanCreateForm {
///     date: String,            // 'YYYY-MM-DD'，默认今天
///     template_id: Option<i64>, // 选模板（可空）
///     exercise_ids: Vec<i64>,   // 手动选动作（可空）
/// }
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 验证阶段属于当前用户 + 未归档
/// 3. 查模板列表（供下拉）+ 查全部动作（供勾选）
/// 4. 拼 HTML
pub async fn plan_create_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    // 提示：日期默认今天可以用 chrono 或简单拼字符串，
    // 项目里 sqlite 用 datetime('now','localtime')，Rust 端拼 YYYY-MM-DD 即可
    unimplemented!("M3 学生实现：新建计划表单")
}

// ============================================================
// 创建计划（POST /phases/{phase_id}/plans）
// ============================================================
/// 处理新建计划表单提交（含"从模板复制"逻辑）
///
/// 【教学：从模板复制 = 模板项 → 计划项的映射】
/// 选模板时：不手动遍历动作，而是查模板的 template_items，
/// 每个 item 复制成 plan_item：
///   plan_sets   = template_item.plan_sets   （模板项有值就用）
///   plan_reps   = template_item.plan_reps
/// 模板项没值（None）→ 用动作库默认值兜底（查 exercises 表）
///
/// 不选模板时：手动选的 exercise_ids 逐个插入，值取动作库默认
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(phase_id) + Form(form)
/// 2. 验证阶段属于当前用户 + 未归档
/// 3. 【查重】SELECT id FROM plans WHERE phase_id = ? AND date = ?
///    → 已存在 → Err(Validation)（"今天已有计划"）
/// 4. begin 事务 → INSERT plans → 拿 plan_id
/// 5. 分支：有 template_id → 查模板项复制；无 → 用手动选的动作
/// 6. 每个计划项都要解决"组/次从哪来"（模板项 → 动作库默认 兜底链）
/// 7. commit → 重定向到计划详情
pub async fn plan_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
    Form(form): Form<PlanCreateForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 2~3 步): 学生实现（步骤见上方注释）
    // 提示：兜底链 helper 可以写一个函数：
    //   async fn resolve_plan_values(
    //       pool: &SqlitePool,
    //       t_sets: Option<i64>, t_reps: Option<i64>, ex_id: i64,
    //   ) -> Result<(Option<i64>, Option<i64>), AppError>
    //   模板项有值用模板项，没有就查动作库 default_sets/default_reps
    unimplemented!("M3 学生实现：创建计划")
}

// ============================================================
// 计划详情（GET /plans/{id}）
// ============================================================
/// 显示计划详情（日期 + 动作清单 + 每个动作的计划组/次/重量）
///
/// 【教学：跨表查名字】
/// 计划项表只有 exercise_id（数字），页面要显示动作名。
/// 需要把 exercise_id → name：JOIN 或每行再查一次。
/// M3 用"先查所有动作再在内存里配对"（Map 索引）：
///   let ex_map: HashMap<i64, String> = exercises.iter().map(|e| (e.id, e.name)).collect();
///   拼行时 ex_map.get(&item.exercise_id) —— 一次查询换 N 次查询，快
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 查计划 + 验证归属（JOIN phases）
/// 3. 查计划项：SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order
/// 4. 查动作库全部动作 → HashMap 索引 id → name
/// 5. 拼表格：动作名 + 组数 + 次数 + 重量（None 显示 "-"）
pub async fn plan_detail(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    // 提示：None 值显示用 match：
    //   item.plan_sets.map_or("-".to_string(), |v| v.to_string())
    // 或 if let Some(v) = item.plan_sets { ... } else { "-" }
    unimplemented!("M3 学生实现：计划详情")
}

// ============================================================
// 编辑计划表单页（GET /plans/{id}/edit）
// ============================================================
/// 显示编辑计划表单（改日期 + 增删动作 + 改计划值）
///
/// 【教学：编辑子表集合 —— 比"先删后插"更好的是逐项更新】
/// M3 计划编辑提供两件事：
///   a. 改日期、备注（父表 UPDATE）
///   b. 增删动作（子表集合变更，M3 简化为"重新提交整个清单"）
/// 为了简单，M3 采用和模板一样的"先删后插"（事务内）。
/// 注意：计划项删除后，M4 的记录若已关联 plan_item_id 会变孤儿——
/// 所以 M3 里计划编辑仅限"当天未训练"（M4 再加强校验），
/// 现在先实现基本 CRUD，注释里说明这个限制。
pub async fn plan_edit_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    // 提示：表单含 date + note + 动作多选（已选的勾上）
    // 和 template_edit_form 结构几乎一样，只是父表是 plans
    unimplemented!("M3 学生实现：编辑计划表单")
}

// ============================================================
// 更新计划（POST /plans/{id}/edit）
// ============================================================
/// 处理编辑计划表单提交
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id) + Form(form)
/// 2. 验证归属 + 未归档
/// 3. 事务：UPDATE plans SET date/note → DELETE plan_items → 循环 INSERT
/// 4. commit → 重定向回计划详情
pub async fn plan_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Form(form): Form<PlanEditForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    // 提示：注意 date 可能撞 UNIQUE(phase_id, date)——排除自己再查重：
    //   SELECT id FROM plans WHERE phase_id = ? AND date = ? AND id != ?
    unimplemented!("M3 学生实现：更新计划")
}

// ============================================================
// 删除计划（POST /plans/{id}/delete）
// ============================================================
/// 删除计划（连同计划项）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 验证归属
/// 3. 事务：DELETE FROM plan_items WHERE plan_id = ? → DELETE FROM plans WHERE id = ?
/// 4. commit → 重定向回计划列表
pub async fn plan_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError>
{
    // TODO(M3 第 2 步): 学生实现（步骤见上方注释）
    // 提示：先子后父，和 template_delete 一模一样
    unimplemented!("M3 学生实现：删除计划")
}

// ============================================================
// 【表单结构体】—— M3 需要的 Deserialize 结构
// ============================================================

/// 模板创建/编辑表单（名字 + 勾选的动作集合）
#[derive(Deserialize)]
pub struct TemplateCreateForm
{
    pub name: String,
    /// checkbox 同名 → 浏览器提交重复 name 参数 → Vec<i64>
    /// 一个都没勾时这个字段会怎样？axum 对缺失字段默认报错
    /// （需要 #[serde(default)] 才能容忍空——M3 要求必选，可不加）
    pub exercise_ids: Vec<i64>,
}

/// 计划创建表单（日期 + 可选模板 + 可选手动选动作）
#[derive(Deserialize)]
pub struct PlanCreateForm
{
    pub date: String,
    /// Option = 可空：没选模板就是 None
    pub template_id: Option<i64>,
    /// 手动选的动作（不选模板时用；和模板二选一）
    #[serde(default)]
    pub exercise_ids: Vec<i64>,
}

/// 计划编辑表单（日期 + 备注 + 动作集合）
#[derive(Deserialize)]
pub struct PlanEditForm
{
    pub date: String,
    pub note: String,
    #[serde(default)]
    pub exercise_ids: Vec<i64>,
}
