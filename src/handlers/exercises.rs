// ============================================================
// handlers/exercises.rs —— 动作库（Exercise）的 HTTP 处理器
// ============================================================
// 【教学说明】
// 这个文件处理"与动作库相关的 HTTP 请求"：
//   GET  /exercises           → 动作列表（list，可按部位筛选）
//   GET  /exercises/new       → 创建动作表单页（create_form）
//   POST /exercises           → 创建动作（create）
//   GET  /exercises/{id}/edit → 编辑表单页（edit_form）
//   POST /exercises/{id}/edit → 更新动作（update）
//   POST /exercises/{id}/delete → 删除动作（delete）
//   GET  /exercises/{id}      → 动作详情（detail，M5 用，先占位）
//
// 7 个函数，比 phases 多了 delete（动作库允许删除）。
//
// 📌 阶段要求：M2 你来实现本文件所有函数（detail 除外）。
//   完整实现已备份在 docs/learning_path/M2_ref/exercises_ref.rs，
//   实现完成后对照检查（不要提前看）。
// ============================================================

// 【教学：本文件新增的导入 —— Query 提取器】
// phases.rs 用过 Path（URL 路径段 /phases/{id}/edit 里的 id）。
// 本文件 list 要多一个能力：按部位筛选（GET /exercises?body_part=胸）。
// "?" 后面的是**查询参数**（query string），用 axum 的 Query 提取器拿：
//   /exercises?body_part=胸
//     → Query(query): Query<ListQuery>  →  query.body_part = Some("胸")
//   /exercises（不带参数）
//     → query.body_part = None（Option 字段：参数缺省 = None）
//
// 三种"从请求拿数据"的提取器对比（都要实现 FromRequestParts/FromRequest）：
//   Path    → URL 路径段：/phases/{id}     → id
//   Query   → URL 问号后：?body_part=胸     → body_part
//   Form    → 请求体：表单字段              → PhaseForm/ExerciseForm
// 三者的共同点：都是"axum 帮我们从请求里拆出数据装进结构体"。
// 区别只是**数据在请求的哪个位置**。
use axum::{
    extract::{Form, Path, Query, State},
    response::Redirect,
};
use serde::Deserialize;

use crate::{AppState, error::AppError, handlers::auth::AuthUser, models::Exercise};

// ============================================================
// 【教学：列表筛选 —— Query 提取器 + Option 字段】
// 学生问："动作列表要按部位筛选，筛选参数放哪？"
//
// 筛选条件放在 URL 查询参数里（GET /exercises?body_part=胸），
// 因为：
//   ① GET 请求没有请求体（只有 POST/PUT 才有 body），参数只能放 URL；
//   ② 查询参数是"可选的"——不筛选就访问 /exercises，照样出全部；
//   ③ URL 可收藏/分享：把筛选后的地址发给别人，对方看到一样的列表。
//
// ListQuery 结构体：
//   body_part: Option<String>
//   —— 用 Option 表达"用户没传这个参数"（None = 不筛选）。
//      注意和表单层的区别：Form 里的字段是必填的（String），
//      Query 里的字段天然可选（不传就是 None），所以用 Option。
// ============================================================
#[derive(Deserialize)]
pub struct ListQuery
{
    body_part: Option<String>,
}

// ============================================================
// 【教学：动作表单 —— 字段更多，类型转换登场】
// PhaseForm 三个字段全是 String（name/note/start_date）。
// ExerciseForm 有 7 个字段，其中 3 个是"数字"：
//   bar_weight   REAL（默认 20.0）      —— 表单层用 String！
//   default_sets INTEGER（默认 3）
//   default_reps INTEGER（默认 8）
//
// 为什么数字字段也用 String 接收？和 start_date 同理：
//   HTML 表单提交的一切都是字符串（"20"、"3"、"8"）。
//   如果字段声明成 f64/i64，用户留空提交 ""（空串），
//   axum 反序列化 "" → f64 失败 → 直接 400 错误。
//   用 String 接收，空串和非空都能进函数，由我们决定怎么处理。
//
// 【教学：String → 数字 的类型转换（parse）】
// 入库前要把 String 转成 f64/i64（表列是 REAL/INTEGER）：
//   "20".parse::<f64>()  → Ok(20.0)
//   "abc".parse::<f64>() → Err(ParseFloatError)  ← 用户填了非数字！
// parse 返回 Result：成功 Ok(值)，失败 Err(解析错误)。
// 失败时我们转成 Validation(422)——"你填的不是数字"是用户输入问题，
// 不是服务器问题，不该 500。
//
// 加上"留空 → 默认值"的规则，bar_weight 的完整转换：
//   空串 → 20.0（表默认）
//   非空且是数字 → 解析值
//   非空但不是数字 → Err(Validation("杆重必须是数字"))
//
// 【教学：下拉选择（<select>）】
// body_part（胸/背/腿/肩/臂/核心）和 default_mode（bar/support/std/lb2kg）
// 是"有限取值"，用下拉框让用户选，而不是自由输入：
//   <select name="body_part">
//     <option value="胸">胸</option>
//     <option value="背">背</option>
//     ...
//   </select>
//   提交时浏览器把选中的 option 的 value 放进表单 → 和 <input> 一样，
//   靠 name 属性对接 Rust 结构体字段。
//   好处：用户不会填错（不用猜"胸"还是"胸部"），后端也少一层校验。
// ============================================================
#[derive(Deserialize)]
pub struct ExerciseForm
{
    name: String,
    body_part: String,
    default_mode: String,
    bar_weight: String,   // 表单层 String，入库前 parse 成 f64
    default_sets: String, // 表单层 String，入库前 parse 成 i64
    default_reps: String, // 表单层 String，入库前 parse 成 i64
    key_points: String,
}

// ============================================================
// 动作列表（GET /exercises，可按部位筛选）
// ============================================================
/// 显示当前用户的动作库（可带 ?body_part=胸 筛选）
///
/// 【教学：动态 SQL —— 筛选条件有和没有，SQL 不一样】
///   body_part = None（不筛选）：
///     SELECT * FROM exercises WHERE user_id = ? ORDER BY body_part, name
///   body_part = Some(胸)（筛选）：
///     SELECT * FROM exercises WHERE user_id = ? AND body_part = ? ORDER BY ...
/// 两种 SQL 差一个 AND 条件。处理方式：
///   做法 A：写两次查询（if/else 各写各的）→ 直观，重复一点
///   做法 B：动态拼 SQL 字符串 → 省重复，但字符串拼接要小心
/// 本项目用做法 A（和 phases 的 list 两次查询同理，简单直观）。
/// （ORDER BY body_part, name：先按部位排，同部位按名字排。
///   用户看到的动作库是"按部位分组"的效果。）
///
/// 【教学：match 表达式整理 Option】
/// 判断 query.body_part 有值无值，用 match 最清晰：
///   match &query.body_part
///   {
///       Some(part) => { /* 带筛选的查询 */ }
///       None => { /* 不带筛选的查询 */ }
///   }
/// 两个分支各做一次查询、各返回 Vec<Exercise>，
/// 最后 match 整体作为表达式的值（两分支类型相同）。
///
/// 【教学：表格加"编辑/删除"链接】
/// 动作列表的每一行除了数据，还要有操作入口：
///   <a href="/exercises/{id}/edit">编辑</a>
///   <form method="post" action="/exercises/{id}/delete">...</form>
/// 注意：删除用 POST 表单（不能是 <a> 链接）！
///   <a> 是 GET 请求，而 DELETE 是有副作用的操作——
///   GET 请求可被浏览器预取/爬虫访问，用 GET 触发删除很危险。
///   所以删除按钮必须是 <form method="post">（PRG 模式，同 create）。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Query(query)
/// 2. match query.body_part：Some(part) 带筛选查，None 查全部
/// 3. Vec<Exercise> → 表格行 HTML（map → collect → join）
/// 4. 返回页面（含"创建动作"链接）
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Query(query): Query<ListQuery>,
) -> Result<String, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    Ok(format!(
        r#"
        <h1>动作库</h1>
        <table border="1">
            <tr><th>名称</th><th>部位</th><th>模式</th><th>组数</th><th>次数</th><th>操作</th></tr>
            {query_ret_rows}
        </table>
        <p><a href="/exercises/new">创建动作</a></p>
        <p><a href="/">返回首页</a></p>
        "#, 
        query_ret_rows = match &query.body_part
    {
        None => sqlx::query_as::<_, Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? ORDER BY body_part, name",
        )
        .bind(&user.id)
        .fetch_all(&state.pool),
        Some(pt) => sqlx::query_as::<_, Exercise>(
            "SELECT * FROM exercises WHERE user_id = ? AND body_part = ? ORDER BY body_part, name",
        )
        .bind(&user.id)
        .bind(pt)
        .fetch_all(&state.pool),
    }
    .await
    .map_err(AppError::Database)?
    .iter()
    .map(|e| {
        format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td>\
             <td><a href=\"/exercises/{}/edit\">编辑</a>\
             <form method=\"post\" action=\"/exercises/{}/delete\" style=\"display:inline\">\
             <button type=\"submit\">删除</button></form></td></tr>",
            e.name,
            e.body_part,
            e.default_mode,
            e.default_sets,
            e.default_reps,
            e.id,
            e.id
        )
    })
    .collect::<Vec<String>>()
    .join("\n")
    ))
}

// ============================================================
// 创建动作表单页（GET /exercises/new）
// ============================================================
/// 显示"创建动作"表单页
///
/// 【教学：这个表单和 create_form 的异同】
/// 和 phases 的 create_form 结构一样（GET 显示 / POST 处理分离），
/// 但字段多了：name/body_part/default_mode 用输入框或下拉框，
/// bar_weight/default_sets/default_reps 是数字输入框
/// （<input type="number">，浏览器自带数字校验），key_points 是文本域。
///
/// 【教学：<select> 下拉框的完整写法】
///   <label>部位
///     <select name="body_part">
///       <option value="胸">胸</option>
///       <option value="背">背</option>
///       <option value="腿">腿</option>
///       <option value="肩">肩</option>
///       <option value="臂">臂</option>
///       <option value="核心">核心</option>
///     </select>
///   </label>
///   提交时 value 进表单。用户看到中文、提交的也是中文，
///   和数据库存的中文一致（body_part TEXT）。
///
/// 【教学：数字输入框 + 默认值提示】
///   <input type="number" name="bar_weight" step="0.5" placeholder="默认 20">
///   type="number"   → 浏览器只让输入数字
///   step="0.5"      → 允许小数（片重常是 0.5 的倍数）
///   placeholder     → 灰色提示文字（不是真实值，只是提示）
///   不填提交 → 空串 → 后端转默认值（bar_weight → 20.0）
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser
/// 2. 返回 <form method="post" action="/exercises"> 的 HTML
///    （下拉框：body_part 6 项、default_mode 4 项；
///     数字框：bar_weight/default_sets/default_reps；
///     文本域：key_points）
/// 3. 返回链接 /exercises
pub async fn create_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<String, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    unimplemented!("M2 学生实现：创建动作表单页")
}

// ============================================================
// 创建动作（POST /exercises）
// ============================================================
/// 处理创建动作表单提交
///
/// 【教学：parse 错误处理 —— 又一个 ? 的用法】
/// 转换数字字段时，parse 返回 Result：
///   "abc".parse::<f64>()  → Err
/// 我们想把"解析失败"转成"用户输入不合法"（422）：
///   form.bar_weight
///       .parse::<f64>()
///       .map_err(|_| AppError::Validation("杆重必须是数字".to_string()))?
/// map_err 把 ParseFloatError（底层解析错误）换成
/// AppError::Validation（业务语义错误），? 解包。
/// （|_| 是闭包——忽略错误细节，只转换类型。M3 详细讲闭包。）
///
/// 【教学：空串 → 默认值 的判断（数字版）】
/// bar_weight 的完整逻辑：
///   空串 → 20.0（用表默认值）
///   非空 → parse（可能失败 → 422）
/// 写法：
///   let bar_weight = if form.bar_weight.trim().is_empty()
///   {
///       20.0
///   }
///   else
///   {
///       form.bar_weight.trim().parse::<f64>()
///           .map_err(|_| AppError::Validation("杆重必须是数字".to_string()))?
///   };
/// default_sets/default_reps 同理（默认 3 / 8，类型 i64）。
///
/// 【教学：create 的其他部分 —— 与 phases 相同】
///   校验 name 非空 → 查重（UNIQUE(user_id, name)）
///   → INSERT（9 列，user_id/name/body_part/default_mode/
///      bar_weight/default_sets/default_reps/key_points）
///   → Ok(Redirect::to("/exercises"))
/// 这些都在 phases 练过了，这里照搬模式即可。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Form<ExerciseForm>
/// 2. 校验 name 非空
/// 3. 查重：SELECT id FROM exercises WHERE user_id = ? AND name = ?
///    → 查到就 Err(Validation("动作名已存在"))
/// 4. 转换数字字段：bar_weight → f64，default_sets/reps → i64
///    （空串 → 默认值；非空 → parse，失败 → 422）
/// 5. INSERT INTO exercises (user_id, name, body_part, default_mode,
///    bar_weight, default_sets, default_reps, key_points) VALUES (?,?,?,?,?,?,?,?)
/// 6. Ok(Redirect::to("/exercises"))
pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<ExerciseForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    unimplemented!("M2 学生实现：创建动作")
}

// ============================================================
// 编辑动作表单页（GET /exercises/{id}/edit）
// ============================================================
/// 显示"编辑动作"表单页（预填当前值）
///
/// 【教学：数字字段的预填 —— f64/i64 转回 String 显示】
/// edit_form 要把旧值填进 input 的 value 属性：
///   phase 的 start_date 是 String，直接插。
///   但 exercise 的 bar_weight 是 f64、default_sets 是 i64——
///   value 属性是字符串，要用 {} 格式化：
///     value="{exercise.bar_weight}"   → value="20"
///     value="{exercise.default_sets}" → value="3"
///   （{} 对 f64 用 Display，输出 "20" 而不是 "20.0"——
///     Rust 的 Display 会省略无意义的小数。够用即可。）
///
/// 【教学：下拉框预选 —— selected 属性】
/// 编辑时下拉框要显示"当前值"：
///   <option value="胸" selected>胸</option>   ← selected = 默认选中
/// 但"当前值"是动态的（可能是胸/背/腿...），怎么只给匹配的那个加 selected？
///   用 Rust 判断后拼字符串：
///   let body_part_options = ["胸", "背", "腿", "肩", "臂", "核心"]
///       .iter()
///       .map(|part| {
///           let sel = if *part == exercise.body_part { " selected" } else { "" };
///           format!("<option value=\"{part}\"{sel}>{part}</option>")
///       })
///       .collect::<Vec<_>>()
///       .join("");
///   迭代器 + 条件拼字符串（"当前值加 selected，其他不加"）。
///   format! 里 {sel} 是空串或 " selected"。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 查这一行：SELECT * FROM exercises WHERE id = ? AND user_id = ?
///    → fetch_optional → None 则 Err(NotFound)
/// 3. 拼表单：input 的 value 填旧值，select 的 option 加 selected
pub async fn edit_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<String, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    unimplemented!("M2 学生实现：编辑动作表单页")
}

// ============================================================
// 更新动作（POST /exercises/{id}/edit）
// ============================================================
/// 处理编辑动作表单提交
///
/// 【教学：update 与 phases 的 update 异同】
/// 相同：校验 → 转换数字 → UPDATE（WHERE id AND user_id）→ 重定向。
/// 不同：
///   ① phases 有"归档禁编辑"守卫；exercises 没有归档概念，直接更新。
///   ② phases 没做查重（保留现状，靠 UNIQUE 兜底）；exercises 同样。
///   ③ 数字字段要重新 parse（和 create 一样，空串 → 默认值）。
///
/// 【教学：为什么 exercises 的 update 可以不做查重？】
/// 和 phases 的 update 同理：不排除自己（id != ?）的查重会误伤
/// "不改名字直接提交"。而 UNIQUE(user_id, name) 约束是数据库兜底——
/// 真撞名时 UPDATE 报约束错误（500），简单且极少发生。
/// 这是"用数据库约束兜底，用业务查询优化体验"的取舍，可接受。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Path(id) + Form<ExerciseForm>
/// 2. 校验 name 非空
/// 3. 转换数字字段（同 create）
/// 4. UPDATE exercises SET name=?, body_part=?, default_mode=?,
///    bar_weight=?, default_sets=?, default_reps=?, key_points=?
///    WHERE id = ? AND user_id = ?
/// 5. rows_affected() == 0 → Err(NotFound)；否则 Ok(Redirect::to("/exercises"))
pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
    Form(form): Form<ExerciseForm>,
) -> Result<Redirect, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    unimplemented!("M2 学生实现：更新动作")
}

// ============================================================
// 删除动作（POST /exercises/{id}/delete）
// ============================================================
/// 删除动作
///
/// 【教学：为什么动作用 delete，而阶段用 archive？—— 引用关系】
/// 阶段（phase）不能删，只能归档，因为阶段是"时间容器"，
/// 计划/记录都挂在 phase_id 上，删了阶段 = 删历史。
/// 动作（exercise）是"动作字典"，看起来可以删——但注意！
/// 数据库里 template_items / plan_items / records 都引用 exercise_id。
///
/// 真正的设计取舍：
///   - 方案 A（本步实现）：直接 DELETE。当前阶段动作库刚建，
///     还没有模板/记录引用它，删掉无风险。
///   - 方案 B（M3/M4 完善）：删除前检查引用，有则拒绝
///     （COUNT template_items/plan_items/records WHERE exercise_id = ?），
///     与 phase 归档同理，保护历史。
/// 本步用 A，但注释标明：M3/M4 建引用后应升级为 B。
/// 这种"先做能跑的，标注演进点"是开发节奏的一部分——
/// 不要现在为不存在的场景过度设计（YAGNI 原则）。
///
/// 【教学：DELETE 的 rows_affected 判断】
/// 和 UPDATE 一样：execute() 返回 SqliteQueryResult，
/// rows_affected() == 0 → id 不存在或不是自己的 → 404。
/// 删除成功 → 重定向回列表（PRG 模式）。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Path(id)
/// 2. DELETE FROM exercises WHERE id = ? AND user_id = ?
/// 3. rows_affected() == 0 → Err(NotFound)；否则 Ok(Redirect::to("/exercises"))
pub async fn delete(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<Redirect, AppError>
{
    // TODO(M2 第 3 步): 学生实现（步骤见上方注释）
    unimplemented!("M2 学生实现：删除动作")
}

// ============================================================
// 动作详情（GET /exercises/{id}）—— M5 占位
// ============================================================
/// 显示单个动作的详细信息（M5 扩展为图表/历史，先占位）
///
/// 【教学：detail 为什么现在只占位？】
/// M2 的动作库只需要 CRUD + 筛选。动作详情页的真正价值是
/// "这个动作的历史表现"（趋势图、最大重量等）——那需要 M4 的记录
/// 数据，现在没有。所以先返回最简单的信息页，M5 再扩展。
/// 这就是"占位实现"：接口先立住（路由、签名、返回类型定型），
/// 内容后续填充，避免以后改接口。
///
/// 【实现步骤】（老师实现）
/// 1. 签名：State + AuthUser + Path(id)
/// 2. 查这一行：SELECT * FROM exercises WHERE id = ? AND user_id = ?
///    → fetch_optional → None 则 Err(NotFound)
/// 3. 返回信息页：动作名 + 全部字段
pub async fn detail(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(id): Path<i64>,
) -> Result<String, AppError>
{
    let exercise =
        sqlx::query_as::<_, Exercise>("SELECT * FROM exercises WHERE id = ? AND user_id = ?")
            .bind(id)
            .bind(user.id)
            .fetch_optional(&state.pool)
            .await
            .map_err(AppError::Database)?
            .ok_or_else(|| AppError::NotFound("动作不存在".to_string()))?;

    Ok(format!(
        r#"
        <h1>动作详情</h1>
        <p>名称：{name}</p>
        <p>部位：{body_part}</p>
        <p>默认模式：{default_mode}</p>
        <p>默认杆重：{bar_weight}</p>
        <p>默认组数：{default_sets}</p>
        <p>默认次数：{default_reps}</p>
        <p>要领：{key_points}</p>
        <p><a href="/exercises">返回列表</a></p>
        "#,
        name = exercise.name,
        body_part = exercise.body_part,
        default_mode = exercise.default_mode,
        bar_weight = exercise.bar_weight,
        default_sets = exercise.default_sets,
        default_reps = exercise.default_reps,
        key_points = exercise.key_points,
    ))
}
