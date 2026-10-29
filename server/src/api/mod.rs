// ============================================================
// api/mod.rs —— 程序接口层总入口（M8 REST + M9 gRPC）
// ============================================================
// 【教学说明：为什么这里有两层目录？】
//
// 页面（handlers/）是"给人看的出口"，api/ 是"给程序看的出口"。
// 到 M9 为止，"给程序的出口"已经有**两种协议**：
//
//   src/api/rest/   REST（HTTP/1.1 + JSON）—— M8 做的，浏览器/脚本/iced 都能用
//   src/api/grpc/   gRPC（HTTP/2 + Protobuf）—— M9 做的，强类型 + 支持流式
//
// 同一份数据库、同一套业务逻辑，两个协议出口：
//
//           ┌──────────────┐
//   SSR ────┤ handlers/    │  HTML      （人看，手机浏览器）
//           │              │
//   REST ───┤ api/rest/    │  JSON      （curl / 脚本 / iced）
//           │              │
//   gRPC ───┤ api/grpc/    │  Protobuf  （iced 客户端 / 未来其他客户端）
//           └──────┬───────┘
//                  │
//               SQLite（train_record.db）
//
// 【教学：目录名对应"协议"，不是"业务"】
// rest/ 与 grpc/ 内部都按业务分文件（auth/phases/exercises/plans/records/stats），
// 但**业务规则只应有一份**：M8 的 REST 层里已经抽出了若干 pub(crate) 查询函数
// （phase_out / plan_out / exercise_out / today_out ...），
// M9 的 gRPC 层**直接调用它们**，不重写 SQL——这就是"先复制，后抽取"的兑现。
//
// 【本次（M9）改动：目录归并】
// M8 时代 api/mod.rs 里同时放着 ApiError 和路由组装，gRPC 加进来后再混在一起
// 就分不清"哪种协议的错误"了。所以把 M8 的代码整体挪进 api/rest/，
// api/mod.rs 只留下"模块树"这一件事：
//     api/mod.rs       ← 你在这里（只声明子模块）
//     api/rest/mod.rs  ← M8：ApiError（HTTP 状态码 + JSON）+ rest::router()
//     api/grpc/mod.rs  ← M9：Status（gRPC 状态码）+ 服务实现
//
//  阶段要求：
//   M8：REST 层实现在 api/rest/（已完成，验收通过）
//   M9：gRPC 层实现在 api/grpc/（本阶段，见 docs/learning_path/M9.md）
//
// ============================================================
// 【M8 → M9 的“兼容转发”演进（教学）】
// M8 阶段本文件是 `src/api/mod.rs`，M9 归并成 `api/rest/` 时加过一行
// `pub use rest::ApiError;` 做兼容转发（让 M8 的 7 个文件零改动）。
// M9 之后全项目统一“全路径”约定（见 AGENTS.md），转发已无必要：
//   调用方直接写 `crate::api::rest::ApiError`，一条路径一个出处。
// 结论：能靠“路径”表达的关系，不靠“导入”——少一层间接就少一处要同步的地方。
// ============================================================
pub mod grpc;
pub mod rest;
