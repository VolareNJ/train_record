// ============================================================
// api/grpc/server.rs —— 组装并启动 gRPC 服务器（M9 第 7 步）
// ============================================================
// 【教学：tonic 的服务器只有三步】
//   ① Server::builder()                      创建服务器构建器
//   ② .add_service(XxxServer::new(impl))     注册每个 service 的实现
//   ③ .serve(addr).await                     开始监听（永不返回，直到进程结束）
// 与 axum 的对照：
//   axum:  Router::new().route(...)      .merge(...)   .with_state(state)  → axum::serve(listener, app)
//   tonic: Server::builder().add_service(...).add_service(...)             → .serve(addr)
// 两个框架都是"注册 → serve"的骨架，只是注册的东西不同
// （axum 注册"路径 → 函数"，tonic 注册"service 实现"）。
//
// 【教学：为什么要给每个 service 塞一份 AppState？】
// tonic 的服务实现在**每个连接**上会被 clone（所以 impl 必须 Clone）。
// AppState 内部是 Arc（连接池 + 配置），clone 只复制指针 → 廉价且共享同一份数据。
// 与 axum 的 with_state 完全同理（M0 教学里那句"公共储物柜"）。
//
// 【教学：两个服务器怎么共存？】
//   HTTP（axum，端口 PORT 默认 8080）  ← 页面 + REST
//   gRPC（tonic，端口 GRPC_PORT 默认 50051）← 本文件
// 两个独立监听端口，各自跑在同一个 tokio 运行时里（见 main.rs 的 tokio::spawn）。
// 为什么不用一个端口？因为 tonic 只讲 HTTP/2 + protobuf，
// axum 讲 HTTP/1.1 + HTML/JSON；虽然理论上有"多路复用同一端口"的方案
// （tonic-web / 手动 multiplex），但对本项目属于过度设计：
// 换端口的成本是"部署时多开一个端口"，换来的清晰度是"两套协议互不干扰"。
//
// 【教学：日志为什么写在这里？】
// 服务器起不来最常见的原因是端口被占。tracing::info! 打一行"我监听在哪"，
// 排查时一看日志就知道（比"curl 没反应"猜半天强）。
// ============================================================

/// 启动 gRPC 服务器（在 main.rs 里被 tokio::spawn 起来，与 axum 并行）
///
/// 【教学：返回 Box<dyn Error> 合适吗？】
/// 这里的错误只会在"启动"路径上出现（端口占用、地址非法），
/// 处理方式就是记一条 error 日志让运维看见——不需要调用方按类型分派，
/// 所以用 `Box<dyn Error>` 装箱足够（这也是应用层 main/启动函数的常规写法；
/// 库代码才需要精确的错误类型——项目纪律"避免 dyn"针对的是**热路径**，
/// 启动路径一次性装箱没有性能影响）。
pub async fn serve(
    state: crate::AppState,
    addr: std::net::SocketAddr,
) -> Result<(), Box<dyn std::error::Error>>
{
    tracing::info!(
        "gRPC 服务监听 {addr}（auth/phases/exercises/plans/records/stats 共 6 个 service）"
    );

    // 【教学：add_service 的顺序无关紧要】
    // tonic 内部按 proto 里的"服务全名"（train_record.v1.PhaseService）路由，
    // 不是按注册顺序匹配。所以下面这串纯粹是"把 6 张名片递上去"。
    tonic::transport::Server::builder()
        .add_service(
            crate::api::grpc::pb::auth_service_server::AuthServiceServer::new(
                crate::api::grpc::service::auth::AuthServiceImpl::new(state.clone()),
            ),
        )
        .add_service(
            crate::api::grpc::pb::phase_service_server::PhaseServiceServer::new(
                crate::api::grpc::service::phases::PhaseServiceImpl::new(state.clone()),
            ),
        )
        .add_service(
            crate::api::grpc::pb::exercise_service_server::ExerciseServiceServer::new(
                crate::api::grpc::service::exercises::ExerciseServiceImpl::new(state.clone()),
            ),
        )
        .add_service(
            crate::api::grpc::pb::plan_service_server::PlanServiceServer::new(
                crate::api::grpc::service::plans::PlanServiceImpl::new(state.clone()),
            ),
        )
        .add_service(
            crate::api::grpc::pb::record_service_server::RecordServiceServer::new(
                crate::api::grpc::service::records::RecordServiceImpl::new(state.clone()),
            ),
        )
        .add_service(
            crate::api::grpc::pb::stats_service_server::StatsServiceServer::new(
                crate::api::grpc::service::stats::StatsServiceImpl::new(state.clone()),
            ),
        )
        // 【教学：serve 会一直 await 到进程结束】
        // 它内部是个 loop { accept; spawn(处理连接) }。
        // 所以调用方要么 spawn 它（本项目：main.rs），要么把它作为 main 的最后一行。
        .serve(addr)
        .await?;

    Ok(())
}
