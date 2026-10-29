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
// 服务器起不来最常见的原因是端口被占。tracing::info! 打一行“我监听在哪”，
// 排查时一看日志就知道（比“curl 没反应”猜半天强）。
//
// ============================================================
// 【教学：TLS 解决了什么？为什么公网必须开？】
// ============================================================
// 明文 gRPC 在公网上会漏两样东西：
//   1. metadata 里的 token（authorization: Bearer xxx）—— 中间人拿到就能冒充你
//   2. 请求/响应正文（训练数据；Login 请求里就是账号密码）
// TLS 做的事：握手时协商出只有两端知道的会话密钥，之后所有帧都加密；
//   并用**证书**证明“对端确实是我要访问的那台服务器”（防中间人顶替）。
//
// 【教学：证书从哪来？两条现实路线】
//   1. 公网 IP + 自签证书：用 openssl 自己签一张（命令见 docs/deploy.md）。
//      客户端要显式信任这张证书（tonic 客户端：ClientTlsConfig::ca_certificate）。
//      本项目当前没有域名 → 走这条。
//   2. 有域名：Let's Encrypt 免费签发（certbot / caddy 自动续期），
//      客户端信任系统根证书即可，无需手工配置。
// 注意：证书里必须有 SAN（Subject Alternative Name，写明 IP 或域名）——
//   现代 TLS 实现（rustls 也是）**只看 SAN 不看 CN**，只写 CN 的证书会被直接拒。
//
// 【教学：为什么做成“配置存在才启用”，而不是硬编码开/关？】
//   本地开发与冒烟测试走 127.0.0.1，明文最省事（不用先造证书）；
//   公网部署才要 TLS。配置驱动两边都兼顾，且启动日志会明说
//   当前是 TLS 还是明文——运维一眼能确认，不用猜。
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

    // 【M9 TLS】证书 + 私钥都配了 → 启用 TLS
    // （配置层已经保证“要么都给、要么都不给”，见 config.rs）
    //
    // 【教学：为什么读文件失败就直接返错、不静默降级成明文？】
    // 如果证书文件写错路径就默默变明文，运维会以为“已经加密了”——
    // 这比启动失败危险得多（安全性静默丢失，且没有任何报警）。
    // 所以启动阶段宁可失败：让 systemd 重启告警、日志里看到原因。
    //
    // 【教学：为什么用 tokio::fs 而不是 std::fs？】
    // 这里在 async 上下文里，std::fs 会阻塞 runtime 线程；
    // 启动阶段虽然只读一次，但保持“async 里不阻塞”的习惯更省心。
    let mut server = tonic::transport::Server::builder();
    match (&state.config.grpc_tls_cert, &state.config.grpc_tls_key)
    {
        (Some(cert_path), Some(key_path)) =>
        {
            let cert = tokio::fs::read(cert_path).await?;
            let key = tokio::fs::read(key_path).await?;
            tracing::info!("gRPC 已启用 TLS（证书 {cert_path}）");
            server = server.tls_config(
                tonic::transport::ServerTlsConfig::new()
                    .identity(tonic::transport::Identity::from_pem(cert, key)),
            )?;
        },
        _ =>
        {
            tracing::warn!(
                "gRPC 明文模式（未设置 GRPC_TLS_CERT / GRPC_TLS_KEY）——仅限本机/内网使用"
            );
        },
    }

    // 【教学：add_service 的顺序无关紧要】
    // tonic 内部按 proto 里的“服务全名”（train_record.v1.PhaseService）路由，
    // 不是按注册顺序匹配。所以下面这串纯粹是“把 6 张名片递上去”。
    server
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
