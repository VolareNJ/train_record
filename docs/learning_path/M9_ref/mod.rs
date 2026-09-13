// ============================================================
// api/grpc/mod.rs —— gRPC 出口（M9）总入口
// ============================================================
// 【教学说明：gRPC 出口在整体架构里的位置】
//
//        ┌──────────────┐
//  SSR ──┤ handlers/    │  HTML      （手机浏览器，人看）
//        │              │
//  REST──┤ api/rest/    │  JSON      （curl / 脚本 / 任意 HTTP 客户端）
//        │              │
//  gRPC──┤ api/grpc/    │  Protobuf  （iced 桌面客户端 M10）
//        └──────┬───────┘
//               │
//          SQLite（train_record.db）
//
// **三个出口共享同一份数据库和同一份业务逻辑**：本目录不写业务 SQL，
// 只做"协议适配"——把 gRPC 请求翻译成对 `api::rest::*` 里 pub(crate)
// 查询函数的调用（M8 §2.5 说的"先复制，后抽取"在这里兑现），
// 再把结果翻译成 proto 消息返回。
//
// 【教学：一次 gRPC 调用的完整旅程】
//   iced 客户端                       服务器
//   ───────────────────────────────────────────────────────────
//   PhaseServiceClient::list_phases()
//        │  ① 序列化请求消息（protobuf 二进制）
//        │  ② HTTP/2 POST /train_record.v1.PhaseService/ListPhases
//        │     （内容类型 application/grpc；token 放 metadata）
//        ├───────────────────────────────► tonic 解析 → 找到生成的 Server
//                                         ③ 我们的 `PhaseService::list_phases` 被调用
//                                         ④ require_user() 从 metadata 取 token 验身份
//                                         ⑤ 调 rest::phases::phase_list()（共用 SQL）
//                                         ⑥ DTO → proto 消息（convert.rs）
//                                         ⑦ 打包 Response + 状态码 OK
//        ◄───────────────────────────────┤
//  ⑧ 反序列化得到 ListPhasesResponse（强类型 struct，不用手写 JSON 解析）
//
//   任何一步出错 → 返回 gRPC 状态码（如 UNAUTHENTICATED），
//   客户端拿到 `Status`（不是 HTTP 302/401 JSON，见 error.rs）。
//
// 【教学：目录结构（每个文件一个职责）】
//   mod.rs      本文件：include_proto!（生成代码）+ 模块声明 + 全局说明
//   error.rs    错误映射：ApiError → tonic::Status（对照 REST 的 HTTP 状态码）
//   auth.rs     认证：从 metadata 取 token → 查 session → User（"gRPC 版守卫"）
//   convert.rs  DTO → proto 消息 的转换（REST 读模型 → 线格式契约）
//   service/    6 个 service 的实现（每个 RPC 一个方法，业务在这里"翻译"）
//   server.rs   组装 tonic Server（端口/服务注册）——由 main.rs 调用
//
// 阶段要求（M9）：
//   本目录的**框架与定义**已由老师写好，若干核心函数挖空（`todo!(...)`）
//   留给你实现——每个挖空处上方都有【实现步骤】。
//   实现顺序见 docs/learning_path/M9.md §3；完整实现备份在
//   docs/learning_path/M9_ref/（做完再看，别提前翻）。
//
// 挖空期间会有 `unused` / `dead_code` 警告（函数还没被调用）：
//   实现完成后应全部消失（M7 的纪律：不留警告）。
// ============================================================

// ============================================================
// 【教学：tonic::include_proto! —— 把生成的代码"粘"进来】
// ============================================================
// build.rs 里让 protoc 生成 Rust 代码，产物在 OUT_DIR/$crate-$hash/out/。
// `include_proto!("train_record.v1")` 做的事：
//   1. 按 package 名找到生成文件（train_record.v1.rs）
//   2. 把它的内容原样 include! 进当前模块
// 生成的东西（本项目的 6 个 service × 每个 RPC）：
//   pub struct Phase { ... }          ← message → struct
//   pub struct ListPhasesRequest { }  ← 请求消息
//   pub trait PhaseService            ← 我们要实现的 trait（方法 = RPC）
//   pub struct PhaseServiceServer<T>  ← 包装实现、挂进 tonic 路由器
//   pub struct PhaseServiceClient<T>  ← 客户端（examples/grpc_smoke.rs 用）
//
// 【教学：为什么包一层 `pub mod pb`？】
//   ① 生成类型统一走 `pb::` 前缀，和手写类型一眼区分
//   ② 生成代码里有本项目用不到的字段/客户端方法（比如服务器侧用不到 client），
//      会触发 dead_code 警告——用模块级 `#![allow(dead_code)]` 一次性关掉，
//      把"别人的代码"和"我们的代码"在警告层面隔离开
//      （ 不要对 service/ 手写代码关警告：那里是我们自己的责任）
pub mod pb
{
    #![allow(dead_code, clippy::all)]
    tonic::include_proto!("train_record.v1");
}

pub mod auth;
pub mod convert;
pub mod error;
pub mod server;
pub mod service;

// 【M9】gRPC 端到端冒烟测试（临时数据库 + 临时端口，见文件头说明）
#[cfg(test)]
mod smoke_test;

// ============================================================
// 【教学：本层的"契约边界"在哪？】
// ============================================================
// gRPC 层的每个函数只做三件事（service/*.rs 里你会反复看到这个模式）：
//   1. 取身份：`let user = auth::require_user(&request, &self.state).await?;`
//   2. 调共享业务：`phase_list(&pool, user.id).await.map_err(Status::from)?`
//   3. 转消息：`Ok(Response::new(pb::Phase::from(&out)))`
// 判断"这段代码该不该写在 gRPC 层"的口诀：
//   · 只有"协议相关"的东西（metadata/状态码/流）才属于这里
//   · 任何 SQL、任何校验规则 → 属于 rest 层（未来抽到 src/service/，见 todo.md）
// ============================================================
