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
use std::collections::{HashMap, HashSet};

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
    // ① 两级验证：阶段必须属于当前用户（数据隔离底线）
    let phase_ret = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    // ② 归档阶段显示"只读"提示（M3 指南 §2.4：归档 = 只读，不能建/改）
    let archived_note = if phase_ret.archived
    {
        "<p style=\"color:red\">⚠️ 该阶段已归档，只读（不能新建/编辑/删除模板）</p>"
    }
    else
    {
        ""
    };

    // ③ 查模板列表 → 每行：名称 + 编辑/删除操作链接（M3 指南第 1 步验收要求）
    //    操作链接用表单 POST（删除是改数据，不能用 GET 链接）
    let template_ret = sqlx::query_as::<_, Template>("SELECT * FROM templates WHERE phase_id = ?")
        .bind(&phase_ret.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?
        .iter()
        .map(|item| {
            format!(
                r#"<tr><td>{tmp_name}</td>
                <td><a href="/templates/{tmp_id}/edit">编辑</a></td>
                <td><form method="post" action="/templates/{tmp_id}/delete"
                style="display:inline"><button type="submit">删除</button></form></td>
                </tr>"#,
                tmp_id = item.id,
                tmp_name = item.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ④ 拼页面（注意：r#"... "# 内部不能再出现裸引号，否则会渲染到页面）
    Ok(Html(format!(
        r#"
                <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
                <h2>训练模板</h2>
                {archived_note}
                    <table border="1"><tr><th>名称</th><th>操作</th></tr>
                        {tmp_content}
                    </table>
                <p><a href="/phases/{phase_id}/templates/new">创建训练模板</a></p>
                <p><a href="/">返回首页</a></p>
            "#,
        archived_note = archived_note,
        tmp_content = template_ret,
        phase_id = phase_ret.id
    )))
}

// ============================================================
// 新建模板表单页（GET /phases/{phase_id}/templates/new）
// ============================================================
/// 显示新建模板的表单（模板名 + 多选动作）
///
/// 【教学：多选（checkbox）表单】
/// 一个模板包含多个动作，表单里每个动作一个勾选框。
///
/// ⚠️ 关键陷阱：checkbox 的 name **不能都用 exercise_ids**！
/// axum 的 Form 用 serde_urlencoded 解析（map 语义）：重复键后值覆盖前值，
/// 实测 `exercise_ids=6&exercise_ids=7` 只剩 7，且 `Vec<i64>` 会 422。
///
/// ✅ 正确做法（本项目采用）：
///   name = 动作 id（唯一键），value = "1"（勾选标记）
///   <input type="checkbox" name="{id}" value="1">
/// 提交后形如：name=模板名&6=1&7=1
/// 结构体用 #[serde(flatten)] 把未匹配键收进 HashMap，再按数字键过滤。
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
    // ① 两级验证：阶段必须属于当前用户
    let phase_ret = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    // ② 查全部动作 → checkbox 行（name = 动作 id，value = 1）
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // ②b 部位筛选下拉框选项：从动作列表去重生成（"全部"用空串表示）
    let mut part_list: Vec<String> = all_exercises
        .iter()
        .map(|ex| ex.body_part.clone())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    part_list.sort();
    let part_options = part_list
        .iter()
        .map(|p| format!(r#"<option value="{p}">{p}</option>"#, p = p))
        .collect::<Vec<String>>()
        .join("\n");

    let checkbox_rows = all_exercises
        .iter()
        .map(|ex| {
            format!(
                // checkbox 的 name 用动作 id（唯一键），value=1（勾选标记）
                // 不能用 name="exercise_ids" 重复键——serde_urlencoded 会覆盖
                // data-part 属性：供前端 JS 按部位显隐过滤
                r#"<label data-part="{part}"><input type="checkbox" name="{id}" value="1"> {name}</label><br>"#,
                part = ex.body_part,
                id = ex.id,
                name = ex.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ③ 拼表单
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>创建训练模板</h2>
        <form method="post" action="/phases/{phase_id}/templates">
            模板名：<input name="name"><br>
            部位筛选：
            <select id="part_filter" onchange="filterByPart()">
                <option value="">全部</option>
                {part_options}
            </select><br>
            <div id="exercise_list">
                {checkbox_rows}
            </div>
            <button type="submit">创建</button>
        </form>
        <p><a href="/phases/{phase_id}/templates">返回模板列表</a></p>
        <script>
            {javascript}
        </script>
        "#,
        phase_id = phase_ret.id,
        part_options = part_options,
        checkbox_rows = checkbox_rows,
        javascript = r#"function filterByPart(){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#exercise_list label').forEach(function(lb){
                    lb.style.display = (part === '' || lb.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                filterByPart();"#
    )))
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
///    → query_scalar::<_, i64> + RETURNING id 拿回模板 id
/// 5. 遍历 form.exercise_ids()，逐个 INSERT template_items（enumerate 生成 sort_order）
/// 6. commit → 重定向回模板列表
///
/// 【教学：表单结构体】
/// checkbox name = 动作 id（唯一键）、value = "1"，#[serde(flatten)] 收 HashMap，
/// exercise_ids() 方法按"能 parse 成 i64 的键"过滤（serde_urlencoded 多选陷阱，见 todo.md §2.1）
pub async fn template_create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
    Form(form): Form<TemplateCreateForm>,
) -> Result<Redirect, AppError>
{
    // ① 验证阶段属于当前用户 + 未归档
    let target_phase =
        sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
            .bind(&phase_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such phase or not your phase".to_string()))?;

    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    let template_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO templates
    (phase_id, name, sort_order) VALUES (?, ?, ?)
    RETURNING id",
    )
    .bind(&phase_id)
    .bind(&form.name)
    .bind(0_i64)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    // form.exercise_ids 是 HashMap<String, String>（flatten 收集的勾选键值对）
    // checkbox name = 动作 id，值 = "1"（勾选标记）
    let ex_ids: Vec<i64> = form.exercise_ids();

    for (idx, ex_id) in ex_ids.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO template_items (template_id, exercise_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&template_id)
        .bind(ex_id) // ex_id 已经是 &i64，不用再 &
        .bind(idx as i64) // ← usize 必须转 i64
        .execute(&mut *tx) // ← 事务要 &mut *tx（Transaction 可变解引用）
        .await
        .map_err(AppError::Database)?;
    }

    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!("/phases/{phase_id}/templates")))
}

// ============================================================
// 编辑模板表单页（GET /templates/{id}/edit）
// ============================================================
/// 显示编辑模板的表单（模板名 + 动作多选，已选的勾上）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(template_id)
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
    Path(template_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // ① 查模板 + 验证归属：JOIN phases 一次性把 user_id 也取出来
    //    模板不属于当前用户 → NotFound（数据隔离）
    let current_template = sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No template found in such user and phase".to_string()))?;

    // ② 查模板已有的动作（只查 exercise_id 一列）
    //    注意返回类型是 i64（跟数据库列类型一致），不是 String！
    let current_template_item_ids = sqlx::query_scalar::<_, i64>(
        "SELECT exercise_id FROM template_items WHERE template_id = ?",
    )
    .bind(&template_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 把 Vec<i64> 转成 HashSet<i64>：判断"这个动作勾没勾"用 O(1) 查找
    // （用 HashSet 而不是 Vec.contains —— 动作多时哈希查找比线性扫描快）
    let selected_item_ids: HashSet<i64> = current_template_item_ids.into_iter().collect();

    // ③ 查【全部】动作（和 create_form 一样）→ 生成所有 checkbox 行
    //    编辑页必须显示全部动作，否则用户没法新增没勾过的动作
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // ③b 部位筛选下拉框选项（从动作列表去重，动态生成）
    let mut part_list: Vec<String> = all_exercises
        .iter()
        .map(|ex| ex.body_part.clone())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    part_list.sort();
    let part_options = part_list
        .iter()
        .map(|p| format!(r#"<option value="{p}">{p}</option>"#, p = p))
        .collect::<Vec<String>>()
        .join("\n");

    let checkbox_rows = all_exercises
        .iter()
        .map(|ex| {
            // checked 是条件字符串：选中的输出 " checked"，没选中的输出 ""
            // 放到 value="1" 后面：value="1" checked> 或 value="1">
            let checked = if selected_item_ids.contains(&ex.id)
            {
                " checked"
            }
            else
            {
                ""
            };
            format!(
                // checkbox 的 name 用动作 id（唯一键）、value=1 —— 和 create_form 同一套约定
                // 这样 POST 提交后 #[serde(flatten)] 能收集到所有勾选
                // data-part 属性：供前端 JS 按部位显隐过滤
                r#"<label data-part="{part}"><input type="checkbox" name="{id}" value="1"{checked}> {name}</label><br>"#,
                part = ex.body_part,
                id = ex.id,
                checked = checked,
                name = ex.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ④ 拼表单：
    //    - action 指向编辑提交地址 /templates/{template_id}/edit（不是创建页！）
    //    - 模板名输入框预填当前名字 value="{name}"
    //    - 按钮文字改成"保存"
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>编辑训练模板</h2>
        <form method="post" action="/templates/{template_id}/edit">
            模板名：<input name="name" value="{name}"><br>
            部位筛选：
            <select id="part_filter" onchange="filterByPart()">
                <option value="">全部</option>
                {part_options}
            </select><br>
            <div id="exercise_list">
                {checkbox_rows}
            </div>
            <button type="submit">保存</button>
        </form>
        <p><a href="/phases/{phase_id}/templates">返回模板列表</a></p>
        <script>
            {javascript}
        </script>
        "#,
        template_id = template_id,
        name = current_template.name,
        phase_id = current_template.phase_id,
        part_options = part_options,
        checkbox_rows = checkbox_rows,
        javascript = r#"function filterByPart(){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#exercise_list label').forEach(function(lb){
                    lb.style.display = (part === '' || lb.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                filterByPart();"#
    )))
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
/// 1. 签名：State + AuthUser + Path(template_id) + Form(form)
/// 2. 验证模板归属当前用户（同 edit_form 的 JOIN 验证）
/// 3. begin 事务 → UPDATE 父表 → DELETE 子表 → 循环 INSERT
/// 4. commit → 重定向回模板列表
pub async fn template_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(template_id): Path<i64>,
    Form(form): Form<TemplateCreateForm>,
) -> Result<Redirect, AppError>
{
    // ① 先查后改：验证模板归属当前用户（JOIN phases 拿 user_id）
    //    顺带拿到 phase_id（重定向要用）——一条查询两个用途
    //    模板不存在或不属于当前用户 → 404，根本不进入事务
    let current_template = sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No template found in such user and phase".to_string()))?;

    // ② 归档阶段不可编辑（改历史数据）
    let target_phase =
        sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
            .bind(&current_template.phase_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    // ③ 开事务：三步"先删后插"要么全成要么全败
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 3.1 更新父表（改名）——只改这一行，不会插入新记录
    sqlx::query("UPDATE templates SET name = ? WHERE id = ?")
        .bind(&form.name)
        .bind(&template_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 3.2 删掉所有旧子表行（先删后插：清空重来，避免"残留旧动作"）
    sqlx::query("DELETE FROM template_items WHERE template_id = ?")
        .bind(&template_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 3.3 重新插入所有勾选的动作（enumerate 生成 sort_order）
    let ex_ids: Vec<i64> = form.exercise_ids();
    for (idx, ex_id) in ex_ids.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO template_items (template_id, exercise_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&template_id)
        .bind(ex_id) // ex_id 已经是 &i64，不用再 &
        .bind(idx as i64) // ← usize 必须转 i64
        .execute(&mut *tx) // ← 事务要 &mut *tx（Transaction 可变解引用）
        .await
        .map_err(AppError::Database)?;
    }

    // ④ 全部成功才提交
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/phases/{phase_id}/templates",
        phase_id = current_template.phase_id
    )))
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
/// 1. 签名：State + AuthUser + Path(template_id)
/// 2. 验证归属（JOIN phases 查 user_id）
/// 3. 事务：DELETE FROM template_items WHERE template_id = ?
///        → DELETE FROM templates WHERE id = ?
/// 4. commit → 重定向回模板列表
pub async fn template_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(template_id): Path<i64>,
) -> Result<Redirect, AppError>
{
    // ① 先查后改：验证模板归属当前用户（JOIN phases 拿 user_id）
    //    顺带拿到 phase_id（重定向要用）——和 template_update 完全一样
    let current_template = sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No template found in such user and phase".to_string()))?;

    // ② 开事务：先删子（template_items）后删父（templates）
    //    顺序不能反：父表被子表引用时先删父会留孤儿数据
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 2.1 删孩子：模板的所有动作项
    sqlx::query("DELETE FROM template_items WHERE template_id = ?")
        .bind(&template_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 2.2 删父亲：模板本身
    sqlx::query("DELETE FROM templates WHERE id = ?")
        .bind(&template_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // ③ 全部成功才提交
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/phases/{phase_id}/templates",
        phase_id = current_template.phase_id
    )))
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
    // ① 两级验证：阶段必须属于当前用户
    let phase_ret = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&state.pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    // ② 归档阶段显示"只读"提示
    let archived_note = if phase_ret.archived
    {
        "<p style=\"color:red\">⚠️ 该阶段已归档，只读（不能新建/编辑/删除计划）</p>"
    }
    else
    {
        ""
    };

    // ③ 查计划列表（日期倒序，最新在前）→ 每行：日期 + 备注 + 操作
    //    删除必须用表单 POST（路由只注册了 post，GET 链接会 405）
    let plan_ret =
        sqlx::query_as::<_, Plan>("SELECT * FROM plans WHERE phase_id = ? ORDER BY date DESC")
            .bind(&phase_ret.id)
            .fetch_all(&state.pool)
            .await
            .map_err(AppError::Database)?
            .iter()
            .map(|item| {
                format!(
                    r#"<tr><td>{plan_dt}</td><td>{plan_note}</td>
                    <td><a href="/plans/{plan_id}">详情</a>
                    <a href="/plans/{plan_id}/edit">编辑</a>
                    <form method="post" action="/plans/{plan_id}/delete"
                    style="display:inline"><button type="submit">删除</button></form></td></tr>"#,
                    plan_id = item.id,
                    plan_dt = item.date,
                    plan_note = item.note
                )
            })
            .collect::<Vec<String>>()
            .join("\n");

    // ④ 拼页面
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>训练计划</h2>
        {archived_note}
        <table border="1"><tr><th>日期</th><th>备注</th><th>操作</th></tr>
            {content}
        </table>
        <p><a href="/phases/{phase_id}/plans/new">创建当日计划</a></p>
        <p><a href="/">返回首页</a></p>
        "#,
        archived_note = archived_note,
        content = plan_ret,
        phase_id = phase_ret.id
    )))
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
    // ① 验证阶段属于当前用户 + 未归档
    let target_phase =
        sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
            .bind(&phase_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    // ② 查该阶段的模板列表（下拉框选项）
    let template_rows = sqlx::query_as::<_, Template>(
        "SELECT * FROM templates WHERE phase_id = ? ORDER BY sort_order, id",
    )
    .bind(&phase_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .iter()
    .map(|t| {
        format!(
            // value = 模板 id，提交后 form.template_id = Some(模板id)
            r#"<option value="{tid}">{tname}</option>"#,
            tid = t.id,
            tname = t.name
        )
    })
    .collect::<Vec<String>>()
    .join("\n");

    // ③ 查全部动作（checkbox 列表）
    //    ⚠️ checkbox name 用动作 id（唯一键）！不能都叫 exercise_ids
    //    （serde_urlencoded map 语义会覆盖，见 PlanCreateForm 注释）
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // ③b 部位筛选下拉框选项（从动作列表去重，动态生成）
    let mut part_list: Vec<String> = all_exercises
        .iter()
        .map(|ex| ex.body_part.clone())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    part_list.sort();
    let part_options = part_list
        .iter()
        .map(|p| format!(r#"<option value="{p}">{p}</option>"#, p = p))
        .collect::<Vec<String>>()
        .join("\n");

    let checkbox_rows = all_exercises
        .iter()
        .map(|ex| {
            format!(
                r#"<label data-part="{part}"><input type="checkbox" name="{id}" value="1"> {name}</label><br>"#,
                part = ex.body_part,
                id = ex.id,
                name = ex.name
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ④ 今天日期：和数据库保持一致用 SQLite 的 localtime（避免 Rust 端时区偏差）
    let today = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // ⑤ 拼表单
    //    【教学：JS 控制"动作勾选区"显隐】
    //    需求：选了模板 → 动作由模板决定，手动勾选没意义，隐藏；
    //          "不选模板，手动选动作" → 显示勾选区。
    //    做法（纯前端，零后端改动）：
    //      1. 模板下拉 <select id="template_id" onchange="toggleManualExercises()">
    //      2. 勾选区包 <div id="manual_exercises">（默认显示）
    //      3. JS：select.value 为空串（"不选模板"）→ 显示，否则隐藏
    //    ⚠️ JS 内容用命名参数 {javascript} 传入 format!（避开 {} 冲突，
    //    与 exercises.rs 的 toggleBarWeight 同款写法）。
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>新建当日计划</h2>
        <form method="post" action="/phases/{phase_id}/plans">
            日期：<input type="date" name="date" value="{today}"><br>
            模板：<select name="template_id" id="template_id" onchange="toggleManualExercises()">
                <option value="">（不选模板，手动选动作）</option>
                {template_rows}
            </select><br>
            <div id="manual_exercises">
                动作（不选模板时手动勾选）：<br>
                部位筛选：
                <select id="part_filter" onchange="filterByPart()">
                    <option value="">全部</option>
                    {part_options}
                </select><br>
                {checkbox_rows}
            </div>
            <button type="submit">创建计划</button>
        </form>
        <p><a href="/phases/{phase_id}/plans">返回计划列表</a></p>
        <script>
            {javascript}
        </script>
        "#,
        phase_id = phase_id,
        today = today,
        template_rows = template_rows,
        part_options = part_options,
        checkbox_rows = checkbox_rows,
        javascript = "function toggleManualExercises(){
                var select = document.getElementById('template_id');
                var box = document.getElementById('manual_exercises');
                box.style.display = (select.value === '') ? '' : 'none';
                }
                function filterByPart(){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#manual_exercises label').forEach(function(lb){
                lb.style.display = (part === '' || lb.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                toggleManualExercises();
                filterByPart();"
    )))
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
    // ① 验证阶段属于当前用户 + 未归档
    let target_phase =
        sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
            .bind(&phase_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    // ② 查重：同阶段同日期只能有一条计划（数据库有 UNIQUE 约束兜底）
    //    提前拦截给用户一个明确的错误，而不是等数据库报 UNIQUE 冲突
    let exists =
        sqlx::query_scalar::<_, i64>("SELECT id FROM plans WHERE phase_id = ? AND date = ?")
            .bind(&phase_id)
            .bind(&form.date)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?;
    if exists.is_some()
    {
        return Err(AppError::Validation(format!(
            "该日期 {} 已有计划，不能重复创建",
            form.date
        )));
    }

    // ③ 兜底链：解析"组/次从哪来"
    //    模板项有值 → 用模板项；没有 → 查动作库默认值（default_sets/default_reps）
    //    返回 (sets, reps) 都是 Option：都没有就保持 None
    async fn resolve_plan_values(
        pool: &SqlitePool,
        t_sets: Option<i64>,
        t_reps: Option<i64>,
        ex_id: i64,
    ) -> Result<(Option<i64>, Option<i64>), AppError>
    {
        if t_sets.is_some() && t_reps.is_some()
        {
            return Ok((t_sets, t_reps));
        }
        let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ?")
            .bind(&ex_id)
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
        let sets = t_sets.or(Some(ex.default_sets));
        let reps = t_reps.or(Some(ex.default_reps));
        Ok((sets, reps))
    }

    // ④ 事务：写两张表（plans 父 + plan_items 子）要么全成要么全败
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 4.1 插入计划（父表），拿回 plan_id
    let plan_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO plans (phase_id, date, note) VALUES (?, ?, '') RETURNING id",
    )
    .bind(&phase_id)
    .bind(&form.date)
    .fetch_one(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    // 4.2 分支：选模板 → 复制模板项；没选 → 手动选的动作
    if let Some(tid) = form.template_id
    {
        // ⑤ 从模板复制：查模板的 template_items，逐个复制成 plan_items
        let template_items = sqlx::query_as::<_, TemplateItem>(
            "SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order",
        )
        .bind(&tid)
        .fetch_all(&mut *tx)
        .await
        .map_err(AppError::Database)?;

        for (idx, ti) in template_items.iter().enumerate()
        {
            let (sets, reps) =
                resolve_plan_values(&state.pool, ti.plan_sets, ti.plan_reps, ti.exercise_id)
                    .await?;
            sqlx::query(
                "INSERT INTO plan_items (plan_id, exercise_id, sort_order, plan_sets, plan_reps)
            VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&plan_id)
            .bind(&ti.exercise_id)
            .bind(idx as i64)
            .bind(sets)
            .bind(reps)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }
    }
    else
    {
        // ⑥ 手动选动作：每个动作从动作库拿默认组/次
        let ex_ids: Vec<i64> = form.exercise_ids();
        for (idx, ex_id) in ex_ids.iter().enumerate()
        {
            let (sets, reps) = resolve_plan_values(&state.pool, None, None, *ex_id).await?;
            sqlx::query(
                "INSERT INTO plan_items (plan_id, exercise_id, sort_order, plan_sets, plan_reps)
            VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&plan_id)
            .bind(ex_id)
            .bind(idx as i64)
            .bind(sets)
            .bind(reps)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }
    }

    // ⑦ 全部成功才提交
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!("/plans/{plan_id}")))
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
/// 1. 签名：State + AuthUser + Path(plan_id)
/// 2. 查计划 + 验证归属（JOIN phases）
/// 3. 查计划项：SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order
/// 4. 查动作库全部动作 → HashMap 索引 id → name
/// 5. 拼表格：动作名 + 组数 + 次数 + 重量（None 显示 "-"）
pub async fn plan_detail(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(plan_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // ① 查计划 + 验证归属（JOIN phases 拿 user_id）
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    // ② 查计划项（按 sort_order 排序）
    let plan_items = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order",
    )
    .bind(&plan_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // ③ 查全部动作 → HashMap 索引（一次查询换 N 次查询）
    //    计划项表只存 exercise_id（数字），页面要显示动作名
    //    先全部查出来建索引，拼行时 O(1) 查找
    let exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;
    let ex_map: HashMap<i64, String> = exercises.iter().map(|e| (e.id, e.name.clone())).collect();

    // ④ 拼表格行
    let item_rows = plan_items
        .iter()
        .map(|item| {
            // 动作名：从索引取，查不到显示 "?"（理论上不会发生）
            let ex_name = ex_map
                .get(&item.exercise_id)
                .cloned()
                .unwrap_or_else(|| "?".to_string());
            // None 值显示 "-"：map_or 把 Option 转成字符串
            let sets = item.plan_sets.map_or("-".to_string(), |v| v.to_string());
            let reps = item.plan_reps.map_or("-".to_string(), |v| v.to_string());
            let weight = item.plan_weight.map_or("-".to_string(), |v| v.to_string());
            format!(
                "<tr><td>{ex_name}</td><td>{sets}</td><td>{reps}</td><td>{weight}</td></tr>",
                ex_name = ex_name,
                sets = sets,
                reps = reps,
                weight = weight,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ⑤ 拼页面
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>计划详情（{plan_date}）</h2>
        <p>备注：{plan_note}</p>
        <table border="1"><tr><th>动作</th><th>组数</th><th>次数</th><th>重量</th></tr>
            {item_rows}
        </table>
        <p><a href="/plans/{plan_id}/edit">编辑</a> |
        <a href="/phases/{phase_id}/plans">返回计划列表</a></p>
        "#,
        plan_date = current_plan.date,
        plan_note = current_plan.note,
        item_rows = item_rows,
        plan_id = current_plan.id,
        phase_id = current_plan.phase_id,
    )))
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
    Path(plan_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    // ① 查计划 + 验证归属（JOIN phases）
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    // ② 查计划已有的计划项 → 建 exercise_id → PlanItem 索引（回显组/次/重）
    //    不能只查 exercise_id 列表了：编辑页要给每行回显 sets/reps/weight
    let current_items = sqlx::query_as::<_, PlanItem>("SELECT * FROM plan_items WHERE plan_id = ?")
        .bind(&plan_id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;
    let item_map: HashMap<i64, PlanItem> = current_items
        .into_iter()
        .map(|i| (i.exercise_id, i))
        .collect();

    // ③ 查全部动作 → 每行 checkbox + 组/次/重三个输入框
    //    【教学：前缀键方案】
    //    一个动作 4 个输入框，键必须唯一（serde_urlencoded map 语义，同名覆盖）：
    //      checkbox：name = 动作 id（如 6），value = "1" —— 勾选标记
    //      组数/次/重：name = "{字段}_{动作id}"（如 sets_6 / reps_6 / weight_6）
    //    提交形如：6=1&7=1&sets_6=4&reps_6=8&weight_6=60
    //    前缀键互不冲突、与数字勾选键也互不冲突，全部进 flatten 的 rest。
    //    value 回显：已选动作显示计划当前值；未选动作预填动作库默认组/次
    //    （勾选即用，不用二次填写）；重量默认空。
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;

    // ③b 部位筛选下拉框选项（从动作列表去重，动态生成）
    let mut part_list: Vec<String> = all_exercises
        .iter()
        .map(|ex| ex.body_part.clone())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect();
    part_list.sort();
    let part_options = part_list
        .iter()
        .map(|p| format!(r#"<option value="{p}">{p}</option>"#, p = p))
        .collect::<Vec<String>>()
        .join("\n");

    let checkbox_rows = all_exercises
        .iter()
        .map(|ex| {
            let item = item_map.get(&ex.id);
            let checked = if item.is_some() { " checked" } else { "" };
            // 回显链：计划项有值 → 用计划项；没有（未选/没设过）→ 动作库默认
            let sets = item
                .and_then(|i| i.plan_sets)
                .map_or(ex.default_sets.to_string(), |v| v.to_string());
            let reps = item
                .and_then(|i| i.plan_reps)
                .map_or(ex.default_reps.to_string(), |v| v.to_string());
            let weight = item
                .and_then(|i| i.plan_weight)
                .map_or(String::new(), |v| v.to_string());
            format!(
                r#"<label data-part="{part}"><input type="checkbox" name="{id}" value="1"{checked}> {name}</label>
                组数<input name="sets_{id}" value="{sets}" size="3">
                次数<input name="reps_{id}" value="{reps}" size="3">
                重量<input name="weight_{id}" value="{weight}" size="3"><br>"#,
                part = ex.body_part,
                id = ex.id,
                checked = checked,
                name = ex.name,
                sets = sets,
                reps = reps,
                weight = weight,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ④ 拼表单（date + note + 动作多选 + 每动作组/次/重）
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>编辑计划</h2>
        <form method="post" action="/plans/{plan_id}/edit">
            日期：<input type="date" name="date" value="{plan_date}"><br>
            备注：<input name="note" value="{plan_note}"><br>
            动作（组/次/重可直接修改）：<br>
            部位筛选：
            <select id="part_filter" onchange="filterByPart()">
                <option value="">全部</option>
                {part_options}
            </select><br>
            <div id="exercise_list">
                {checkbox_rows}
            </div>
            <button type="submit">保存</button>
        </form>
        <p><a href="/phases/{phase_id}/plans">返回计划列表</a></p>
        <script>
            {javascript}
        </script>
        "#,
        plan_id = current_plan.id,
        plan_date = current_plan.date,
        plan_note = current_plan.note,
        part_options = part_options,
        checkbox_rows = checkbox_rows,
        phase_id = current_plan.phase_id,
        javascript = r#"function filterByPart(){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#exercise_list label').forEach(function(lb){
                    lb.style.display = (part === '' || lb.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                filterByPart();"#
    )))
}

// ============================================================
// 更新计划（POST /plans/{id}/edit）
// ============================================================
/// 处理编辑计划表单提交
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(plan_id) + Form(form)
/// 2. 验证归属 + 未归档
/// 3. 事务：UPDATE plans SET date/note → DELETE plan_items → 循环 INSERT
/// 4. commit → 重定向回计划详情
pub async fn plan_update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(plan_id): Path<i64>,
    Form(form): Form<PlanEditForm>,
) -> Result<Redirect, AppError>
{
    // ① 先查后改：验证归属（JOIN phases）→ 拿到 phase_id 供重定向
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    // ② 归档阶段不可编辑
    let target_phase =
        sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
            .bind(&current_plan.phase_id)
            .bind(&user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;
    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    // ③ 查重：date 撞 UNIQUE(phase_id, date) 时排除自己
    //    编辑时日期可能没变（自己占着这个日期）——必须 AND id != ?
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT id FROM plans WHERE phase_id = ? AND date = ? AND id != ?",
    )
    .bind(&current_plan.phase_id)
    .bind(&form.date)
    .bind(&plan_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?;
    if exists.is_some()
    {
        return Err(AppError::Validation(format!(
            "该日期 {} 已有计划，不能重复",
            form.date
        )));
    }

    // ④ 事务：先删后插（和模板 update 一样的套路）
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 4.1 更新父表（日期 + 备注）
    sqlx::query("UPDATE plans SET date = ?, note = ? WHERE id = ?")
        .bind(&form.date)
        .bind(&form.note)
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 4.2 删掉所有旧计划项
    sqlx::query("DELETE FROM plan_items WHERE plan_id = ?")
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 4.3 重新插入勾选的动作
    //    组/次/重直接来自表单（编辑页已回显当前值，未选动作预填默认值），
    //    空字符串 → None → 存 NULL（plan_detail 显示 "-"）
    let ex_ids: Vec<i64> = form.exercise_ids();
    for (idx, ex_id) in ex_ids.iter().enumerate()
    {
        sqlx::query(
            "INSERT INTO plan_items (plan_id, exercise_id, sort_order, plan_sets, plan_reps, plan_weight)
            VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&plan_id)
        .bind(ex_id)
        .bind(idx as i64)
        .bind(form.plan_sets(*ex_id))
        .bind(form.plan_reps(*ex_id))
        .bind(form.plan_weight(*ex_id))
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    }

    // ⑤ 提交
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/plans/{plan_id}",
        plan_id = current_plan.id
    )))
}

// ============================================================
// 删除计划（POST /plans/{id}/delete）
// ============================================================
/// 删除计划（连同计划项）
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(plan_id)
/// 2. 验证归属
/// 3. 事务：DELETE FROM plan_items WHERE plan_id = ? → DELETE FROM plans WHERE id = ?
/// 4. commit → 重定向回计划列表
pub async fn plan_delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(plan_id): Path<i64>,
) -> Result<Redirect, AppError>
{
    // ① 先查后改：验证归属（JOIN phases）→ 拿 phase_id 供重定向
    let current_plan = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found in such user and phase".to_string()))?;

    // ② 事务：先删子（plan_items）后删父（plans）
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    sqlx::query("DELETE FROM plan_items WHERE plan_id = ?")
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    sqlx::query("DELETE FROM plans WHERE id = ?")
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/phases/{phase_id}/plans",
        phase_id = current_plan.phase_id
    )))
}

// ============================================================
// 【表单结构体】—— M3 需要的 Deserialize 结构
// ============================================================

/// 模板创建/编辑表单（名字 + 勾选的动作集合）
#[derive(Deserialize)]
pub struct TemplateCreateForm
{
    pub name: String,
    /// 勾选的动作 id 集合：checkbox 的 name 直接用动作 id，值是 "1"
    ///
    /// ⚠️ 不能用 exercise_ids: Vec<i64> 或同名重复键！
    /// axum 的 Form 用 serde_urlencoded 解析，它是 **map 语义**：
    /// 重复键后值覆盖前值（实测 exercise_ids=6&exercise_ids=7 → 只剩 7），
    /// 无法收集成数组（实测报 422: invalid type: string "6", expected a sequence）。
    ///
    /// 正确做法：checkbox name = 动作 id（唯一键），#[serde(flatten)] 收集全部，
    /// handler 里按"能解析成 i64 的键"过滤出选中的动作。
    #[serde(flatten)]
    pub rest: HashMap<String, String>,
}

impl TemplateCreateForm
{
    /// 从 flatten 的键值对里提取选中的动作 id 列表（保持表单提交顺序）
    /// checkbox name 是 "6"、"7"…，值是 "1"（勾选标记）
    pub fn exercise_ids(&self) -> Vec<i64>
    {
        self.rest
            .iter()
            .filter_map(|(k, v)| {
                if v == "1"
                {
                    k.parse::<i64>().ok()
                }
                else
                {
                    None
                }
            })
            .collect()
    }
}

/// 计划创建表单（日期 + 可选模板 + 可选手动选动作）
#[derive(Deserialize)]
pub struct PlanCreateForm
{
    pub date: String,
    /// Option = 可空：没选模板就是 None（下拉框没选 → 不提交 template_id 键）
    pub template_id: Option<i64>,
    /// 手动选的动作集合：checkbox 的 name 直接用动作 id，值是 "1"
    ///
    /// ⚠️【serde_urlencoded 多选陷阱】（模板表单踩过的坑，这里再讲一遍）
    /// axum 的 Form<T> 底层用 serde_urlencoded 解析，它是 **map 语义**：
    ///   - 重复键：后值覆盖前值
    ///     `exercise_ids=6&exercise_ids=7` → 解析结果只剩 `7`
    ///   - 想用 `exercise_ids: Vec<i64>` 收集数组：直接 422 报错
    ///     `invalid type: string "6", expected a sequence`
    ///   - 加 `[]` 后缀（`exercise_ids[]=6`）也不生效
    /// 结论：**同名重复键无法收集成数组**，这是 serde_urlencoded 的固有行为。
    ///
    /// ✅ 本项目方案：checkbox name = 动作 id（唯一键），value = "1"
    ///   <input type="checkbox" name="6" value="1"> 卧推
    ///   <input type="checkbox" name="7" value="1"> 深蹲
    /// 提交后形如：date=2026-08-09&template_id=2&6=1&7=1
    /// 结构体用 #[serde(flatten)] 把未匹配键收进 HashMap，再按数字键过滤。
    #[serde(flatten)]
    pub rest: HashMap<String, String>,
}

impl PlanCreateForm
{
    /// 从 flatten 的键值对里提取选中的动作 id 列表
    /// checkbox name 是 "6"、"7"…，值是 "1"（勾选标记）
    pub fn exercise_ids(&self) -> Vec<i64>
    {
        self.rest
            .iter()
            .filter_map(|(k, v)| {
                if v == "1"
                {
                    k.parse::<i64>().ok()
                }
                else
                {
                    None
                }
            })
            .collect()
    }
}

/// 计划编辑表单（日期 + 备注 + 动作集合 + 每动作组/次/重）
#[derive(Deserialize)]
pub struct PlanEditForm
{
    pub date: String,
    pub note: String,
    /// ⚠️ 多选陷阱：和 TemplateCreateForm / PlanCreateForm 一样，
    /// 不能用 exercise_ids: Vec<i64>（serde_urlencoded map 语义，重复键覆盖）
    /// 用 flatten 收集所有未匹配键，再按规则解析：
    ///   - 纯数字键（"6"）值 == "1" → 勾选的动作 id
    ///   - "{前缀}_{动作id}"（"sets_6"）→ 该动作的组/次/重
    #[serde(flatten)]
    pub rest: HashMap<String, String>,
}

impl PlanEditForm
{
    /// 从 flatten 的键值对里提取选中的动作 id 列表
    pub fn exercise_ids(&self) -> Vec<i64>
    {
        self.rest
            .iter()
            .filter_map(|(k, v)| {
                if v == "1"
                {
                    k.parse::<i64>().ok()
                }
                else
                {
                    None
                }
            })
            .collect()
    }

    /// 【教学：前缀键方案】
    /// 一个动作有 4 个输入：勾选标记（name = {动作id}）+ 组/次/重（name = {字段}_{动作id}）。
    /// serde_urlencoded 是 map 语义，同名键会覆盖——所以键必须唯一。
    /// 做法：键 = "{前缀}_{动作id}"（sets_6 / reps_6 / weight_6），
    /// 与 checkbox 的数字键（6）互不冲突，全部进 flatten 的 rest。
    /// 提交形如：6=1&7=1&sets_6=4&reps_6=8&weight_6=60
    ///
    /// 泛型 T: FromStr —— i64（组/次）和 f64（重量）共用一套解析逻辑
    fn plan_value<T>(&self, prefix: &str, ex_id: i64) -> Option<T>
    where
        T: std::str::FromStr,
    {
        self.rest
            .get(&format!("{prefix}_{ex_id}"))
            .and_then(|v| v.trim().parse::<T>().ok())
    }

    /// 组数（键 sets_{id}；空字符串 → None，即存 NULL）
    pub fn plan_sets(&self, ex_id: i64) -> Option<i64>
    {
        self.plan_value("sets", ex_id)
    }

    /// 次数（键 reps_{id}；空字符串 → None）
    pub fn plan_reps(&self, ex_id: i64) -> Option<i64>
    {
        self.plan_value("reps", ex_id)
    }

    /// 重量 kg（键 weight_{id}；空字符串 → None）
    pub fn plan_weight(&self, ex_id: i64) -> Option<f64>
    {
        self.plan_value("weight", ex_id)
    }
}
