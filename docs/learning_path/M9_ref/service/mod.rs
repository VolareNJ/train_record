// ============================================================
// api/grpc/service/mod.rs —— 6 个 service 的实现（M9 第 6 步）
// ============================================================
// 【教学：gRPC 服务端代码长什么样？】
// proto 里每写一个 `service XxxService { rpc A(...) ... }`，
// protoc 就生成一个 **trait**（`pb::xxx_service_server::XxxService`），
// 我们只要实现这个 trait，再交给 tonic 挂到服务器上（见 server.rs）。
// 也就是说：**实现 RPC = 给 trait 写方法**，和实现普通 Rust trait 没有区别。
//
// 【教学：每个 RPC 方法的固定三步（本目录 30 个方法全是这个骨架）】
//   async fn list_phases(&self, request: Request<...>) -> Result<Response<...>, Status>
//   {
//       // ① 取身份（需要登录的接口才有这一步）
//       let user = auth::require_user(&request, &self.state).await?;
//       // ② 调共享业务逻辑（REST 层抽出来的 pub(crate) 函数，同一份 SQL）
//       let pool = pool_of(&self.state).await;
//       let out = rest_phases::phase_list(&pool, user.id).await?;   // ← ? 自动把 ApiError 变 Status
//       // ③ 转消息 + 包 Response
//       Ok(Response::new(pb::ListPhasesResponse { phases: ... }))
//   }
// 判断"这行代码该不该写在这里"的标准：
//   · 协议相关（metadata / Status / 流）= 属于这里
//   · 业务规则与 SQL = 属于 api/rest（未来抽到 src/service，见 todo.md）
//
// 【教学：为什么每个 service 一个文件，而不是一个大文件？】
// 与 REST 层同一套心思：按业务分（auth/phases/exercises/plans/records/stats），
// 定位问题只需记住"这是哪个业务"，不用在 3000 行里翻。
//
// 【教学：为什么 impl 结构体要 `#[derive(Clone)]`？】
// tonic 内部对每个连接 clone 一份 service（Server<T> 要求 T: Clone），
// 它只是复制 state 的 Arc 指针，不会复制连接池/数据库。
// 与 axum 的 with_state 是同一个道理（M0 的 AppState 教学）。
//
// 📌 本目录挖空清单（都要你实现，共 8 处，每处上方有【实现步骤】）：
//   auth.rs      login                        （一元 + 建会话）
//   phases.rs    set_phase_archived           （一元 + 复用 REST 的归档逻辑）
//   exercises.rs stream_exercise_series       （★ 服务器流式）
//   plans.rs     create_plan                  （一元 + 嵌套 repeated 写入）
//   records.rs   get_today / upsert_record    （一元：读今日 + 写记录）
//   records.rs   submit_workout               （★ 客户端流式）
//   stats.rs     stream_records               （★ 服务器流式 + 区间过滤）
// （另有 4 处基础件挖空：error.rs 的映射、auth.rs 的守卫、convert.rs 的两个转换）
// ============================================================

pub mod auth;
pub mod exercises;
pub mod phases;
pub mod plans;
pub mod records;
pub mod stats;

use crate::AppState;
use sqlx::SqlitePool;

/// 取连接池快照
///
/// 【教学：为什么要"读锁 + clone"这一套？】
/// AppState.pool 是 `Arc<RwLock<SqlitePool>>`（M7 为了"上传备份后免重启热替换"）：
///   · read()   拿读锁（很多请求可以同时读）
///   · .clone() SqlitePool 内部是 Arc，clone 只是多一个指针，
///             关键是**锁不跨 await 持有**——克隆完立刻释放锁，
///             再拿这个克隆去做耗时的查询。
/// （如果直接持锁去查询，一旦有写操作排队，就会拖慢整个服务器。）
pub(crate) async fn pool_of(state: &AppState) -> SqlitePool
{
    state.pool.read().await.clone()
}
