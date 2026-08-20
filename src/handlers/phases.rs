// ============================================================
// handlers/phases.rs —— 训练阶段（Phase）的 HTTP 处理器
// ============================================================
// 【教学说明】
// 这个文件处理"与阶段相关的 HTTP 请求"：
//   GET  /phases              → 阶段列表（list）
//   GET  /phases/new          → 创建阶段表单页（create_form）
//   POST /phases              → 创建阶段（create）
//   GET  /phases/{id}/edit    → 编辑表单页（edit_form）
//   POST /phases/{id}/edit    → 更新阶段（update）
//   POST /phases/{id}/archive → 归档（archive，archived=1）
//   POST /phases/{id}/unarchive → 重新启用（unarchive，archived=0）
//
// 7 个函数，前 5 个是标准 CRUD，后 2 个是"状态切换"。
//
// 📌 阶段要求：M2 你来实现本文件所有函数。
//   完整实现已备份在 docs/learning_path/M2_ref/phases_ref.rs，
//   实现完成后对照检查（不要提前看）。
// ============================================================

// 【教学：本文件用到的导入】
// 和 M1 的 auth.rs 对比，多了一个 Path——这是"从 URL 拿参数"的提取器：
//   /phases/{id}/edit  →  Path(id): Path<i64>  →  id 就是 URL 里的数字
// 少了 HeaderMap + require_user：因为 M2 改用 AuthUser 提取器做守卫，
// 签名里写 AuthUser(user) 就是"已登录用户"，axum 自动注入。
use axum::{
    extract::{Form, Path, State},
    response::{Html, Redirect},
};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::{
    AppState,
    error::AppError,
    handlers::auth::AuthUser, // M2 第 1 步的成果：声明式守卫提取器
    models::Phase,
};

// ============================================================
// 【教学：HTML 快速入门（零基础版）】
// 学生："我没接触过 HTML，能简单介绍一下吗？"
//
// HTML = 用"标签"描述网页结构的语言，浏览器读它渲染成页面。
//
// 基本规则：标签成对出现
//   <p>这是一段文字</p>   ← 开始标签 + 内容 + 结束标签（/ 表示结束）
// 少数标签自闭合（没有内容）：<br>（换行）、<input>（输入框）
//
// 标签可以有属性（配置项）：
//   <a href="/phases">阶段列表</a>   ← href：链接指向哪
//   <input name="username">          ← name：这个输入框叫什么（Rust 靠它对接！）
//
// 本项目用到的标签（就这些）：
//   <h1>~<h6>   标题（1 最大）       <h2>进行中</h2>
//   <p>         段落                 <p>欢迎回来</p>
//   <a href=""> 超链接               <a href="/phases/new">创建阶段</a>
//   <table><tr><td><th>  表格：行/单元格/表头
//   <form method="post" action="/phases">  表单容器（提交时 POST 到 action 地址）
//   <input name="xxx">    输入框
//   <textarea name="note"> 多行文本框（备注用）
//   <button type="submit"> 提交按钮（点它触发表单提交）
//   <br>        换行
//
// 表单与 Rust 对接的关键（务必理解）：
//   <form method="post" action="/phases">
//     <label>名称 <input name="name"></label><br>
//     <label>备注 <textarea name="note"></textarea></label><br>
//     <button type="submit">创建</button>
//   </form>
//   点"创建"按钮 → 浏览器收集所有 <input name="..."> 的值
//   → 组成 name=xxx&note=yyy → POST 到 /phases
//   → axum 的 Form<PhaseForm> 按 name 字段名匹配填充结构体
//   所以：HTML 的 name 属性 必须和 Rust 结构体字段名一致，否则 400！
//
// 我们项目怎么"写" HTML？
//   不手写 .html 文件，而是在 Rust 代码里用 format! 拼字符串
//   （服务端渲染）：查数据库 → 数据填进 HTML 模板 → 返回给浏览器。
//   你看到的 format!(r#"..."#) 就是干这个的。
// ============================================================

// ============================================================
// 【教学：请求-响应模型 —— 用户点击后数据是怎么到 create 的？】
// 学生问："create_form 展示了 HTML 界面，那么程序是通过什么来接收
//           用户在 HTML 界面的点击和输入的呢？接收后又是如何传给 create 的？
//           现在这个进度需不需要关心这些东西？"
//
// 一句话答案：**收集输入的是浏览器，不是你的 Rust 代码。**
// 你写的 HTML 不是在"接收"输入——它是在"告诉浏览器怎么收集、发往哪里"。
// 就像你写一份问卷（HTML），真正向受访者收问卷并寄出去的，
// 是快递员（浏览器）。
//
// 完整流程（9 步）：
//   ① 用户打字、点击        ← 浏览器里发生，Rust 完全不知情！
//   ② 浏览器收集输入         ← 按 HTML 规范：收集所有 <input name="...">
//                              的值，拼成 name=xxx&note=yyy（urlencoded）
//   ③ 浏览器发 HTTP 请求     ← POST /phases，请求体就是②拼的字符串
//   ④ axum 路由匹配          ← Router 里 "POST /phases" → create 函数
//   ⑤ Form<PhaseForm> 提取器 ← 解析请求体，按 name 字段名填充结构体
//   ⑥ 你的代码：INSERT 入库
//   ⑦ 302 重定向到 /phases
//   ⑧ 浏览器自动发 GET /phases（无需用户操作）
//   ⑨ 服务器返回列表页 HTML，浏览器渲染
//
// 你只需要关心"声明契约"：
//   - HTML 里 <input name="name">    声明"这里有字段叫 name"
//   - 函数签名 Form(form): Form<PhaseForm> 声明"我接收 PhaseForm"
//   - 契约的纽带：**name 属性必须等于结构体字段名**
//   浏览器负责收集、axum 负责解析装配——两层都按标准工作，你信任它们即可。
//
// 类比 C++ 的回调：你注册一个回调（Form<PhaseForm> 签名），
// 运行时框架（axum）在事件（HTTP 请求到达）发生时调用它。
// 区别：C++ 回调是进程内事件，Web 回调是跨进程请求（浏览器在另一台机器！）。
// Form<PhaseForm> 就是 axum 版的"参数自动装配"——
// 你永远不需要手动解析 name=xxx&note=yyy。
//
// 现在需不需要关心？分三层：
//   需要理解（概念层）    ：请求-响应模型、GET vs POST、action 指向、
//                            name 属性对接——这些你已经掌握了。
//   暂不关心（实现层）    ：urlencoded 格式细节、HTTP 报文长什么样、
//                            监听端口/并发——都是 axum 内部，M2 不需要。
//   M5 才深入             ：前端框架（React/Vue），那时你才写
//                            "浏览器端收集输入"的 JS 代码，POST JSON 给后端。
// ============================================================

// ============================================================
// 【教学：表单数据结构 —— 为什么 create 和 update 共用 PhaseForm？】
// 创建阶段（POST /phases）和更新阶段（POST /phases/{id}/edit）
// 提交的字段一模一样（name/note/start_date），所以共用一个表单结构体。
// 区别只在 SQL：create 是 INSERT，update 是 UPDATE + WHERE id = ?。
//
// 【教学：start_date 为什么是 String 而不是 Option<String>？】
// HTML 表单里 <input type="date"> 不填时提交的是**空字符串 ""**，
// 不是"缺失"。如果字段类型是 Option<String>，axum 会因空字符串
// 无法反序列化成 None 而报 400。所以表单层用 String 接收，
// 存库前再转 Option：空串 → None，非空 → Some(值)。
// （模型层 Phase.start_date 是 Option<String>，表单层是 String，
//   两层职责不同：模型层表达"可空列"，表单层表达"表单原样提交"。）
// ============================================================
#[derive(Deserialize)]
pub struct PhaseForm
{
    name: String,
    note: String,
    start_date: String, // 表单原样：空串或 'YYYY-MM-DD'
}

// ============================================================
// 阶段列表（GET /phases）
// ============================================================
/// 显示当前用户的阶段列表（进行中 / 已归档 两个分区）
///
/// 【教学：三步走（M1 已学过）】
///   ① 守卫：AuthUser(user) 在签名里，axum 自动验证（失败 401 进不来）
///   ② 查数据：fetch_all → Vec<Phase>
///   ③ 拼页面：迭代器 map → HTML 表格行
///
/// 【教学：数据隔离 —— WHERE user_id = ? 是安全底线】
/// 所有阶段查询都必须带 user_id 条件，绝不能只 SELECT * FROM phases：
///   SELECT * FROM phases                          ❌ 会查出所有用户的阶段！
///   SELECT * FROM phases WHERE user_id = ?        ✅ 只查当前用户的
/// user_id 从哪来？AuthUser(user).0.id —— user 是 User，user.id 是当前登录者。
/// （AuthUser 是元组结构体，AuthUser(user) 解构后 user 就是 User。）
///
/// 【教学：ORDER BY —— 列表的"顺序感"】
/// ORDER BY created_at DESC：最新的排最上面（倒序）。
/// 用户看到的列表应该是"最近创建的阶段"在最前，而不是随机顺序。
///
/// 【教学：两次查询 vs 一次查询】
/// 进行中/已归档要分两个区显示。两种做法：
///   做法 A：查两次（各带 WHERE archived = 0/1）→ 代码直观，各查各的
///   做法 B：查一次全部，在 Rust 里 partition 分两堆 → 少一次数据库往返
/// 本项目用做法 A（简单直观），M5 数据量大时可优化为 B。
///
/// 【教学：为什么这里返回 String，而 M1 的 home 返回 Response？】
/// 学生提问："之前 home 把 String 改成了 Response，这里为什么不改？"
///
/// 判断标准：数一下成功分支里有几种返回值。
///   home（M1）：成功分支有两种
///     - 已登录   → 欢迎语 HTML（String/Html）
///     - 未登录   → Redirect::to("/login")（重定向）
///     两种值类型不同，String 装不下，只能统一转成 Response：
///       Ok(Html(...).into_response())
///       Ok(Redirect::to("/login").into_response())
///
///   list（M2）：成功分支只有一种
///     - 返回阶段列表 HTML（String）
///     没有 Redirect（list 是 GET 展示页，不处理提交、不重定向），
///     只有一种类型 → 直接返回 String，axum 自动当 text/html 响应。
///     用 Response 反而多余（还要多导入 Html/IntoResponse/Response）。
///
///   create/update/archive/unarchive：成功分支只有 Redirect
///     → Result<Redirect, AppError>，同理，只有一种类型就不用 Response。
///
/// 💡 口诀：数成功分支。
///    1 种 → 用那个具体类型（String / Redirect / Html）
///    2 种+ → 统一转 Response（.into_response()）
/// 这也是 Rust 的"最少必要类型"思想：能用简单类型就不用复杂类型，
/// 简单类型意味着更少的导入、更少的转换、更少的出错面。
///
/// 【教学：Vec<Phase> → HTML 字符串 —— map → collect → join】
/// 学生提问："想把 vector 转成 string 直接放 html 里，忘了适配器是什么"
///   + 追问："两个 phases 是什么类型？为什么 collect 后再 join，直接 join 不行吗？"
///
/// 链条是三个适配器接力（M1 的 admin_users 已用过）：
///   phases.iter()                              // Vec<Phase> → Iter<'_, Phase>
///     .map(|p| format!("<tr>...</tr>", ...))   // 每个阶段 → 一行 HTML（Phase → String）
///     .collect::<Vec<_>>()                     // 迭代器 → Vec<String>
///     .join("\n")                              // Vec<String> → String（换行连接）
///
/// 每个适配器的职责：
///   map     = 逐个转换（惰性！只登记"转换规则"，不真正执行）
///   collect = 把迭代器"收"成集合（驱动执行的关键，没有它 map 白写）
///   join    = Vec<String> → String（换行连接成一个大字符串）
/// format! 占位符 {} 接收 String，正好直接放进 HTML。
///
/// 【追问 1：两个 phases 是什么类型？】
///   active_phases 和 archived_phases 都是 Vec<Phase>
///   （sqlx 的 fetch_all 返回的就是 Vec<Phase>，一行 → 一个结构体）。
///
/// 【追问 2：为什么不能直接 join？】
/// 关键：join 是"切片（数组/Vec）的方法"，不是"迭代器的方法"。
///   phases.iter()          → 迭代器（Iter<'_, Phase>）    ← 迭代器没有 join！
///   .map(...)              → 迭代器（元素 Phase → String）← 还是没有 join！
///   .collect::<Vec<_>>()   → Vec<String>（变成数组了）    ← 数组才有 join
///   .join("\n")            → String
/// 三个原因：
///   ① 标准库里 join 定义在 [T]（切片）上，不在 Iterator trait 上。
///      Vec<String> 能调 join 是自动 deref 到 &[String]。
///      （类比你熟悉的 C++：vector 有 push_back，迭代器没有——
///        方法和"容器 vs 遍历器"绑在一起。）
///   ② map 是惰性的，不 collect 不执行（collect 兼当"引擎 + 容器"）。
///   ③ join 要"看全部元素"才能串起来；迭代器是"一个接一个吐"的流，
///      没有整体视野；数组是"全部在内存里"，才能做整体拼接。
/// 💡 一句话：迭代器负责"逐个处理"，数组负责"整体操作"。
///    想 join（整体拼接），先把流 collect 成数组。
///
/// 常见错误：只写 map 不写 collect/join，然后问"为什么没变化"——
///   因为 map 是惰性的，没人消费它就不执行。
///
/// 【实现步骤】
/// 1. 守卫：签名里 AuthUser(user): AuthUser 已自动完成
/// 2. 查进行中：
///    let active = sqlx::query_as::<_, Phase>(
///        "SELECT * FROM phases WHERE user_id = ? AND archived = 0 ORDER BY created_at DESC")
///        .bind(user.id).fetch_all(&pool).await.map_err(AppError::Database)?;
/// 3. 查已归档（同上，archived = 1）
/// 4. 拼 HTML：两个分区各一个 <h2> + 表格，active/archived 各自迭代 map
/// 5. 返回完整页面字符串
pub async fn list(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
) -> Result<Html<String>, AppError>
{
    let pool = state.pool.read().await.clone();

    let active_phases = sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived  = 0 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::Database)?;

    let archived_phases = sqlx::query_as::<_, Phase>(
        "SELECT * FROM phases WHERE user_id = ? AND archived  = 1 ORDER BY created_at DESC",
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .map_err(AppError::Database)?;

    // Vec<Phase> → 表格行 HTML（map → collect → join，三步接力）
    // 【教学：功能入口的"归属"设计 —— 为什么模板/计划链接不放在首页？】
    // 模板和计划都"挂"在某个阶段下（templates.phase_id / plans.phase_id），
    // 离开阶段谈模板/计划没有意义。所以它们的入口放在【阶段列表的每一行】：
    //   "训练模板" → /phases/{id}/templates
    //   "训练计划" → /phases/{id}/plans
    // 这样用户点进一个阶段，立刻能看到这个阶段下的模板和计划。
    // 首页只放"大入口"（阶段/动作），细粒度入口放在各自归属的页面——导航不迷路。
    // 进行中列表：空 → 友好提示；非空 → 表格行
    // 【M7 第 4 步：空态 —— 空列表不再是一张空表格】
    let active_rows = if active_phases.is_empty()
    {
        r#"<tr><td colspan="4" class="empty-tip">还没有进行中的阶段，去创建一个吧</td></tr>"#
            .to_string()
    }
    else
    {
        active_phases
            .iter()
            .map(|p| {
                format!(
                    "<tr><td>{id}</td><td>{name}</td><td>{note}</td>\
                    <td><a href=\"/phases/{id}/templates\">训练模板</a> \
                    <a href=\"/phases/{id}/plans\">训练计划</a> \
                    <a href=\"/phases/{id}/edit\">编辑</a></td></tr>",
                    id = p.id,
                    name = p.name,
                    note = p.note
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    // 已归档：只读，操作列只给查看入口（不能编辑——归档阶段不可修改）
    let archived_rows = if archived_phases.is_empty()
    {
        r#"<tr><td colspan="4" class="empty-tip">暂无已归档阶段</td></tr>"#.to_string()
    }
    else
    {
        archived_phases
            .iter()
            .map(|p| {
                format!(
                    "<tr><td>{id}</td><td>{name}</td><td>{note}</td>\
                    <td><a href=\"/phases/{id}/templates\">训练模板</a> \
                    <a href=\"/phases/{id}/plans\">训练计划</a> \
                    <span style=\"color:gray\">（只读）</span></td></tr>",
                    id = p.id,
                    name = p.name,
                    note = p.note
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(Html(format!(
        r#"{head}
        <h2>进行中</h2>
            <table border="1"><tr><th>ID</th><th>名称</th><th>备注</th><th>操作</th></tr>
                {active_rows}
            </table>
        <h2>已归档</h2>
            <table border="1"><tr><th>ID</th><th>名称</th><th>备注</th><th>操作</th></tr>
                {archived_rows}
            </table>
        <p><a href="/phases/new">创建阶段</a></p>
        <p><a href="/">返回首页</a></p>"#,
        head = crate::page::page_head("训练阶段"),
    )))
}

// ============================================================
// 创建阶段表单页（GET /phases/new）
// ============================================================
/// 显示"创建阶段"表单页
///
/// 【教学：表单页 vs 提交处理 —— 两个 handler 的分工】
///   GET /phases/new   → create_form：只显示表单（用户填写）
///   POST /phases      → create：接收表单数据并入库
/// 一个"创建动作"拆成 GET + POST 两个 handler，这是 Web 表单的标准模式：
///   GET  = "给我看表单"（无副作用，可刷新、可收藏）
///   POST = "处理提交"  （有副作用，刷新会重复提交，所以提交后必须重定向）
/// 后面 create/edit_form/update 同理，都是"显示表单"和"处理提交"分离。
///
/// 【教学：为什么这个 handler 也要 AuthUser 守卫？】
/// 只是显示个表单而已，也要登录吗？要！因为：
///   1. 页面顶部要显示"当前用户是谁"（可能还要显示登出按钮）
///   2. 未登录用户看到创建表单毫无意义——提交时反正会被 401 拦下
/// 所以 M2 的页面，凡是"登录后才有意义"的都加 AuthUser 守卫。
///
/// 【教学：这个表单要放什么？—— 表单字段 = PhaseForm 结构体字段】
/// 学生问："create_form 要放些什么？要创建什么表单？"
///
/// 答案：表单字段 = 你要创建的对象的属性 = PhaseForm 的三个字段。
/// 创建"阶段"需要用户填什么，表单就放什么——没有其他！
///   PhaseForm { name, note, start_date }   ← 就这三个
/// 具体对应：
///   name        → <input name="name">                 （阶段名，必填）
///   note        → <textarea name="note">              （备注，可空）
///   start_date  → <input type="date" name="start_date">（开始日期，可空）
///
/// 完整表单 HTML 模板（可直接抄）：
///   <h1>创建阶段</h1>
///   <form method="post" action="/phases">
///     <label>阶段名 <input name="name" required></label><br>
///     <label>备注 <textarea name="note"></textarea></label><br>
///     <label>开始日期 <input type="date" name="start_date"></label><br>
///     <button type="submit">创建</button>
///   </form>
///   <p><a href="/phases">返回列表</a></p>
///
/// 几个要点：
///   ① action="/phases"：提交到 POST /phases（create 处理它）
///   ② method="post"：必须 POST（create 的签名要 Form<PhaseForm>）
///   ③ name 属性 = PhaseForm 字段名：name / note / start_date，一字不差！
///      （对不上 axum 反序列化失败 → 400）
///   ④ required：HTML 自带的必填校验（空值根本提交不了）
///   ⑤ <input type="date">：浏览器渲染成日期选择器，提交 'YYYY-MM-DD'
///   ⑥ 页面顶部可显示"欢迎，{user.username}"（user 已从 AuthUser 解构出来）
///      外加一个 <a href="/logout"> 需要 POST——先放 <a href="/"> 即可，
///      登出按钮是第 5 步首页改造的事，这里不弄复杂。
///
/// 【教学：写 HTML 模板的两个编译错误 —— 学生实现踩坑实录】
/// 学生实现 create_form 时，编译器报了一串错误，根源只有两个：
///
/// 【坑 1：原始字符串前缀是 r#，不是 #】
///   学生写：Ok(Html(#" ... "#))
///              ↑ 少了 r
///   r#"..."# 才是原始字符串（r = raw，类比 C++ 的 R"(...)"）。
///   只写 #"，编译器把 # 当成"自定义字符串前缀"，报：
///     error: prefix `phases` is unknown
///   后面一串 error 都是这一个错误引发的连锁反应（编译器懵了，
///   一路把字符串当代码解析）。
///   口诀：r 表示 raw，见到 #" 先问自己"r 在哪？"
///
/// 【坑 2：返回类型是 Result<String, AppError>，Ok 里必须装 String】
///   学生写 Ok(Html("..."))，两个问题：
///   ① Html 是 axum 的响应包装类型（要额外导入），不是 String，
///      直接放 Ok 里类型不匹配；
///   ② r#"..."# 的类型是 &'static str（字符串切片引用），
///      也不是 String，要 .to_string() 转换。
///   正确写法：Ok(r#"..."#.to_string())
///   再念一遍口诀：数成功分支，1 种 → 用具体类型（这里是 String）。
///
/// 【小问题：HTML 标签没闭合】
///   学生写 <p><a href="/phases">返回</a><p>，结尾应是 </p> 不是 <p>。
///   浏览器对未闭合标签很宽容（会自动补），但养成闭合的习惯：
///   每个开始标签必有对应结束标签，写 <p> 就检查有没有 </p>。
///
/// 【实现步骤】
/// 1. 签名：State(state) + AuthUser(user)（守卫 + 拿当前用户）
/// 2. 返回一个包含 <form method="post" action="/phases"> 的 HTML 字符串
///    （表单字段：name 文本、note 文本域、start_date 日期、提交按钮）
/// 3. 页面里可显示"欢迎，{user.username}"（user 已解构出来）
pub async fn create_form(
    State(_state): State<AppState>,
    AuthUser(_user): AuthUser,
) -> Result<Html<String>, AppError>
{
    Ok(Html(format!(
        r#"
        {head}
        <h1>开启新征程</h1>
        <form method="post" action="/phases">
            <label>名称 <input name="name" required></label><br>
            <label>备注 <textarea name="note"></textarea></label><br>
            <label>开始日期 <input type="date" name="start_date"></label><br>
            <button type="submit">创建</button>
        </form>
        <p><a href="/phases">返回</a></p>
        "#,
        head = crate::page::page_head("创建训练阶段"),
    )))
}

// ============================================================
// 创建阶段（POST /phases）
// ============================================================
/// 处理创建阶段表单提交
///
/// 【教学：创建 = INSERT + 重定向（POST-Redirect-GET 模式）】
/// 处理 POST 提交的标准流程：
///   1. 校验数据（name 不能为空）
///   2. INSERT 入库（带 user_id —— 归属当前用户！）
///   3. 重定向到列表页（Redirect::to("/phases")）
/// 为什么提交成功后必须重定向？因为 POST 是"有副作用"的请求，
/// 如果直接返回页面，用户按 F5 刷新会**再次提交**（重复创建）。
/// 重定向后浏览器发 GET /phases，刷新就安全了。
/// 这叫 PRG 模式（Post/Redirect/Get），Web 开发的铁律。
///
/// 【教学：start_date 空串 → None 的转换】
/// 表单提交的是 String（空串或日期），数据库列可空（TEXT NULL）。
/// 入库前必须转换：
///   let start_date = if form.start_date.trim().is_empty() { None } else { Some(form.start_date) };
/// 如果直接把空串塞进列，数据库存的是 "" 而不是 NULL，
/// 后续"start_date 为空 = 未设开始日期"的判断就会失效。
///
/// 【教学：name 唯一性 —— 同用户下阶段名不能重复】
/// phases 表有 UNIQUE(user_id, name) 约束（0001_init.sql）。
/// 重名时 INSERT 会报"UNIQUE constraint failed"——这是数据库层的兜底。
/// 更好的体验是入库前先查重（SELECT ... WHERE user_id = ? AND name = ?），
/// 命中则返回 Validation 错误。这样用户看到的是"阶段名已存在"，
/// 而不是数据库报错（500）。
///
/// 【教学：空日期 → None 还是 → 当前日期？—— 设计决策】
/// 学生提问："如果为空，返回 None 就相当于传到了上一层，
///              如果说在这一层把空日期转换为当前日期呢？"
///
/// 先纠正一个误解：None 不是"传到上一层"。
///   Option<String> 的 None 在这一层就定值了，存进 SQLite 就是 NULL，
///   不会"冒泡"到别处。真正"传到上一层"的是 ? 运算符（提前返回错误）。
///
/// 两种方案的业务语义不同：
///   转 None（本项目设计）    → 库里 NULL  = "开始日期未设置"
///   转当前日期（学生想法）    → 库里日期  = "开始日期 = 创建当天"
///
/// 为什么本项目选 None：
///   ① 语义干净：NULL 明确表示"没填"，可做"未开始阶段"统计；
///   ② 信息不丢失：转成今天后，无法区分"用户填的今天"和"默认的今天"；
///   ③ 日期"未定"是真实业务状态——用户可能还没决定哪天开始。
///
/// 如果想"默认今天"，更好的做法是前端 <input type="date"> 预填今天的
/// 日期（用户看得见、可修改），而不是后端偷偷塞值。
/// （time 库在 Cargo.toml 里，可用：
///    time::OffsetDateTime::now_utc().date().to_string() → "2026-08-06"）
///
/// 【学生代码的语法错误：方法调用必须带 ()】
///   .is_empty { ... }   ❌ 缺括号！
///   .is_empty() { ... } ✅
/// .is_empty 是方法（函数），调用要写 ()。不写 = 把函数本身当值用，
/// 类型是 fn(&str) -> bool，放进 if 条件报类型错误。
/// （C++ 里你传成员函数指针才这么写，Rust 里方法调用一律带括号。）
///
/// 【教学：bind Option 直接处理 NULL —— 学生踩坑实录】
/// 学生实现 INSERT 时，把 start_date 的 Option 用 match 拆开再 bind：
///   .bind(match start_date { Some(dt) => dt, None => "NULL".to_string() })
///
/// 两个问题：
///   ① bind("NULL") 存的是**文本 'NULL'**，不是数据库空值 NULL！
///      库里变成五个字母的字符串，WHERE start_date IS NULL 永远查不到它。
///      bind 的职责是安全传值（防注入），bind 什么就存什么——
///      字符串就是字符串，绝不会有"字符串自动变成空值"的魔法。
///   ② 完全多余！sqlx 对 Option<T> 有原生支持：
///        .bind(&start_date)  // Some("2026-08-06") → 存日期
///                            // None              → 存 SQL NULL
///      这就是为什么设计上要"先转换 Option"——让 bind 直接处理
///      None → NULL 的映射，不需要手动 match。
///
/// 【教学：bind 传引用还是按值移动？—— 学生思考实录】
/// 学生："我想到了移动语义的问题，但后面这些变量都不用了，
///        就直接按移动语义传递了。"
///
/// 学生观察正确：走到 INSERT 时，form 的字段确实不再使用了。
/// bind(form.name) 按值移动（String 非 Copy，传值=移动），
/// 之后 form.name 不可用——但这行之后 form 确实没用了，完全合法。
///
/// 两种写法都编译通过、逻辑等价：
///   引用版  bind(&form.name)  统一用引用，规整一致
///   移动版  bind(form.name)   语义精确（"最后一次，直接交出去"）
///
/// 本项目选**引用版**，理由：
///   ① 一致性：user.id 是 Copy 只能传值，state.pool 必须借引用，
///      字段类型不同本来就会混用；全用引用减少"为什么这个没有 &"的疑问。
///   ② 改动成本：想移动 form.name，form.note、start_date 也得跟着想，
///      收益只是少打 3 个 &，不值得。
///   ③ 教学上：所有权/move 的完整细节 M3（闭包）再展开，这里先统一借用。
///
/// 一句话：能用引用就用引用，明确"最后一步转移所有权"时才 move。
/// （若想改成移动版：bind(form.name) / bind(form.note) / bind(start_date)
///   去掉 & 即可，后面都不再使用这些变量。）
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Form<PhaseForm>
/// 2. 校验：name 非空（trim().is_empty()），空则 Err(Validation)
/// 3. 查重：SELECT id FROM phases WHERE user_id = ? AND name = ?
///    → 查到就 Err(Validation("阶段名已存在"))（用 fetch_optional）
/// 4. 转换 start_date：空串 → None
/// 5. INSERT INTO phases (user_id, name, note, start_date) VALUES (?, ?, ?, ?)
///    .bind(user.id).bind(&form.name).bind(&form.note).bind(&start_date)
/// 6. Ok(Redirect::to("/phases")) —— 回到列表页
pub async fn create(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Form(form): Form<PhaseForm>,
) -> Result<Redirect, AppError>
{
    let pool = state.pool.read().await.clone();

    // 校验：name 非空（空则立刻返回 422）
    if form.name.trim().is_empty()
    {
        return Err(AppError::Validation("阶段名称不能为空".to_string()));
    }
    // 查重：查到重名 → 422（is_some() 压成 bool，链不打断）
    if sqlx::query_scalar::<_, i64>("SELECT id FROM phases WHERE user_id = ? AND name = ?")
        .bind(user.id)
        .bind(&form.name)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::Database)?
        .is_some()
    {
        return Err(AppError::Validation("阶段名已存在".to_string()));
    }
    // 转换 start_date：空串 → None（"未设置"是真实业务状态）
    let start_date = if form.start_date.trim().is_empty()
    {
        None
    }
    else
    {
        Some(form.start_date.trim().to_string())
    };
    sqlx::query("INSERT INTO phases (user_id, name, note, start_date) VALUES (?, ?, ?, ?)")
        .bind(user.id)
        .bind(&form.name)
        .bind(&form.note)
        .bind(&start_date)
        .execute(&pool)
        .await
        .map_err(AppError::Database)?;
    Ok(Redirect::to("/phases"))
}

// ============================================================
// 编辑阶段表单页（GET /phases/{id}/edit）
// ============================================================
/// 显示"编辑阶段"表单页（预填当前值）
///
/// 【教学：Path 提取器 —— 从 URL 拿参数】
/// 路由 /phases/{id}/edit，axum 把 URL 里 {id} 部分解析出来：
///   /phases/3/edit  →  Path(3): Path<i64>  →  id = 3
/// 语法要点（axum 0.8）：
///   - 路由用 {id} 占位符（旧版 :id 已废弃）
///   - handler 参数 Path(id): Path<i64>，i64 是目标类型
///   - URL 里不是数字（如 /phases/abc/edit）→ 提取失败 → 400
///
/// 【教学：编辑页为什么先查一次库？】
/// 编辑表单要"预填当前值"——用户看到的是"旧值，改了提交"。
/// 所以 edit_form 要先按 id 查出这条阶段，把 name/note/start_date
/// 填进 HTML 的 value 属性。
/// 这一步查库和 list 不同：只查一行 → fetch_optional。
///
/// 【教学：查不到怎么办 —— 404 语义】
/// 如果 id 对应的阶段不存在（被删了？URL 乱敲？）：
///   fetch_optional 返回 None → Err(AppError::NotFound("阶段不存在"))
/// 用户看到的是 404 页面，而不是空白页或数据库错误。
/// （AppError::NotFound 是 M1 定义好的，见 error.rs。）
///
/// 【教学：编辑表单的"预填旧值" —— 三个 HTML 细节】
/// 编辑表单 = 创建表单 + 旧值回填。回填有三种方式，要分清：
///
/// ① <input> 用 value 属性回填：
///      <input name="name" value="{phase.name}">
///    用户打开页面时，输入框里已经显示旧阶段名，改完提交。
///
/// ② <textarea> 没有 value 属性！旧值放在开始/结束标签之间：
///      <textarea name="note">{phase.note}</textarea>
///    （textarea 是"容器"型标签，内容写在标签中间，
///      input 是"自闭合"型标签，内容写在 value 属性里——
///      这是 HTML 设计的历史遗留，记住即可。）
///
/// ③ <input type="date"> 的 value 格式必须是 'YYYY-MM-DD'：
///      phase.start_date 是 Option<String>（数据库可空列），
///      直接插进 value 会变成 "Some(2026-08-06)" 或者 "None" 文本！
///      要先解包：
///        let start_date = phase.start_date.as_deref().unwrap_or("");
///      as_deref() 把 Option<String> 变成 Option<&str>（借用，不克隆），
///      unwrap_or("") 是 None 时的兜底（空值 = 日期框留空）。
///      这一句是"Option 解包三连"，M3 会再见到。
///
/// ④ action 指向当前阶段的更新地址：
///      <form method="post" action="/phases/{id}/edit">
///    用 format! 把 id 拼进去——每个阶段编辑自己的那条记录。
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 查这一行（带 user_id 数据隔离）：
///    SELECT * FROM phases WHERE id = ? AND user_id = ?
///    → fetch_optional → None 则 Err(NotFound)
/// 3. 拼表单：value="{phase.name}" 等预填旧值，action="/phases/{phase_id}/edit"
pub async fn edit_form(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Html<String>, AppError>
{
    let pool = state.pool.read().await.clone();

    let phase = sqlx::query_as::<_, Phase>("SELECT * FROM phases WHERE id = ? AND user_id = ?")
        .bind(&phase_id)
        .bind(&user.id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("No such phase".to_string()))?;

    // Option<String> 解包：Some → 日期字符串，None → ""（日期框留空）
    let start_date = phase.start_date.as_deref().unwrap_or("");

    Ok(Html(format!(
        r#"
        {head}
        <h1>编辑训练阶段</h1>
        <p>欢迎，{username} —— 正在编辑：{phase_name}</p>
        <form method="post" action="/phases/{phase_id}/edit">
            <label>名称 <input name="name" value="{phase_name}" required></label><br>
            <label>备注 <textarea name="note">{phase_note}</textarea></label><br>
            <label>开始日期 <input type="date" name="start_date" value="{start_date}"></label><br>
            <button type="submit">提交</button>
        </form>
        <p><a href="/phases">返回列表</a></p>
        "#,
        head = crate::page::page_head("编辑训练阶段"),
        username = user.username,
        phase_name = phase.name,
        phase_note = phase.note,
        start_date = start_date,
        phase_id = phase_id,
    )))
}

// ============================================================
// 更新阶段（POST /phases/{id}/edit）
// ============================================================
/// 处理编辑表单提交，更新阶段信息
///
/// 【教学：UPDATE 的三个要点】
/// 1. WHERE id = ? AND user_id = ? —— 双重条件！
///    只按 id 更新，可能改到别人的阶段（id 是全局递增的）。
///    必须带 user_id，保证"只能改自己的"。
/// 2. 归档阶段禁止编辑 —— 已归档是"历史快照"，改了会破坏历史。
///    实现：update 前先查 archived 字段，为 1 则 Err(Forbidden)。
/// 3. 更新后重定向到列表页（PRG 模式，同 create）。
///
/// 【教学：UPDATE 怎么知道"改了几行"？】
/// execute() 返回 SqliteQueryResult，它有 rows_affected()：
///   0 行被改 → id 不存在或不是自己的（返回 NotFound）
///   1 行被改 → 正常
/// （这比"改前查一次"更省：一次 SQL 搞定"存在性 + 更新"。）
///
/// 【实现步骤】
/// 1. 签名：State + AuthUser + Path(phase_id) + Form<PhaseForm>
/// 2. 校验 name 非空
/// 3. 先查归档状态：
///    SELECT archived FROM phases WHERE id = ? AND user_id = ?
///    → fetch_optional（None → NotFound；Some(true) → Forbidden("阶段已归档"))
/// 4. 转换 start_date（空串 → None）
/// 5. UPDATE phases SET name = ?, note = ?, start_date = ? WHERE id = ? AND user_id = ?
/// 6. rows_affected() == 0 → Err(NotFound)；否则 Ok(Redirect::to("/phases"))
pub async fn update(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
    Form(form): Form<PhaseForm>,
) -> Result<Redirect, AppError>
{
    let pool = state.pool.read().await.clone();

    // 校验 name 非空（空则立刻返回 422）
    if form.name.trim().is_empty()
    {
        return Err(AppError::Validation("阶段名称不能为空".to_string()));
    }
    // 查归档状态：查不到 → 404，已归档 → 403
    // 【教学：await/map_err/ok_or_else 顺序 —— 学生踩坑实录】
    // 学生第一版把顺序写成 .map_err(...).await?，E0599 三连：
    //   ① map_err 必须在 await 之后！fetch_optional 返回 Future（还没执行），
    //      先 .await 拿到 Result，才能 map_err。
    //      口诀：await 紧贴查询方法，map_err 在 await 之后。
    //   ② ok_or_else 必须在 ? 之前！? 会把 Option 解包出来，解包后再
    //      ok_or_else 就是给 bool 调方法。
    // 学生第二版又踩了【类型推断陷阱】：
    //   没写 query_scalar::<_, bool>，却写 == Some(true)。
    //   编译器看到 Some(true)，把泛型 T 推断成 Option<bool>！
    //   .ok_or_else? 解包的是"查不查得到"那层，
    //   内层 Option 还在 → 碰巧能编译、碰巧逻辑对（archived 列 NOT NULL），
    //   但这是运气代码：T 的真实类型和你的意图不一致。
    //   教训：泛型查询永远显式写 ::<_, 类型>，别让编译器猜。
    if sqlx::query_scalar::<_, bool>("SELECT archived FROM phases WHERE id = ? AND user_id = ?")
        .bind(phase_id)
        .bind(user.id)
        .fetch_optional(&pool)
        .await
        .map_err(AppError::Database)?
        .ok_or_else(|| AppError::NotFound("阶段不存在".to_string()))?
    {
        // 又见"盒子 vs 货物"：return AppError 少了 Err() 包装 → E0308。
        // 且语义错：归档禁编辑是"权限不足" = Forbidden(403)，不是 Other(500)。
        return Err(AppError::Forbidden("阶段已归档，不可编辑".to_string()));
    }

    let start_dt = if form.start_date.trim().is_empty()
    {
        None
    }
    else
    {
        Some(form.start_date.trim().to_string())
    };

    // 【教学：execute 返回 Future —— rows_affected 报错的根源】
    // 学生写：let ext_ret = sqlx::query(...).execute(&pool);
    //              ↑ 没有 .await！ext_ret 是 Future（"待办事项"），
    //              ↑ 不是 SqliteQueryResult。
    //         if ext_ret.rows_affected() == 0  → E0599！
    // rows_affected() 是 SqliteQueryResult 的方法（await 之后的结果类型）。
    // 与上次 map_err 同根：await 之前一切都是 Future，只有 await 一个方法。
    // 正确链：.execute(...).await.map_err(...)?  → 拿到 SqliteQueryResult
    //        → 再 rows_affected()。
    //
    // 另外学生还埋了 3 个编译抓不到的雷：
    //   ① SQL 里 "note = ," 少了 ?  —— SQL 是字符串，编译器不查，
    //      SQLite 解析时直接报错。占位符数量必须和 bind 数量一一对应。
    //   ② bind(form.start_date) 是原始字符串，不是转换好的 start_dt！
    //      上面转换了不用 = 白转换。
    //   ③ bind 了 5 个值，SQL 却只有 4 个 ?（还缺 note 的）→ 运行时报错。
    // 记住：SQL 的 ? 数 = bind 数 = 一一按序对应。
    let ext_ret = sqlx::query(
        "UPDATE phases SET name = ?, note = ?, start_date = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&form.name)
    .bind(&form.note)
    .bind(&start_dt)
    .bind(phase_id)
    .bind(user.id)
    .execute(&pool)
    .await
    .map_err(AppError::Database)?;

    // rows_affected() == 0 → id 不存在或不是自己的 → 404
    if ext_ret.rows_affected() == 0
    {
        return Err(AppError::NotFound("阶段不存在".to_string()));
    }
    // 更新成功 → 回列表页（PRG 模式）
    Ok(Redirect::to("/phases"))
}

// ============================================================
// 归档 / 重新启用（POST /phases/{id}/archive 与 /unarchive）
// ============================================================
/// 【教学：参数化 —— 用枚举表达"动作"（学生实现实录）】
/// 学生思路：一个 set_archive 函数 + action_t 枚举吞并 archive/unarchive。
/// 方向正确（消灭重复），但实现有三个问题：
///   ① handler 参数必须是 axum 能注入的提取器（State/AuthUser/Path/Form），
///      它们实现 FromRequestParts / FromRequest，axum 才能自动装配。
///      自定义枚举没实现这俩特型 → axum 注入不了。
///      业务参数必须由"调用方"传，不能出现在 handler 签名里。
///   ② 路由是两个 URL（/archive 与 /unarchive），必须有两个 handler
///      各自调用公共实现——action 参数恰恰不能帮 axum 区分路由。
///   ③ 命名：Rust 惯例 PascalCase（ActionType::Archive），
///      C 风格的 _t 后缀 + 全大写是 C 语言习惯，Rust 不用。
///
/// 正确方案：内部函数（非 handler）+ 两个薄 handler
///   - set_archived：真正干活，动作由参数传入
///   - archive / unarchive：薄包装，各传各的动作
/// 这样"变化的部分"（动作）由调用方决定，axum 只注入请求里的东西。
enum ActionType
{
    Archive,
    Unarchive,
}

/// 内部辅助函数：设置归档状态（Archive → archived=1，Unarchive → archived=0）
///
/// 【教学：为什么这个函数不是 handler？】
/// 它不是 pub、没有 State/AuthUser/Path 提取器参数、不被 axum 调用——
/// 它只是 archive/unarchive 两个 handler 的公共实现。
/// 参数最小化：只需要 pool/user_id/id/action，不给多余的东西
/// （不需要整个 AppState，只需要数据库连接）。
async fn set_archived(
    pool: &SqlitePool,
    user_id: i64,
    phase_id: i64,
    action: ActionType,
) -> Result<(), AppError>
{
    let archived: bool = match action
    {
        ActionType::Archive => true,
        ActionType::Unarchive => false,
    };
    let result = sqlx::query("UPDATE phases SET archived = ? WHERE id = ? AND user_id = ?")
        .bind(archived) // bool → SQLite 自动映射 1/0
        .bind(phase_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    if result.rows_affected() == 0
    {
        return Err(AppError::NotFound("阶段不存在".to_string()));
    }
    Ok(())
}

/// 归档阶段（archived 0 → 1，变为只读）
///
/// 【教学：为什么归档不做成 DELETE？】
/// 阶段承载着训练历史（M3 计划、M4 记录都挂在 phase_id 上）。
/// 删掉阶段 = 删掉所有历史！所以"不再使用"的阶段不能删，只能归档：
///   - 列表里不再显示在"进行中"区（移到"已归档"区）
///   - 归档阶段禁止再建计划/记录（M3/M4 的守卫会查 archived）
///   - 数据还在，随时可以 unarchive 恢复
/// 这是"软删除"思想：用状态标记代替物理删除，保护历史数据。
///
/// 【教学：归档 = UPDATE 一个字段】
/// 归档不是特殊操作，就是一次 UPDATE：
///   UPDATE phases SET archived = 1 WHERE id = ? AND user_id = ?
/// unarchive 是同一句 SQL，只把 1 改成 0。
/// 所以 archive 和 unarchive 结构几乎一样，区别只是一个数字。
///
/// 【教学：重复归档/重复启用 —— 幂等性】
/// 用户连点两次归档按钮：第一次成功（0→1），第二次呢？
///   UPDATE ... SET archived = 1 WHERE archived = 0   → 第二次 0 行受影响
/// 如果只按 id 更新，第二次也"成功"（1→1，无意义但无害）。
/// 本项目简单处理：不查当前状态，直接 UPDATE，结果都是重定向回列表页。
/// 这种"重复操作结果一样"的性质叫**幂等**，是良好 API 设计的一部分。
///
/// 【实现步骤】（archive 与 unarchive 相同，仅数字不同）
/// 1. 签名：State + AuthUser + Path(phase_id)
/// 2. 调 set_archived(..., ActionType::Archive)
/// 3. Ok(Redirect::to("/phases"))
pub async fn archive(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Redirect, AppError>
{
    let pool = state.pool.read().await.clone();

    set_archived(&pool, user.id, phase_id, ActionType::Archive).await?;
    Ok(Redirect::to("/phases"))
}

/// 重新启用阶段（archived 1 → 0）
///
/// 与 archive 完全对称：传 ActionType::Unarchive。
/// 【实现步骤】
/// 1. 同 archive，动作改成 Unarchive
/// 2. Ok(Redirect::to("/phases"))
pub async fn unarchive(
    State(state): State<AppState>,
    AuthUser(user): AuthUser,
    Path(phase_id): Path<i64>,
) -> Result<Redirect, AppError>
{
    let pool = state.pool.read().await.clone();

    set_archived(&pool, user.id, phase_id, ActionType::Unarchive).await?;
    Ok(Redirect::to("/phases"))
}
