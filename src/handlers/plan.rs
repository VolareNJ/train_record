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
//   POST /templates/{id}/sort                → 模板排序（template_sort）【M4 修订】
//   POST /templates/{id}/items/{item_id}/move
//                                           → 模板项上移/下移（template_item_move）【M4 修订】
//
// 二、当日计划（Plan）—— 某一天的训练安排
//   GET  /phases/{phase_id}/plans            → 计划列表（list_plans）
//   GET  /phases/{phase_id}/plans/new        → 新建计划表单（plan_create_form）
//   POST /phases/{phase_id}/plans            → 创建计划（plan_create）
//   GET  /plans/{id}                         → 计划详情 + 编辑（plan_detail）【M4 修订：原 plan_edit_form 并入】
//   POST /plans/{id}/edit                    → 更新计划（plan_update）
//   POST /plans/{id}/delete                  → 删除计划（plan_delete）
//   POST /plans/{id}/items/{item_id}/move    → 计划项上移/下移（plan_item_move）【M4 修订】
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
    handlers::{auth::AuthUser, stats::CalQuery},
    models::{Exercise, Phase, Plan, PlanItem, Template, TemplateItem, group_by_body_part},
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
// 【教学：部位筛选 JS —— 4 个表单页共用（模板/计划 × 创建/编辑）】
// ============================================================
// 需求：表单页的"部位筛选"下拉框选中某部位后，只显示该部位的动作行。
//
// ⚠️【踩坑：筛选后"表格不连续"（空白行）】
// 早期版本每行是 <label>...</label><br>：JS 把 label 设 display:none，
// 但 <br> 是 label 的**兄弟节点**（在 label 外面），隐藏 label 后 <br> 仍占位，
// 于是中间出现一排排空行，筛选结果看起来"断断续续"。
// ✅ 修复：每行用块级 <div class="ex-row"> 包裹（div 自身换行），
// JS 隐藏整个 div → 不留任何残留空白，列表连续。
//
// ⚠️【踩坑：筛选后隐藏行里的输入框仍会提交】
// display:none 的元素依然在 form 里，其 name/value 照常提交。
// 对模板/计划创建页没问题（未勾选动作本来就不提交勾选标记）。
// 但**编辑计划页**的详情编辑框（mode_{id}/rest_{id}/...）是每行都渲染的，
// 如果隐藏行也提交这些键，后端 plan_update 会因为 exercise_ids 不含
// 该动作而忽略这些键——无副作用，但为保险，编辑页只对**已勾选**动作
// 显示编辑框（见 plan_edit_form 的 JS）。
//
// 【教学：一个 JS 两个职责 —— 传 id 参数复用】
// 模板页筛选范围是 #exercise_list，计划创建页是 #manual_exercises。
// 同一个函数用参数指定容器 id，避免复制粘贴两份几乎一样的 JS：
//   function filterByPart(listId) {
//       var part = document.getElementById('part_filter').value;
//       document.querySelectorAll('#' + listId + ' .ex-row').forEach(...)
//   }
// 所有页面统一 .ex-row 类名 → 一套 JS 全适用。

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

    // ③ 查模板列表 → 每行：名称 + 排序（上移/下移）+ 编辑/删除操作
    //     【M4 修订：模板排序】templates.sort_order 从预留字段变成实际字段，
    //     列表里每行给"上移/下移"小表单（POST /templates/{id}/sort?dir=up|down），
    //     handler 在同一阶段内交换相邻模板的 sort_order。
    //     操作链接用表单 POST（删除是改数据，不能用 GET 链接）
    let template_ret = sqlx::query_as::<_, Template>(
        "SELECT * FROM templates WHERE phase_id = ? ORDER BY sort_order, id",
    )
    .bind(&phase_ret.id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .iter()
    .map(|item| {
        format!(
            r#"<tr><td>{tmp_name}</td>
                <td>
                <form method="post" action="/templates/{tmp_id}/sort?dir=up"
                style="display:inline"><button type="submit">↑</button></form>
                <form method="post" action="/templates/{tmp_id}/sort?dir=down"
                style="display:inline"><button type="submit">↓</button></form>
                </td>
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
                    <table border="1"><tr><th>名称</th><th>排序</th><th>操作</th></tr>
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
                // class="ex-row"：块级 div 整行，JS 按部位隐藏时不残留空行
                // data-part 属性：供前端 JS 按部位显隐过滤
                r#"<div class="ex-row" data-part="{part}"><label><input type="checkbox" name="{id}" value="1"> {name}</label></div>"#,
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
            <select id="part_filter" onchange="filterByPart('exercise_list')">
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
        javascript = r#"function filterByPart(listId){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#' + listId + ' .ex-row').forEach(function(row){
                    row.style.display = (part === '' || row.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                filterByPart('exercise_list');"#
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
            .ok_or_else(|| AppError::NotFound("No such phase in your profile".to_string()))?;

    if target_phase.archived
    {
        return Err(AppError::Forbidden(
            "Can not edit archived phase".to_string(),
        ));
    }

    if form.exercise_ids().is_empty()
    {
        return Err(AppError::Validation("至少选择一个动作".to_string()));
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

    // 【M5 第 6 步打磨项：空动作校验（todo.md §1.2）】
    // 一个动作都不勾选 → ex_ids 为空 Vec → 生成"空壳模板"（0 个 template_items）。
    // 这里加校验：空 → 返回 Validation 错误"至少选择一个动作"。
    // ⚠️ 校验要在开事务之前（tx 已 begin 了就在这之前判断——检查上面
    // 事务 begin 的位置，把校验挪到 begin 之前更干净：既省连接又避免空事务）。
    // 提示：ex_ids.is_empty() 判断即可。

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
/// 显示编辑模板的表单（模板名 + 已选动作表格 + 添加动作 + 排序）
///
/// 【M4 修订：表格化改造（任务 1）】
/// 旧版：模板名 + 全量动作 checkbox 列表（勾选=选中）。
/// 新版：以"表格"形式呈现已选动作（动作名 | 排序 | 删除），
///   上面修改模板名，下面"添加动作"（身体部位下拉 + 动作下拉 + 添加按钮）。
///   每行有上移/下移按钮（POST /templates/{id}/items/{item_id}/move）。
///
/// 实现步骤：
/// 1. 签名：State + AuthUser + Path(template_id)
/// 2. 查模板 + 验证归属（JOIN phases）
/// 3. 查模板已有的动作项（按 sort_order 排序，连同 item id）
/// 4. 查全部动作 → 两个下拉框选项（部位 + 动作）
/// 5. 拼表单：模板名 input + 表格（hidden checkbox name={id} value=1 checked
///    + ↑↓ + 删除）+ 添加动作区 + 保存
///    动作数据嵌入 JSON（EX_OPTIONS），JS 的 addRow 克隆行模板
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

    // ② 查模板已有的动作项（带 item id，供上移/下移路由用）
    let current_items = sqlx::query_as::<_, TemplateItem>(
        "SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order, id",
    )
    .bind(&template_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let selected_item_ids: HashSet<i64> = current_items.iter().map(|i| i.exercise_id).collect();

    // ③ 查【全部】动作（供下拉框 + 表格行名 + JSON）
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;
    let ex_map: HashMap<i64, Exercise> = all_exercises.iter().map(|e| (e.id, e.clone())).collect();

    // ③b 部位下拉框选项（从动作列表去重，动态生成）
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

    // ③c 动作下拉框选项：value = 动作 id，文字 = 名称（JS 按部位过滤）
    let ex_options = all_exercises
        .iter()
        .map(|ex| {
            format!(
                r#"<option value="{id}" data-part="{part}">{name}</option>"#,
                id = ex.id,
                part = ex.body_part,
                name = ex.name,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ③d 表格行（已选动作，按 sort_order 排，再按 body_part 分组）
    //     【M4 修订：分组显示（单表 + 分组标题行）】
    //     先按 sort_order 拼好"裸行"（含 data-part 供 JS 归组），
    //     再用 group_by_body_part 分组（组间按 BODY_PART_ORDER 常量排序，
    //     组内保持 sort_order 顺序）。
    //     每组渲染一行 <tr class="group-header" data-part="部位"> 作分节标题，
    //     后跟本组行 —— 单表格内分区，JS addRow 只需在组头行后插入（见下）。
    let raw_rows = current_items
        .iter()
        .map(|item| {
            let ex = ex_map.get(&item.exercise_id);
            let ex_name = ex
                .map(|e| e.name.clone())
                .unwrap_or_else(|| "?".to_string());
            let part = ex
                .map(|e| e.body_part.clone())
                .unwrap_or_else(|| "未分组".to_string());
            let row = format!(
                r#"<tr id="ex-row-{ex_id}" data-part="{part}">
                <td><input type="checkbox" name="{ex_id}" value="1" checked hidden>
                {ex_name}</td>
                <td>
                <button type="button" onclick="submitMove('/templates/{template_id}/items/{item_id}/move?dir=up')">↑</button>
                <button type="button" onclick="submitMove('/templates/{template_id}/items/{item_id}/move?dir=down')">↓</button>
                </td>
                <td><button type="button" onclick="removeRow({ex_id})">删除</button></td>
                </tr>"#,
                ex_id = item.exercise_id,
                item_id = item.id,
                template_id = template_id,
                ex_name = ex_name,
                part = part,
            );
            (part, row)
        })
        .collect::<Vec<_>>();
    // 分组渲染：组头行 + 组内行（colspan = 3 列）
    // 【M4 修订：组间顺序来自配置 AppConfig.body_part_order（环境变量可配）】
    let item_rows = group_by_body_part(raw_rows.into_iter(), &state.config.body_part_order)
        .iter()
        .map(|(part, rows)| {
            format!(
                r#"<tr class="group-header" data-part="{part}"><td colspan="3">{part}</td></tr>
{rows}"#,
                part = part,
                rows = rows.join("\n"),
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ③e 动作数据 JSON（JS 添加动作时用：名称/部位/默认组次）
    //     serde_json 序列化：动作名可能含引号等特殊字符，必须 JSON 转义
    let ex_data_json = all_exercises
        .iter()
        .map(|ex| {
            serde_json::json!({
                "id": ex.id,
                "name": ex.name,
                "part": ex.body_part,
            })
        })
        .collect::<Vec<_>>();
    let ex_data_json = serde_json::to_string(&ex_data_json)
        .map_err(|_| AppError::Validation("动作数据序列化失败".to_string()))?;

    // ③f 部位顺序 JSON（JS 动态组头按配置顺序插入，与后端 group_by_body_part 一致）
    let body_part_order_json = serde_json::to_string(&state.config.body_part_order)
        .map_err(|_| AppError::Validation("部位顺序序列化失败".to_string()))?;

    // ④ 拼表单：
    //    - action 指向编辑提交地址 /templates/{template_id}/edit（不是创建页！）
    //    - 模板名输入框预填当前名字 value="{name}"
    //    - 表格：已选动作行（hidden checkbox 保证提交后后端收到勾选）
    //    - 添加动作区：部位下拉 + 动作下拉 + 添加按钮（JS addRow）
    //    - 每行 ↑↓：表单用 form 属性关联外部隐藏 form（避免 form 嵌套）
    //    【M4 修订：单表格 + 组头行分组；JS 用 PART_HEADER_MAP 记录
    //      "部位 → 组头行 id"，addRow 新行插到对应组头行之后】
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>编辑训练模板</h2>
        <form method="post" action="/templates/{template_id}/edit">
            模板名：<input name="name" value="{name}" required><br>
            <table border="1">
                <tr><th>动作</th><th>排序</th><th>操作</th></tr>
                {item_rows}
            </table>
            <button type="submit">保存</button>
        </form>
        <p>添加动作：
            <select id="part-select" onchange="filterExByPart()">
                <option value="">全部</option>
                {part_options}
            </select>
            <select id="ex-select">
                {ex_options}
            </select>
            <button type="button" onclick="addRow()">添加</button>
        </p>
        <p><a href="/phases/{phase_id}/templates">返回模板列表</a></p>
        <script>
            var EX_DATA = {ex_data_json};
            function exById(id) {{
                for (var i = 0; i < EX_DATA.length; i++) {{
                    if (String(EX_DATA[i].id) === String(id)) return EX_DATA[i];
                }}
                return null;
            }}
            /* 上移/下移：页面级动态 form（不能直接嵌 <tr> 里，HTML 解析器会忽略） */
            function submitMove(url) {{
                var f = document.createElement('form');
                f.method = 'post';
                f.action = url;
                f.style.display = 'none';
                document.body.appendChild(f);
                f.submit();
            }}
            function filterExByPart() {{
                var part = document.getElementById('part-select').value;
                document.querySelectorAll('#ex-select option').forEach(function(opt) {{
                    opt.style.display = (part === '' || opt.getAttribute('data-part') === part) ? '' : 'none';
                }});
            }}
            /* 部位 → 组头行 id：addRow 插行时定位目标组 */
            /* 【M4 修订：部位标准顺序】动态添加"新部位"时组头也按
               配置顺序（BODY_PART_ORDER 环境变量，默认腿→背→胸→核心→手臂→肩）
               插入，不在表内 → 末尾 */
            var PART_ORDER = {body_part_order_json};
            function partRank(p) {{
                var i = PART_ORDER.indexOf(p);
                return i === -1 ? PART_ORDER.length : i;
            }}
            var PART_HEADER_MAP = {{}};
            function initPartHeaders() {{
                PART_HEADER_MAP = {{}};
                document.querySelectorAll('tr.group-header').forEach(function(h) {{
                    PART_HEADER_MAP[h.getAttribute('data-part')] = h;
                }});
            }}
            initPartHeaders();
            function addRow() {{
                var sel = document.getElementById('ex-select');
                var id = sel.value;
                if (!id) return;
                if (document.getElementById('ex-row-' + id)) return; // 已在表格中
                var ex = exById(id);
                if (!ex) return;
                var tr = document.createElement('tr');
                tr.id = 'ex-row-' + id;
                tr.setAttribute('data-part', ex.part);
                // 新行还没有 item_id，无法上移/下移 → 排序单元格留空占位
                // （若放"待保存"按钮则必须等保存后才有 item_id，得不偿失）
                tr.innerHTML =
                    '<td><input type="checkbox" name="' + id + '" value="1" checked hidden>' +
                    escapeHtml(ex.name) + '</td>' +
                    '<td></td>' +
                    '<td><button type="button" onclick="removeRow(' + id + ')">删除</button></td>';
                // 插到该部位的组头行之后（组头不存在 → 按常量顺序新建组头）
                var header = PART_HEADER_MAP[ex.part];
                if (header) {{
                    header.parentNode.insertBefore(tr, header.nextSibling);
                }} else {{
                    var table = document.querySelector('table');
                    var tbody = table.tBodies[0] || table; // 浏览器隐式 tbody
                    var newHeader = document.createElement('tr');
                    newHeader.className = 'group-header';
                    newHeader.setAttribute('data-part', ex.part);
                    newHeader.innerHTML = '<td colspan="3">' + escapeHtml(ex.part) + '</td>';
                    // 找第一个标准顺序比它靠后的已有组头 → 插到其前面；否则插末尾
                    var after = null;
                    document.querySelectorAll('tr.group-header').forEach(function(h) {{
                        if (after === null && partRank(h.getAttribute('data-part')) > partRank(ex.part)) {{
                            after = h;
                        }}
                    }});
                    if (after) {{
                        tbody.insertBefore(newHeader, after);
                    }} else {{
                        tbody.appendChild(newHeader);
                    }}
                    tbody.insertBefore(tr, newHeader.nextSibling);
                    PART_HEADER_MAP[ex.part] = newHeader;
                }}
                // 重新筛选下拉框（保持当前部位过滤）
                filterExByPart();
            }}
            function removeRow(id) {{
                var row = document.getElementById('ex-row-' + id);
                if (row) row.remove();
            }}
            function escapeHtml(s) {{
                return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
            }}
        </script>
        "#,
        template_id = template_id,
        name = current_template.name,
        phase_id = current_template.phase_id,
        part_options = part_options,
        ex_options = ex_options,
        item_rows = item_rows,
        ex_data_json = ex_data_json,
        body_part_order_json = body_part_order_json,
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

    if form.exercise_ids().is_empty()
    {
        return Err(AppError::Validation("至少选择一个动作".to_string()));
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

    // 3.1.5 【M4 修订：保留旧 sort_order（排序修复）】
    //     旧版"先删后插"用 enumerate 重新编号 → 用户在编辑页调的排序全丢！
    //     而且 exercise_ids() 来自 HashMap，顺序本就不定。
    //     修复：删前把 (exercise_id → sort_order) 存下来，INSERT 时沿用旧值；
    //     新添加的动作排末尾（MAX+1 起）。
    let old_items = sqlx::query_as::<_, TemplateItem>(
        "SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order, id",
    )
    .bind(&template_id)
    .fetch_all(&mut *tx)
    .await
    .map_err(AppError::Database)?;
    let old_order: HashMap<i64, i64> = old_items
        .iter()
        .map(|i| (i.exercise_id, i.sort_order))
        .collect();
    let base_order = old_order.values().copied().max().unwrap_or(-1);

    // 3.2 删掉所有旧子表行（先删后插：清空重来，避免"残留旧动作"）
    sqlx::query("DELETE FROM template_items WHERE template_id = ?")
        .bind(&template_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 3.3 重新插入所有勾选的动作（enumerate 生成 sort_order）
    let ex_ids: Vec<i64> = form.exercise_ids();
    let mut new_idx: i64 = 0;
    for ex_id in ex_ids
    {
        // 旧动作沿用旧 sort_order；新动作从 base_order+1 起递增
        let sort_order = match old_order.get(&ex_id)
        {
            Some(o) => *o,
            None =>
            {
                new_idx += 1;
                base_order + new_idx
            },
        };
        sqlx::query(
            "INSERT INTO template_items (template_id, exercise_id, sort_order) VALUES (?, ?, ?)",
        )
        .bind(&template_id)
        .bind(ex_id) // ex_id 已经是 &i64，不用再 &
        .bind(sort_order)
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
    Query(query): Query<CalQuery>,
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

    // ③ 【M6 修订：日历模式（与 history 同款）——取消列表显示】
    //    有计划的日期 → 绿色可点击；无计划 → 灰色。
    //    批量查"日期 → 计划 id"映射（一次查询，避免逐日 N+1）。
    let plan_date_map: HashMap<String, i64> =
        sqlx::query_as::<_, (String, i64)>("SELECT date, id FROM plans WHERE phase_id = ?")
            .bind(&phase_ret.id)
            .fetch_all(&state.pool)
            .await
            .map_err(AppError::Database)?
            .into_iter()
            .collect();

    // 目标年月：query 参数优先，默认当前年月（与 history 同款）
    let now_ym =
        sqlx::query_scalar::<_, String>("SELECT strftime('%Y-%m', date('now','localtime'))")
            .fetch_one(&state.pool)
            .await
            .map_err(AppError::Database)?;
    let target_ym = match (query.year, query.month)
    {
        (Some(y), Some(m)) => format!("{y}-{m}"),
        _ => now_ym.clone(),
    };
    let target_year = target_ym[..4].to_string();
    let target_month = target_ym[5..7].to_string();

    // 年份选项（从计划日期去重 + 目标年兜底）
    let mut year_set: HashSet<String> =
        plan_date_map.keys().map(|dt| dt[..4].to_string()).collect();
    year_set.insert(target_year.clone());
    let mut year_list: Vec<String> = year_set.into_iter().collect();
    year_list.sort_by(|a, b| b.cmp(a));
    let year_options = year_list
        .iter()
        .map(|y| {
            format!(
                r#"<option value="{y}"{sel}>{y}</option>"#,
                sel = if *y == target_year { " selected" } else { "" },
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // 月份选项 01-12，selected 目标月
    let month_options = (1..=12)
        .map(|m| {
            let mm = format!("{m:02}");
            format!(
                r#"<option value="{mm}"{sel}>{mm}月</option>"#,
                sel = if mm == target_month { " selected" } else { "" },
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // 目标月天数（SQLite 日期运算，与 history 同款）
    let days_in_month = sqlx::query_scalar::<_, i64>(
        "SELECT CAST(strftime('%d', date(?, '+1 month', '-1 day')) AS INTEGER)",
    )
    .bind(format!("{target_ym}-01"))
    .fetch_one(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // 日历单元格：有计划的日期 → 绿色链接到 plan_detail；否则灰色
    // 【M6 修订：删除按钮移到 plan_detail 下方（PRG 回训练计划）】
    let cells = (1..=days_in_month)
        .map(|day| {
            let date_str = format!("{target_ym}-{day:02}");
            let cell = match plan_date_map.get(&date_str)
            {
                Some(pid) => format!(
                    r#"<td style="background-color:#b7e4b0"><a href="/plans/{pid}">{day}</a></td>"#
                ),
                None => format!(r#"<td style="background-color:#dddddd">{day}</td>"#),
            };
            if day % 7 == 0 && day != days_in_month
            {
                format!("{cell}</tr><tr>")
            }
            else
            {
                cell
            }
        })
        .collect::<String>();

    // ④ 拼页面（日历 + 年月导航 + 创建入口）
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>训练计划</h2>
        {archived_note}
        <p>年份：
        <select id="cal-year-filter" onchange="changeCalMonth()">
            {year_options}
        </select>
        月份：
        <select id="cal-month-filter" onchange="changeCalMonth()">
            {month_options}
        </select></p>
        <h3>日历（绿色 = 有计划）</h3>
        <table border="1">
        <tr><th>一</th><th>二</th><th>三</th><th>四</th><th>五</th><th>六</th><th>日</th></tr>
        <tr>{cells}</tr>
        </table>
        <p><a href="/phases/{phase_id}/plans/new">创建当日计划</a></p>
        <p><a href="/">返回首页</a></p>
        <script>
            function changeCalMonth(){{
                var y = document.getElementById('cal-year-filter').value;
                var m = document.getElementById('cal-month-filter').value;
                if (y && m) {{ window.location.href = '/phases/{phase_id}/plans?year=' + y + '&month=' + m; }}
            }}
        </script>
        "#,
        archived_note = archived_note,
        year_options = year_options,
        month_options = month_options,
        cells = cells,
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
                // class="ex-row"：块级 div 整行，JS 按部位隐藏时不残留空行
                r#"<div class="ex-row" data-part="{part}"><label><input type="checkbox" name="{id}" value="1"> {name}</label></div>"#,
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
            备注：<input name="note" value=""><br>
            模板：<select name="template_id" id="template_id" onchange="toggleManualExercises()">
                <option value="">（不选模板，手动选动作）</option>
                {template_rows}
            </select><br>
            <div id="manual_exercises">
                动作（不选模板时手动勾选）：<br>
                部位筛选：
                <select id="part_filter" onchange="filterByPart('manual_exercises')">
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
                function filterByPart(listId){
                var part = document.getElementById('part_filter').value;
                document.querySelectorAll('#' + listId + ' .ex-row').forEach(function(row){
                row.style.display = (part === '' || row.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                toggleManualExercises();
                filterByPart('manual_exercises');"
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

    // ③ 兜底链：解析"组/次/计重方式从哪来"
    //    模板项有值 → 用模板项；没有 → 查动作库默认值
    //    （default_sets/default_reps/default_mode/bar_weight/key_points）
    //    返回的都是 Option：都没有就保持 None
    //    ⚠️ 注意：template_items 表没有 plan_mode/plan_bar_weight/plan_key_points 列
    //    （模板层不做计重预设），所以创建计划时这三项永远落回动作库默认。
    //    plan_rest 同理：动作库没有默认休息 → None（record_form 让用户填）。
    async fn resolve_plan_values(
        pool: &SqlitePool,
        t_sets: Option<i64>,
        t_reps: Option<i64>,
        ex_id: i64,
    ) -> Result<
        (
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<f64>,
            Option<String>,
        ),
        AppError,
    >
    {
        let ex = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ?")
            .bind(&ex_id)
            .fetch_one(pool)
            .await
            .map_err(AppError::Database)?;
        let sets = t_sets.or(Some(ex.default_sets));
        let reps = t_reps.or(Some(ex.default_reps));
        let mode = Some(ex.default_mode);
        let bar_weight = Some(ex.bar_weight);
        let key_points = Some(ex.key_points);
        Ok((sets, reps, mode, bar_weight, key_points))
    }

    // 【M5 第 6 步打磨项：空动作校验（todo.md §1.2）】
    // plan_create 有两类动作来源：
    //   ① form.template_id 选了模板 → 复制模板项（模板自身已有校验，不会空）
    //   ② 没选模板 → 用 form.exercise_ids()（手动勾选）
    // 校验：template_id 为 None 且 exercise_ids() 为空 → Validation"至少选择一个动作"。
    // ⚠️ 放在 ④ 事务 begin 之前，避免空动作也开事务插一条空壳计划。
    if form.template_id.is_none() && form.exercise_ids().is_empty()
    {
        return Err(AppError::Validation("至少选择一个动作".to_string()));
    }

    // ④ 事务：写两张表（plans 父 + plan_items 子）要么全成要么全败
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 4.1 插入计划（父表），拿回 plan_id
    //     【M5 修订：备注随表单保存（之前硬编码 ''）】
    let plan_id = sqlx::query_scalar::<_, i64>(
        "INSERT INTO plans (phase_id, date, note) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(&phase_id)
    .bind(&form.date)
    .bind(&form.note)
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
            let (sets, reps, mode, bar_weight, key_points) =
                resolve_plan_values(&state.pool, ti.plan_sets, ti.plan_reps, ti.exercise_id)
                    .await?;
            sqlx::query(
                "INSERT INTO plan_items
                (plan_id, exercise_id, sort_order, plan_sets, plan_reps,
                plan_mode, plan_bar_weight, plan_key_points)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&plan_id)
            .bind(&ti.exercise_id)
            .bind(idx as i64)
            .bind(sets)
            .bind(reps)
            .bind(mode)
            .bind(bar_weight)
            .bind(key_points)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
        }
    }
    else
    {
        // ⑥ 手动选动作：每个动作从动作库拿默认组/次/计重/杆重/要领
        let ex_ids: Vec<i64> = form.exercise_ids();
        for (idx, ex_id) in ex_ids.iter().enumerate()
        {
            let (sets, reps, mode, bar_weight, key_points) =
                resolve_plan_values(&state.pool, None, None, *ex_id).await?;
            sqlx::query(
                "INSERT INTO plan_items
                (plan_id, exercise_id, sort_order, plan_sets, plan_reps,
                plan_mode, plan_bar_weight, plan_key_points)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&plan_id)
            .bind(ex_id)
            .bind(idx as i64)
            .bind(sets)
            .bind(reps)
            .bind(mode)
            .bind(bar_weight)
            .bind(key_points)
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
// ============================================================
// 计划详情 + 编辑（GET /plans/{id}）【M4 修订：原 plan_edit_form 并入】
// ============================================================
/// 显示计划详情，并直接以表格形式编辑（任务 0）
///
/// 【M4 修订说明（任务 0）】
/// 旧版：plan_detail 只读表格 + 独立的 plan_edit_form 编辑页（checkbox 列表）。
/// 新版：两页合并 —— GET /plans/{id} 直接是"表格形式的编辑页"：
///   - 上面：日期 + 备注 input（随表单提交）
///   - 中间：表格（动作 | 组数 | 次数 | 实际强度 | 计重方式 | 杆重/支撑 |
///           观测强度换算 | 休息 | 要领 | 备注 | 操作）
///   - 每行一个 hidden checkbox（name=动作id，value=1，checked）
///     → 提交时 exercise_ids() 收集到"仍存在于 DOM 的行"；
///       删除行 = JS remove DOM → checkbox 随之消失 → 不提交
///   - 每行 ↑↓ 按钮：HTML5 form 属性关联外部隐藏 form（form 不能嵌套）
///   - 下方"添加动作"区：身体部位下拉 + 动作下拉 + 添加按钮（JS addRow）
///   - 动作数据以 JSON 嵌入（EX_OPTIONS），JS 动态加行
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

    // ② 查计划项（按 sort_order 排序，带 item id 供上移/下移路由用）
    let plan_items = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order, id",
    )
    .bind(&plan_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;

    // ③ 查全部动作 → HashMap 索引（一次查询换 N 次查询）
    //    需要 name（显示）+ body_part（部位）+ 默认值（新动作回显）
    let all_exercises = sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE user_id = ?")
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?;
    let ex_map: HashMap<i64, Exercise> = all_exercises.iter().map(|e| (e.id, e.clone())).collect();

    // ③a 【M5 修订（随 M4 迁移到合并页）：最近记录参考（不入库）】
    //    按 exercise_id 取最近一条记录的 weight + strategy，渲染在每行作灰字参考
    //    （渐进超负荷：本次计划重量应比上次实际强度略高；
    //     最近策略提示上次怎么安排的 —— 用户诉求 2）
    //    ⚠️ 数据隔离：JOIN exercises 过滤 user_id
    //    【M7 修订：查询扩展取 r.mode —— "上次观测"逆换算需要上次计重模式】
    let last_record_map: HashMap<i64, (f64, String, String)> =
        sqlx::query_as::<_, (i64, f64, String, String)>(
            "SELECT r.exercise_id AS \"_1\", r.weight AS \"_2\", r.strategy AS \"_3\", r.mode AS \"_4\" FROM records r
             JOIN exercises e ON r.exercise_id = e.id AND e.user_id = ?
             JOIN (
                 SELECT exercise_id, MAX(record_date || '#' || printf('%010d', id)) AS k
                 FROM records GROUP BY exercise_id
             ) latest ON r.exercise_id = latest.exercise_id
             WHERE (r.record_date || '#' || printf('%010d', r.id)) = latest.k",
        )
        .bind(&user.id)
        .fetch_all(&state.pool)
        .await
        .map_err(AppError::Database)?
        .into_iter()
        .map(|(ex_id, w, s, m)| (ex_id, (w, s, m)))
        .collect();

    // ③b 部位下拉框选项（从动作列表去重，动态生成）
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

    // ③c 动作下拉框选项：value = 动作 id，文字 = 名称（JS 按部位过滤）
    let ex_options = all_exercises
        .iter()
        .map(|ex| {
            format!(
                r#"<option value="{id}" data-part="{part}">{name}</option>"#,
                id = ex.id,
                part = ex.body_part,
                name = ex.name,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ③d 动作数据 JSON（JS addRow 用：名称/部位/默认组次/默认计重/杆重/要领）
    //     key = 动作 id 字符串，value = 该动作的默认值
    let ex_data_map: serde_json::Map<String, serde_json::Value> = all_exercises
        .iter()
        .map(|ex| {
            // （M6 清理：lb2kg 历史值已迁移归正，无需归一化）
            let mode = ex.default_mode.clone();
            (
                ex.id.to_string(),
                serde_json::json!({
                    "id": ex.id,
                    "name": ex.name,
                    "part": ex.body_part,
                    "default_sets": ex.default_sets,
                    "default_reps": ex.default_reps,
                    "default_mode": mode,
                    "bar_weight": ex.bar_weight,
                    "default_unit": ex.default_unit,
                    "key_points": ex.key_points,
                }),
            )
        })
        .collect();
    let ex_data_json = serde_json::Value::Object(ex_data_map).to_string();

    // ③d 部位顺序 JSON（JS 动态组头按配置顺序插入，与后端 group_by_body_part 一致）
    let body_part_order_json = serde_json::to_string(&state.config.body_part_order)
        .map_err(|_| AppError::Validation("部位顺序序列化失败".to_string()))?;

    // ③d-2 【M5 修订：全局体重（users.body_weight，首页维护）】
    //     用户问题 0：support 模式的体重来自"可编辑的通用变量"。
    //     这里在函数体级取一次，行渲染（body input 预填）和
    //     JS 注入（addRow 新行 + 逆换算兜底）共用。
    //     未设置 → 空字符串（前端 placeholder 提示"未设置"）。
    let body_weight_text = user.body_weight.map(|v| v.to_string()).unwrap_or_default();

    // ③e 表格行（计划项，按 sort_order 排，再按 body_part 分组）
    //     【M4 修订：列重排 + 分组】
    //     列顺序改为：动作|备注|实际强度|计重方式|杆重/支撑|观测强度换算|
    //                组数|次数|休息|要领|操作
    //       - 备注移到动作后（用户诉求 2：备注紧跟动作名）
    //       - 强度相关列（实际强度/计重方式/杆重/支撑/观测强度换算）在备注后、组数前
    //       - 要领保留末尾（超长文本不撑开前段列）
    //       - 操作永远最后
    //     分组：先按 sort_order 拼裸行（含 data-part），
    //           再用 group_by_body_part 分组（组间按常量排序，组内保序），
    //           每组一行 <tr class="group-header"> 分节标题。
    //     回显链：计划项有值 → 用计划项；没有 → 动作库默认
    //     每行：hidden checkbox（保证提交收集）+ 全部编辑输入 + ↑↓ + 删除
    let raw_rows = plan_items
        .iter()
        .map(|item| {
            let ex = ex_map.get(&item.exercise_id);
            // 动作名 + 部位（查不到显示 "?"，理论不发生）
            let (ex_name, ex_part) = match ex
            {
                Some(e) => (e.name.clone(), e.body_part.clone()),
                None => ("?".to_string(), "未分组".to_string()),
            };
            // 组/次/重/计重/杆重/休息/要领/备注 回显
            let sets = item
                .plan_sets
                .map_or(ex.map_or(0, |e| e.default_sets).to_string(), |v| v.to_string());
            let reps = item
                .plan_reps
                .map_or(ex.map_or(0, |e| e.default_reps).to_string(), |v| v.to_string());
            let weight = item
                .plan_weight
                .map_or(String::new(), |v| v.to_string());
            // 计重方式回显：【M6 修订】直接取动作库默认（历史值已归正，无需归一化）
            let mode = ex.map_or("std".to_string(), |e| e.default_mode.clone());
            // 【M6 修订】杆重回显：直接取动作库默认 bar_weight
            let bar_weight = ex.map_or(0.0, |e| e.bar_weight);
            // 【M5 修订：单位回显 —— 动作 default_unit 决定观测强度下拉预填】
            let unit = ex.map_or("kg".to_string(), |e| e.default_unit.clone());
            let unit_options = ["kg", "lb"]
                .iter()
                .map(|u| {
                    format!(
                        r#"<option value="{u}"{sel}>{u}</option>"#,
                        sel = if *u == unit { " selected" } else { "" },
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            let rest = item
                .plan_rest
                .map_or(String::new(), |v| v.to_string());
            let key_points = item
                .plan_key_points
                .clone()
                .unwrap_or_else(|| ex.map_or(String::new(), |e| e.key_points.clone()));
            let note = item.plan_note.clone().unwrap_or_default();
            // 计重方式下拉选项（当前模式 selected，与 record_form 同款）
            let mode_options = ["bar", "support", "std"]
                .iter()
                .map(|m| {
                    format!(
                        r#"<option value="{m}"{sel}>{name}</option>"#,
                        sel = if *m == mode { " selected" } else { "" },
                        name = match *m
                        {
                            "bar" => "杠铃",
                            "support" => "支撑",
                            "std" => "标准",
                            _ => *m,
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            // 杆重下拉选项（四种规格，与 record_form 同款）
            let bar_weight_options = ["20", "11.3", "10", "0"]
                .iter()
                .map(|bw| {
                    format!(
                        r#"<option value="{bw}"{sel}>{name}</option>"#,
                        sel = if *bw == format!("{bar_weight}")
                        {
                            " selected"
                        }
                        else
                        {
                            ""
                        },
                        name = match *bw
                        {
                            "20" => "Olympic(20kg)",
                            "11.3" => "Smith(11.3kg)",
                            "10" => "短杠(10kg)",
                            "0" => "双边(0kg)",
                            _ => *bw,
                        },
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            // 上次记录参考（有记录才显示灰字）：实际强度 + 最近策略
            //     【M5 修订：用户诉求 2 —— 备注列旁显示最近一次 strategy】
            //     策略是"上次怎么安排的"（如"维持"、"加 5kg"），训练时
            //     参考它决定这次怎么调，和上次实际强度一样是渐进超负荷参照物。
            let last_actual_ref = last_record_map
                .get(&item.exercise_id)
                .map(|(w, _, _)| {
                    format!(
                        r#"<span style="color:#888">（上次实际：{w}kg）</span>"#,
                        w = w
                    )
                })
                .unwrap_or_default();
            let last_strategy_ref = last_record_map
                .get(&item.exercise_id)
                .map(|(_, s, _)| s.clone())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    format!(
                        r#"<br><span style="color:#888;font-size:0.85em">上次策略：{s}</span>"#,
                        s = s
                    )
                })
                .unwrap_or_default();
            // 【M7 修订：观测强度旁的上次参考 —— 反向计算（mode_display 同款逻辑）】
            // 实际强度列旁已有"上次实际"，观测强度列旁补"上次观测"：
            // 用上次记录的实际强度 + 上次计重模式 + 动作库杆重/体重/单位
            // 逆换算成片重/支撑量（与 weight_converter.js 的 inverseConvert 同逻辑）：
            //   bar     → 片重 = (总重 - 杆重) / 2
            //   support → 支撑量 = 体重 - 总重
            //   std     → 片重 = 总重
            // 单位 lb 时显示值 = kg ÷ 0.4536。
            // 上次记录不存杆重 → 统一用动作库当前默认（与 record_form 的 last_ref 同口径）。
            let last_observed_ref = last_record_map
                .get(&item.exercise_id)
                .map(|(w, _, m)| {
                    format!(
                        r#"<br><span style="color:#888;font-size:0.85em">上次观测：{obs}</span>"#,
                        obs = crate::handlers::record::mode_display(
                            m,
                            *w,
                            bar_weight,
                            user.body_weight,
                            &unit,
                        ),
                    )
                })
                .unwrap_or_default();
            // 【M5 修订：Bug 1 —— 杆重/支撑列合并成单 td + std 占位符】
            // 旧版拆两个 td（bar-cell + body-cell），std 模式两个都 display:none
            // → 该行可见 td 少 2 个，与 11 列表头错位（用户报告的错位 bug 根因）。
            // 新版合并为一个 td：内含三种状态，只显示一个：
            //   bar    → 杆重下拉（select）
            //   support → 体重输入（input，预填全局体重）
            //   std    → 占位符 "N/A"（该模式不需要杆重/支撑）
            // 列数恒为 11，不再塌陷。
            let bar_body_cell = format!(
                r#"<td id="bar-body-cell-{ex_id}">
                <select name="bar_weight_{ex_id}" id="bar-{ex_id}" style="display:none">{bar_weight_options}</select>
                <input id="body-{ex_id}" type="number" step="0.5" value="{body_weight}" placeholder="未设置" style="display:none">
                <span id="bar-body-na-{ex_id}" style="color:#999">N/A</span>
                </td>"#,
                ex_id = item.exercise_id,
                bar_weight_options = bar_weight_options,
                body_weight = body_weight_text,
            );
            let row = format!(
                r#"<tr id="row-{ex_id}" data-part="{ex_part}">
                <td><input type="checkbox" name="{ex_id}" value="1" checked hidden>{ex_name}</td>
                <td><input name="note_{ex_id}" value="{note}" size="12">{last_strategy_ref}</td>
                <td><input name="weight_{ex_id}" id="weight-input-{ex_id}" type="number" step="0.5" value="{weight}" readonly style="background:#eee;">{last_actual_ref}</td>
                <td><select name="mode_{ex_id}" id="mode-{ex_id}" class="mode-select" data-ex="{ex_id}">{mode_options}</select></td>
                {bar_body_cell}
                <td><input id="plate-{ex_id}" type="number" step="0.5" value="">
                    <select id="unit-{ex_id}">{unit_options}</select>
                    <span id="result-{ex_id}"></span>{last_observed_ref}</td>
                <td><input name="sets_{ex_id}" type="number" step="1" value="{sets}"></td>
                <td><input name="reps_{ex_id}" type="number" step="1" value="{reps}"></td>
                <td><input name="rest_{ex_id}" type="number" step="1" value="{rest}"></td>
                <td><input name="key_points_{ex_id}" value="{key_points}" size="12"></td>
                <td>
                <button type="button" onclick="submitMove('/plans/{plan_id}/items/{item_id}/move?dir=up')">↑</button>
                <button type="button" onclick="submitMove('/plans/{plan_id}/items/{item_id}/move?dir=down')">↓</button>
                <button type="button" onclick="removeRow({ex_id})">删除</button>
                </td>
                </tr>"#,
                ex_id = item.exercise_id,
                item_id = item.id,
                plan_id = plan_id,
                ex_part = ex_part,
                ex_name = ex_name,
                sets = sets,
                reps = reps,
                weight = weight,
                mode_options = mode_options,
                unit_options = unit_options,
                rest = rest,
                key_points = key_points,
                note = note,
                last_actual_ref = last_actual_ref,
                last_strategy_ref = last_strategy_ref,
                last_observed_ref = last_observed_ref,
                bar_body_cell = bar_body_cell,
            );
            (ex_part, row)
        })
        .collect::<Vec<_>>();
    // 分组渲染：组头行 + 组内行（colspan = 11 列）
    // 【M4 修订：组间顺序来自配置 AppConfig.body_part_order（环境变量可配）】
    let item_rows = group_by_body_part(raw_rows.into_iter(), &state.config.body_part_order)
        .iter()
        .map(|(part, rows)| {
            format!(
                r#"<tr class="group-header" data-part="{part}"><td colspan="11">{part}</td></tr>
{rows}"#,
                part = part,
                rows = rows.join("\n"),
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // ④ 拼页面：
    //    - form（action=/plans/{id}/edit）：日期 + 备注 + 表格 + 保存
    //    - 表格外"添加动作"区（部位下拉 + 动作下拉 + 添加按钮）
    //    - JS：EX_OPTIONS + addRow/removeRow + 多实例换算器（移植自 plan_edit_form）
    Ok(Html(format!(
        r#"
        <head><meta name="viewport" content="width=device-width, initial-scale=1.0"></head>
        <h2>编辑计划（{plan_date}）</h2>
        <form method="post" action="/plans/{plan_id}/edit">
            日期：<input type="date" name="date" value="{plan_date}">
            备注：<input name="note" value="{plan_note}" size="30"><br><br>
            <table border="1">
                <tr><th>动作</th><th>备注</th><th>实际强度</th><th>计重方式</th><th>杆重/支撑</th><th>观测强度换算</th><th>组数</th><th>次数</th><th>休息(秒)</th><th>要领</th><th>操作</th></tr>
                {item_rows}
            </table>
            <button type="submit">保存</button>
        </form>
        <p>添加动作：
            <select id="part-select" onchange="filterExByPart()">
                <option value="">全部</option>
                {part_options}
            </select>
            <select id="ex-select">
                {ex_options}
            </select>
            <button type="button" onclick="addRow()">添加</button>
        </p>
        <p><a href="/phases/{phase_id}/plans">返回计划列表</a></p>
        <hr>
        <form method="post" action="/plans/{plan_id}/delete"
            onsubmit="return confirm('确定删除该计划？训练记录会保留（解除关联）')">
            <button type="submit" style="color:#c00">删除本计划</button>
        </form>
        <script>
            {javascript}
        </script>
        "#,
        plan_id = current_plan.id,
        plan_date = current_plan.date,
        plan_note = current_plan.note,
        phase_id = current_plan.phase_id,
        part_options = part_options,
        ex_options = ex_options,
        item_rows = item_rows,
        javascript = r#"var EX_OPTIONS = __EX_DATA__;
                /* 【M5 修订：全局体重注入（users.body_weight，首页维护）】
                 * addRow 动态新行的 body input 与逆换算兜底都用它。 */
                var BODY_WEIGHT = __BODY_WEIGHT__;
                /* 上移/下移：页面级动态 form（不能直接嵌 <tr> 里，HTML 解析器会忽略） */
                function submitMove(url){
                var f = document.createElement('form');
                f.method = 'post';
                f.action = url;
                f.style.display = 'none';
                document.body.appendChild(f);
                f.submit();
                }
                function escapeHtml(s){ return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;'); }
                function modeOptions(sel){
                var opts = [['bar','杠铃'],['support','支撑'],['std','标准']];
                var html = '';
                for (var i=0;i<opts.length;i++){
                html += '<option value="' + opts[i][0] + '"' + (opts[i][0]===sel?' selected':'') + '>' + opts[i][1] + '</option>';
                }
                return html;
                }
                function barOptions(sel){
                var opts = [['20','Olympic(20kg)'],['11.3','Smith(11.3kg)'],['10','短杠(10kg)'],['0','双边(0kg)']];
                var html = '';
                for (var i=0;i<opts.length;i++){
                html += '<option value="' + opts[i][0] + '"' + (String(opts[i][0])===String(sel)?' selected':'') + '>' + opts[i][1] + '</option>';
                }
                return html;
                }
                /* ---- 添加动作：从下拉框取动作 → 按默认值克隆一行 ---- */
                /* 【M4 修订：列序与组头对齐】
                   动作 | 备注 | 实际强度 | 计重方式 | 杆重/支撑 | 观测强度换算 |
                   组数 | 次数 | 休息 | 要领 | 操作 （11 列，组头 colspan=11） */
                /* 部位标准顺序（与服务端 group_by_body_part 同一配置注入）：
                   动态添加"新部位"时，组头也按此顺序插入（不在表内 → 排最后） */
                var PART_ORDER = __PART_ORDER__;
                function partRank(p){
                var i = PART_ORDER.indexOf(p);
                return i === -1 ? PART_ORDER.length : i;
                }
                var PART_HEADER_MAP = {};
                function initPartHeaders(){
                PART_HEADER_MAP = {};
                document.querySelectorAll('tr.group-header').forEach(function(h){
                PART_HEADER_MAP[h.getAttribute('data-part')] = h;
                });
                }
                initPartHeaders();
                function addRow(){
                var sel = document.getElementById('ex-select');
                var id = sel.value;
                if (!id) return;
                if (document.getElementById('row-' + id)) return; // 已在表格中
                var ex = EX_OPTIONS[String(id)];
                if (!ex) return;
                var tr = document.createElement('tr');
                tr.id = 'row-' + id;
                tr.setAttribute('data-part', ex.part);
                tr.innerHTML =
                '<td><input type="checkbox" name="' + id + '" value="1" checked hidden>' + escapeHtml(ex.name) + '</td>' +
                '<td><input name="note_' + id + '" size="12"></td>' +
                '<td><input name="weight_' + id + '" id="weight-input-' + id + '" type="number" step="0.5" readonly style="background:#eee;"></td>' +
                '<td><select name="mode_' + id + '" id="mode-' + id + '" class="mode-select" data-ex="' + id + '">' + modeOptions(ex.default_mode) + '</select></td>' +
                '<td id="bar-body-cell-' + id + '">' +
                '<select name="bar_weight_' + id + '" id="bar-' + id + '" style="display:none">' + barOptions(ex.bar_weight) + '</select>' +
                '<input id="body-' + id + '" type="number" step="0.5" value="' + escapeHtml(BODY_WEIGHT) + '" placeholder="未设置" style="display:none">' +
                '<span id="bar-body-na-' + id + '" style="color:#999">N/A</span>' +
                '</td>' +
                '<td><input id="plate-' + id + '" type="number" step="0.5" value="">' +
                '<select id="unit-' + id + '"><option value="kg"' + (ex.default_unit === 'kg' ? ' selected' : '') + '>kg</option><option value="lb"' + (ex.default_unit === 'lb' ? ' selected' : '') + '>lb</option></select>' +
                '<span id="result-' + id + '"></span></td>' +
                '<td><input name="sets_' + id + '" type="number" step="1" value="' + ex.default_sets + '"></td>' +
                '<td><input name="reps_' + id + '" type="number" step="1" value="' + ex.default_reps + '"></td>' +
                '<td><input name="rest_' + id + '" type="number" step="1" value=""></td>' +
                '<td><input name="key_points_' + id + '" value="' + escapeHtml(ex.key_points) + '" size="12"></td>' +
                '<td><button type="button" onclick="removeRow(' + id + ')">删除</button></td>';
                /* 插到该部位的组头行之后（组头不存在 → 按常量顺序新建组头） */
                var header = PART_HEADER_MAP[ex.part];
                if (header) {
                header.parentNode.insertBefore(tr, header.nextSibling);
                } else {
                var table = document.querySelector('table');
                var tbody = table.tBodies[0] || table; // 浏览器隐式 tbody
                var newHeader = document.createElement('tr');
                newHeader.className = 'group-header';
                newHeader.setAttribute('data-part', ex.part);
                newHeader.innerHTML = '<td colspan="11">' + escapeHtml(ex.part) + '</td>';
                /* 找第一个标准顺序比它靠后的已有组头 → 插到其前面；否则插末尾 */
                var after = null;
                document.querySelectorAll('tr.group-header').forEach(function(h){
                if (after === null && partRank(h.getAttribute('data-part')) > partRank(ex.part)) {
                after = h;
                }
                });
                if (after) {
                tbody.insertBefore(newHeader, after);
                } else {
                tbody.appendChild(newHeader);
                }
                tbody.insertBefore(tr, newHeader.nextSibling);
                PART_HEADER_MAP[ex.part] = newHeader;
                }
                syncModeRow(id);
                setupRow(id);
                filterExByPart();
                }
                function removeRow(id){
                var row = document.getElementById('row-' + id);
                if (row) row.remove();
                }
                /* ---- 部位筛选（添加动作下拉框） ---- */
                function filterExByPart(){
                var part = document.getElementById('part-select').value;
                document.querySelectorAll('#ex-select option').forEach(function(opt){
                opt.style.display = (part === '' || opt.getAttribute('data-part') === part) ? '' : 'none';
                });
                }
                /* ---- 行级重量换算器（多实例版，与 record_form 的 weight_converter.js 同逻辑）----
                 * bar     总重 = 杆重 + 2×片重
                 * support 总重 = 体重 − 支撑量（下限 0）
                 * std     总重 = 片重
                 * 观测强度(plate) 不入库，只做换算；实际强度(name=weight_{exId}，
                 * readonly) 由 JS 实时自动写入，随表单提交。
                 * 体重 input 不入库，仅 support 换算用，localStorage 记忆。
                 * 【M4 修订：单位选择在观测强度旁（unit-{exId}，kg/lb，不入库）】
                 *   lb → 先 ×0.4536 归一化成 kg 再套公式；杆重/支撑固定 kg。 */
                function convertWeight(mode, plate, bar, body, unit){
                var raw = Number(plate) || 0;
                var barKg = Number(bar) || 0;
                var bodyKg = Number(body) || 0;
                var plateKg = (unit === 'lb') ? raw * 0.4536 : raw;
                switch (mode) {
                case 'bar': return barKg + 2 * plateKg;
                case 'support': return Math.max(0, bodyKg - plateKg);
                case 'std': return plateKg;
                default: return 0;
                }
                }
                function roundToHalf(x){ return Math.round(x * 2) / 2; }
                /* 【M5 修订：逆换算 —— 实际强度 → 观测强度（与 weight_converter.js 同逻辑）】
                 * bar:     片重 = (总重 - 杆重) / 2
                 * support: 支撑量 = 体重 - 总重
                 * std:     片重 = 总重
                 * lb 单位 → 显示值 = kg ÷ 0.4536。负数 clamp 到 0。 */
                function inverseConvert(mode, weight, bar, body, unit){
                var weightKg = Number(weight) || 0;
                var barKg = Number(bar) || 0;
                var bodyKg = Number(body) || 0;
                var plateKg = 0;
                switch (mode) {
                case 'bar': plateKg = (weightKg - barKg) / 2; break;
                case 'support': plateKg = bodyKg - weightKg; break;
                case 'std': plateKg = weightKg; break;
                default: plateKg = 0;
                }
                plateKg = Math.max(0, plateKg);
                return (unit === 'lb') ? plateKg / 0.4536 : plateKg;
                }
                /* 【M5 修订：Bug 1 —— 单 td 三态切换】
                 * 杆重/支撑列合并后，syncModeRow 控制 td 内三个子元素的显隐：
                 *   bar     → 显示杆重 select
                 *   support → 显示体重 input
                 *   std     → 显示 "N/A" 占位符
                 * 三种状态只显示一个，td 本身永远占位 → 列数恒 11 不错位。 */
                function syncModeRow(exId){
                var mode = document.getElementById('mode-' + exId).value;
                document.getElementById('bar-' + exId).style.display = (mode === 'bar') ? '' : 'none';
                document.getElementById('body-' + exId).style.display = (mode === 'support') ? '' : 'none';
                document.getElementById('bar-body-na-' + exId).style.display = (mode === 'std') ? '' : 'none';
                }
                function rowTotal(exId){
                var mode = document.getElementById('mode-' + exId).value;
                var plate = document.getElementById('plate-' + exId).value;
                var bar = document.getElementById('bar-' + exId).value;
                var body = document.getElementById('body-' + exId).value;
                var unit = document.getElementById('unit-' + exId).value;
                var defaultBody = Number(localStorage.getItem('weight_converter_body')) || BODY_WEIGHT || 70;
                return roundToHalf(convertWeight(mode, plate, bar, body || defaultBody, unit));
                }
                function updateRow(exId){
                document.getElementById('result-' + exId).textContent = rowTotal(exId) + ' kg';
                if (document.getElementById('plate-' + exId).value !== '') {
                document.getElementById('weight-input-' + exId).value = rowTotal(exId);
                }
                }
                function setupRow(exId){
                var sel = document.getElementById('mode-' + exId);
                syncModeRow(exId);
                var savedUnit = localStorage.getItem('weight_converter_unit');
                if (savedUnit === 'kg' || savedUnit === 'lb') {
                document.getElementById('unit-' + exId).value = savedUnit;
                }
                sel.addEventListener('input', function(){ syncModeRow(exId); updateRow(exId); });
                document.getElementById('plate-' + exId).addEventListener('input', function(){ updateRow(exId); });
                document.getElementById('unit-' + exId).addEventListener('input', function(){
                localStorage.setItem('weight_converter_unit', this.value);
                updateRow(exId);
                });
                document.getElementById('bar-' + exId).addEventListener('input', function(){ updateRow(exId); });
                document.getElementById('body-' + exId).addEventListener('input', function(){
                localStorage.setItem('weight_converter_body', this.value);
                updateRow(exId);
                });
                /* 【M5 修订：逆换算预填 —— 页面加载只执行一次】
                 * 与 weight_converter.js 同款：weight-input 有回显值（计划值/
                 * 上次值）时，逆换算成观测强度（plate）预填，训练时直接看片重。
                 * 不会循环计算：依赖单向（plate → weight），weight readonly
                 * 无监听器；写 plate 触发 updateRow 重算 weight 幂等（逆的逆≈原值）。 */
                var weightEl = document.getElementById('weight-input-' + exId);
                var plateEl = document.getElementById('plate-' + exId);
                if (weightEl.value !== '' && plateEl.value === '') {
                plateEl.value = roundToHalf(inverseConvert(
                sel.value,
                weightEl.value,
                document.getElementById('bar-' + exId).value,
                document.getElementById('body-' + exId).value || BODY_WEIGHT || 70,
                document.getElementById('unit-' + exId).value || 'kg',
                ));
                }
                if (plateEl.value === '') {
                document.getElementById('result-' + exId).textContent = '';
                }
                }
                /* 初始化：已有行全部 setup + 部位下拉联动 */
                document.querySelectorAll('.mode-select').forEach(function(sel){
                setupRow(sel.getAttribute('data-ex'));
                });
                filterExByPart();"#
                .replace("__EX_DATA__", &ex_data_json)
                .replace("__PART_ORDER__", &body_part_order_json)
                .replace("__BODY_WEIGHT__", &body_weight_text),
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
    //     ⚠️【外键陷阱：同 plan_delete】已训练过的计划项有 records 引用，
    //     直接删会报 FOREIGN KEY constraint failed。
    //     先解除关联（保留训练历史），再删。
    //     【M4 修订：删前先存旧 (exercise_id → sort_order) 映射，
    //     4.3 重新插入时沿用旧 sort_order，用户在编辑页调的排序不丢；
    //     新添加的动作排末尾（base + 1 起）】
    let old_items = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order, id",
    )
    .bind(&plan_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let old_order: HashMap<i64, i64> = old_items
        .iter()
        .map(|i| (i.exercise_id, i.sort_order))
        .collect();
    let base_order = old_order.values().copied().max().unwrap_or(-1);

    // ⚠️【M5 修订：重新关联训练记录】
    // 解除关联前先备份 (exercise_id → record_id 列表)，
    // 4.3 重建 plan_items（新 id）后按此清单精确还原——
    // 否则 today 页/record_form 按 plan_item_id 查记录 → 全部"未训练"！
    // 不能事后按 (plan_id, exercise_id) 猜，会误捞其他计划/历史遗留的 NULL 记录。
    let orphaned: HashMap<i64, Vec<i64>> = sqlx::query_as::<_, (i64, i64)>(
        "SELECT r.exercise_id, r.id FROM records r
        WHERE r.plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&plan_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?
    .into_iter()
    .fold(HashMap::new(), |mut acc, (ex_id, rec_id)| {
        acc.entry(ex_id).or_default().push(rec_id);
        acc
    });

    sqlx::query(
        "UPDATE records SET plan_item_id = NULL
        WHERE plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&plan_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

    sqlx::query("DELETE FROM plan_items WHERE plan_id = ?")
        .bind(&plan_id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;

    // 4.3 重新插入勾选的动作
    //    组/次/重/计重方式/杆重/休息/要领直接来自表单
    //    （编辑页已回显当前值，未选动作预填默认值），
    //    空字符串 → None → 存 NULL（plan_detail 显示 "-"）
    //    ⚠️【M5 修订：重建后按备份清单精确还原记录关联】
    //    4.2 已把该计划下所有 records.plan_item_id 置 NULL（外键防冲突），
    //    重建的 plan_items 是新 id——这里用 orphaned 清单还原：
    //    同一计划内每个动作唯一，exercise_id → 新 plan_item_id 一一对应。
    let ex_ids: Vec<i64> = form.exercise_ids();
    let mut new_idx: i64 = 0;
    for ex_id in ex_ids
    {
        // 旧动作沿用旧 sort_order；新动作从 base_order+1 起递增
        let sort_order = match old_order.get(&ex_id)
        {
            Some(o) => *o,
            None =>
            {
                new_idx += 1;
                base_order + new_idx
            },
        };
        // 【M6 修订：不再插入 plan_mode/plan_bar_weight（已废弃，保持 NULL）】
        let result = sqlx::query(
            "INSERT INTO plan_items
            (plan_id, exercise_id, sort_order, plan_sets, plan_reps, plan_weight,
            plan_rest, plan_key_points, plan_note)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&plan_id)
        .bind(ex_id)
        .bind(sort_order)
        .bind(form.plan_sets(ex_id))
        .bind(form.plan_reps(ex_id))
        .bind(form.plan_weight(ex_id))
        .bind(form.plan_rest(ex_id))
        .bind(form.plan_key_points(ex_id))
        .bind(form.plan_note(ex_id))
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
        let new_item_id = result.last_insert_rowid();

        // 精确还原：仅把备份清单中该动作的记录挂回新 plan_item_id
        //（逐条 UPDATE，避免依赖 SQLite JSON1 扩展）
        if let Some(rec_ids) = orphaned.get(&ex_id)
        {
            for rec_id in rec_ids
            {
                sqlx::query("UPDATE records SET plan_item_id = ? WHERE id = ?")
                    .bind(new_item_id)
                    .bind(rec_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(AppError::Database)?;
            }
        }
    }

    // ⑤ 提交
    tx.commit().await.map_err(AppError::Database)?;

    // ⑥ 重定向回计划列表（本阶段的 plans 列表页）
    //     【M5 修订：原重定向回 /plans/{plan_id}（编辑页自身），
    //     用户要求保存后离开编辑态，回到列表。】
    //     ⚠️ 不能跳裸 /plans（无该路由，会 404）——计划列表挂在阶段下：
    //     GET /phases/{phase_id}/plans（main.rs 已注册）
    Ok(Redirect::to(&format!(
        "/phases/{phase_id}/plans",
        phase_id = current_plan.phase_id
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
/// 3. 事务：UPDATE records 解除关联 → DELETE FROM plan_items → DELETE FROM plans
/// 4. commit → 重定向回计划列表
///
/// ⚠️【踩坑记录：外键约束失败（787）】
/// records.plan_item_id 外键引用 plan_items(id)。
/// 直接删 plan_items 时，若该计划项已被训练过（有 records 引用），
/// SQLite 会报 FOREIGN KEY constraint failed，删除失败。
/// 修复：先 UPDATE records SET plan_item_id = NULL（保留训练历史，解除关联），
/// 再删 plan_items。
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

    // ② 事务：解除 records 外键关联 → 删子（plan_items）→ 删父（plans）
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;

    // 2.1 该计划下的所有计划项 id → 对应 records 的 plan_item_id 置 NULL
    //     训练记录是用户的历史数据，删除计划不该连记录一起删，
    //     只解除关联（records 变成"非计划录入"，plan_item_id 列本来就是可空的）
    sqlx::query(
        "UPDATE records SET plan_item_id = NULL
        WHERE plan_item_id IN (SELECT id FROM plan_items WHERE plan_id = ?)",
    )
    .bind(&plan_id)
    .execute(&mut *tx)
    .await
    .map_err(AppError::Database)?;

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
// 【M4 修订：排序 handler（任务 4）】模板排序 + 模板项/计划项上移下移
// ============================================================
// 三个 handler 共用一套思路（"重排"式 swap）：
//   1. 查同组兄弟行（按 sort_order, id 排好）→ 找到当前行下标 i
//   2. 目标下标 j = i ± 1（dir=up/down），越界（首/尾）直接跳过
//   3. 事务内先重写全部 sort_order = 1..n（保证连续），再交换 i、j 两行
//   4. Redirect 回原页面
//
// 为什么"先重写再交换"？
//   模板 sort_order 是 M4 前预留的字段（恒为 0），直接 swap 两行都变 0 无效；
//   重写 1..n 后再交换 → 每次操作都产生唯一正确的相邻顺序。
//   （模板项/计划项的 sort_order 本来就是 1..n，重写后行为一致。）

/// 查询参数：?dir=up | ?dir=down
#[derive(Deserialize)]
pub struct MoveQuery
{
    pub dir: String,
}

/// 模板上移/下移（POST /templates/{id}/sort?dir=up|down）
/// 在【同一阶段】内交换相邻模板的 sort_order
pub async fn template_sort(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(template_id): Path<i64>,
    Query(query): Query<MoveQuery>,
) -> Result<Redirect, AppError>
{
    // ① 验证归属：JOIN phases 拿 user_id（模板不存在/不属于你 → NotFound）
    let current = sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No template found".to_string()))?;

    // ② 查同阶段全部模板（按 sort_order, id 排）→ 找当前行下标
    let siblings = sqlx::query_as::<_, Template>(
        "SELECT * FROM templates WHERE phase_id = ? ORDER BY sort_order, id",
    )
    .bind(&current.phase_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let index = siblings.iter().position(|t| t.id == template_id);
    let Some(index) = index
    else
    {
        // 理论上不发生（归属已验证），防御性处理
        return Ok(Redirect::to(&format!(
            "/phases/{phase_id}/templates",
            phase_id = current.phase_id
        )));
    };
    // 目标下标：up → index-1，down → index+1；越界 → 不动（首行↑/尾行↓）
    let target = match query.dir.as_str()
    {
        "up" => index.checked_sub(1),
        "down" =>
        {
            let t = index + 1;
            (t < siblings.len()).then_some(t)
        },
        _ => return Err(AppError::Validation("dir must be up or down".to_string())),
    };
    let Some(target) = target
    else
    {
        // 首行/尾行：无需变化，直接回列表
        return Ok(Redirect::to(&format!(
            "/phases/{phase_id}/templates",
            phase_id = current.phase_id
        )));
    };

    // ③ 事务：先重写 1..n 再交换（见文件头注释）
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;
    for (i, t) in siblings.iter().enumerate()
    {
        sqlx::query("UPDATE templates SET sort_order = ? WHERE id = ?")
            .bind((i + 1) as i64)
            .bind(t.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
    }
    let (a, b) = (&siblings[index], &siblings[target]);
    sqlx::query("UPDATE templates SET sort_order = ? WHERE id = ?")
        .bind((target + 1) as i64)
        .bind(a.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    sqlx::query("UPDATE templates SET sort_order = ? WHERE id = ?")
        .bind((index + 1) as i64)
        .bind(b.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/phases/{phase_id}/templates",
        phase_id = current.phase_id
    )))
}

/// 模板项上移/下移（POST /templates/{id}/items/{item_id}/move?dir=up|down）
/// 同一模板内交换相邻 template_items 的 sort_order
pub async fn template_item_move(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((template_id, item_id)): Path<(i64, i64)>,
    Query(query): Query<MoveQuery>,
) -> Result<Redirect, AppError>
{
    // ① 验证模板归属（JOIN phases）
    let _current = sqlx::query_as::<_, Template>(
        "SELECT t.* FROM templates t INNER JOIN phases p ON t.phase_id = p.id
    WHERE t.id = ? AND p.user_id = ?",
    )
    .bind(&template_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No template found".to_string()))?;

    // ② 查同模板全部项（按 sort_order, id 排）→ 找当前行下标
    let siblings = sqlx::query_as::<_, TemplateItem>(
        "SELECT * FROM template_items WHERE template_id = ? ORDER BY sort_order, id",
    )
    .bind(&template_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let index = siblings.iter().position(|i| i.id == item_id);
    let Some(index) = index
    else
    {
        return Ok(Redirect::to(&format!(
            "/templates/{template_id}/edit",
            template_id = template_id
        )));
    };
    let target = match query.dir.as_str()
    {
        "up" => index.checked_sub(1),
        "down" =>
        {
            let t = index + 1;
            (t < siblings.len()).then_some(t)
        },
        _ => return Err(AppError::Validation("dir must be up or down".to_string())),
    };
    let Some(target) = target
    else
    {
        return Ok(Redirect::to(&format!(
            "/templates/{template_id}/edit",
            template_id = template_id
        )));
    };

    // ③ 事务：先重写 1..n 再交换
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;
    for (i, item) in siblings.iter().enumerate()
    {
        sqlx::query("UPDATE template_items SET sort_order = ? WHERE id = ?")
            .bind((i + 1) as i64)
            .bind(item.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
    }
    let (a, b) = (&siblings[index], &siblings[target]);
    sqlx::query("UPDATE template_items SET sort_order = ? WHERE id = ?")
        .bind((target + 1) as i64)
        .bind(a.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    sqlx::query("UPDATE template_items SET sort_order = ? WHERE id = ?")
        .bind((index + 1) as i64)
        .bind(b.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/templates/{template_id}/edit",
        template_id = template_id
    )))
}

/// 计划项上移/下移（POST /plans/{id}/items/{item_id}/move?dir=up|down）
/// 同一计划内交换相邻 plan_items 的 sort_order
pub async fn plan_item_move(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path((plan_id, item_id)): Path<(i64, i64)>,
    Query(query): Query<MoveQuery>,
) -> Result<Redirect, AppError>
{
    // ① 验证计划归属（JOIN phases）
    let _current = sqlx::query_as::<_, Plan>(
        "SELECT p.* FROM plans p INNER JOIN phases ph ON p.phase_id = ph.id
    WHERE p.id = ? AND ph.user_id = ?",
    )
    .bind(&plan_id)
    .bind(&user.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(AppError::Database)?
    .ok_or_else(|| AppError::NotFound("No plan found".to_string()))?;

    // ② 查同计划全部项（按 sort_order, id 排）→ 找当前行下标
    let siblings = sqlx::query_as::<_, PlanItem>(
        "SELECT * FROM plan_items WHERE plan_id = ? ORDER BY sort_order, id",
    )
    .bind(&plan_id)
    .fetch_all(&state.pool)
    .await
    .map_err(AppError::Database)?;
    let index = siblings.iter().position(|i| i.id == item_id);
    let Some(index) = index
    else
    {
        return Ok(Redirect::to(&format!(
            "/plans/{plan_id}",
            plan_id = plan_id
        )));
    };
    let target = match query.dir.as_str()
    {
        "up" => index.checked_sub(1),
        "down" =>
        {
            let t = index + 1;
            (t < siblings.len()).then_some(t)
        },
        _ => return Err(AppError::Validation("dir must be up or down".to_string())),
    };
    let Some(target) = target
    else
    {
        return Ok(Redirect::to(&format!(
            "/plans/{plan_id}",
            plan_id = plan_id
        )));
    };

    // ③ 事务：先重写 1..n 再交换
    let mut tx = state.pool.begin().await.map_err(AppError::Database)?;
    for (i, item) in siblings.iter().enumerate()
    {
        sqlx::query("UPDATE plan_items SET sort_order = ? WHERE id = ?")
            .bind((i + 1) as i64)
            .bind(item.id)
            .execute(&mut *tx)
            .await
            .map_err(AppError::Database)?;
    }
    let (a, b) = (&siblings[index], &siblings[target]);
    sqlx::query("UPDATE plan_items SET sort_order = ? WHERE id = ?")
        .bind((target + 1) as i64)
        .bind(a.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    sqlx::query("UPDATE plan_items SET sort_order = ? WHERE id = ?")
        .bind((index + 1) as i64)
        .bind(b.id)
        .execute(&mut *tx)
        .await
        .map_err(AppError::Database)?;
    tx.commit().await.map_err(AppError::Database)?;

    Ok(Redirect::to(&format!(
        "/plans/{plan_id}",
        plan_id = plan_id
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
    /// 备注（plans.note，训练提醒，如"xxkg晋级赛"）
    pub note: String,
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

    // ⚠️【M6 清理：plan_mode()/plan_bar_weight() 已删除】
    // 计重方式/杆重不再由计划项维护（单一事实来源 = exercises），
    // 这两个解析方法已无调用方。表单里的 mode_{id}/bar_weight_{id}
    // 键仍会提交，但后端不再读取（plan_detail 的计重回显直取动作库）。

    /// 休息秒（键 rest_{id}；空字符串 → None）
    pub fn plan_rest(&self, ex_id: i64) -> Option<i64>
    {
        self.plan_value("rest", ex_id)
    }

    /// 要领（键 key_points_{id}；空字符串 → None）
    pub fn plan_key_points(&self, ex_id: i64) -> Option<String>
    {
        self.rest.get(&format!("key_points_{ex_id}")).and_then(|v| {
            let v = v.trim();
            if v.is_empty()
            {
                None
            }
            else
            {
                Some(v.to_string())
            }
        })
    }

    /// 动作备注（键 note_{id}；空字符串 → None）
    pub fn plan_note(&self, ex_id: i64) -> Option<String>
    {
        self.rest.get(&format!("note_{ex_id}")).and_then(|v| {
            let v = v.trim();
            if v.is_empty()
            {
                None
            }
            else
            {
                Some(v.to_string())
            }
        })
    }
}
