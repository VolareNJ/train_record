// ============================================================
// api/grpc/smoke_test.rs —— gRPC 端到端冒烟测试（M9 验收用）
// ============================================================
// 【教学：这个文件有两个身份】
//   ① 测试：`cargo test grpc_smoke -- --nocapture`
//      它真的把 tonic 服务器跑起来（临时数据库 + 临时端口），再用生成的
//      **客户端**把 6 个 service 各调一遍，验证"契约 → 实现 → 数据库"整条链通。
//      这比"编译通过"强得多：编译只能证明类型对，跑起来才能证明语义对。
//   ② 客户端范例：M10 的 iced 客户端要写的代码，骨架和这里一模一样：
//      建连接 → 登录拿 token → 每次调用带 metadata → 收流式响应。
//      提前在一个文件里看到"客户端长什么样"，比等到 M10 再摸索友好得多。
//
// 【教学：为什么用内存/临时数据库？】
// 测试绝不能碰生产数据（train_record.db 里是真训练记录）。
// 这里用 /tmp 下的随机文件名：每个测试进程一份干净库，跑完即弃。
// 建库 + 迁移直接复用 db::init_pool（与生产同一条路径）——
// 这样测试才真的能发现"迁移写错了/表结构与模型不一致"这类问题。
//
// 【教学：为什么测试里还要手写 INSERT users？】
// 因为本项目没有"注册 API"（用户由管理员建，见 ensure_admin）。
// 测试要先造一个能登录的用户：hash_password（真实哈希）+ 一条 INSERT。
// 其余数据全部走 gRPC 接口创建 —— 这正好顺带验证了写路径。
// ============================================================

#![cfg(test)]

/// 造一份"干净的测试环境"：临时库 + 一个 admin 用户 + 随机端口的服务器
async fn spawn_test_server() -> (String, sqlx::SqlitePool)
{
    // 1. 临时数据库（uuid 命名，避免并发/残留干扰）
    let db_path = format!("/tmp/m9_smoke_{}.db", uuid::Uuid::new_v4());
    let config = crate::config::AppConfig {
        port: 0,
        grpc_port: 0,
        database_path: db_path,
        session_secret: "test-secret".to_string(),
        admin_username: String::new(),
        admin_password: String::new(),
        body_part_order: vec!["腿".to_string(), "背".to_string(), "胸".to_string()],
    };

    let pool = crate::db::init_pool(&config)
        .await
        .expect("测试数据库初始化失败");

    // 2. 造一个可登录的用户（admin / admin123）
    let hash = crate::auth::hash_password("admin123").expect("密码哈希失败");
    sqlx::query(
        "INSERT INTO users (username, password_hash, is_admin, body_weight)
        VALUES (?, ?, 1, 70.0)",
    )
    .bind("admin")
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("插入测试用户失败");

    // 3. 找一个空闲端口（先绑再放开：极小概率被别人抢，测试环境可接受）
    let probe = std::net::TcpListener::bind("127.0.0.1:0").expect("找空闲端口失败");
    let addr = probe.local_addr().expect("读端口失败");
    drop(probe);

    // 4. 起服务器（与 main.rs 同一条路径：server::serve）
    let state = crate::AppState {
        pool: std::sync::Arc::new(tokio::sync::RwLock::new(pool.clone())),
        config,
    };
    tokio::spawn(async move {
        if let Err(e) = super::server::serve(state, addr).await
        {
            eprintln!("测试服务器退出: {e}");
        }
    });

    // 5. 等服务器就绪（重试连接，最多 ~2 秒）
    let url = format!("http://{addr}");
    for _ in 0..20
    {
        if crate::api::grpc::pb::auth_service_client::AuthServiceClient::connect(url.clone())
            .await
            .is_ok()
        {
            return (url, pool);
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("测试服务器启动超时");
}

/// 给请求带上 token（M10 客户端每个调用都要做这一步）
fn authed<T>(msg: T, token: &str) -> tonic::Request<T>
{
    let mut req = tonic::Request::new(msg);
    req.metadata_mut().insert(
        "authorization",
        format!("Bearer {token}").parse().expect("metadata 非法"),
    );
    req
}

#[tokio::test]
//  M9 练习期间默认跳过（12 处挖空未实现时必然 panic）。
// 实现完全部挖空后，用下面这条命令把它跑起来（这是 M9 最重要的一步验收）：
//     cargo test grpc_smoke -- --ignored --nocapture
// 跟通过后，可以把下面这行 ignore 属性删掉，让 cargo test 默认执行它。
#[ignore = "M9 挖空未实现完：全实现后用 -- --ignored 运行"]
async fn grpc_smoke()
{
    let (url, _pool) = spawn_test_server().await;

    // ============================================================
    // 0. 未登录必须拿到 UNAUTHENTICATED（不是 302、不是 OK）
    // ============================================================
    let mut auth_client =
        crate::api::grpc::pb::auth_service_client::AuthServiceClient::connect(url.clone())
            .await
            .expect("连接失败");
    let err = auth_client
        .get_me(crate::api::grpc::pb::GetMeRequest {})
        .await
        .expect_err("未登录居然成功了？");
    assert_eq!(err.code(), tonic::Code::Unauthenticated, "状态码应为 16");

    // ============================================================
    // 1. 登录 → 拿 token → GetMe（一元 RPC + 认证链路）
    // ============================================================
    let login = auth_client
        .login(authed(
            crate::api::grpc::pb::LoginRequest {
                username: "admin".to_string(),
                password: "admin123".to_string(),
            },
            "",
        ))
        .await
        .expect("登录失败")
        .into_inner();
    let token = login.token.clone();
    assert!(!token.is_empty(), "login 必须返回 token");
    assert_eq!(
        login.user.as_ref().map(|u| u.username.as_str()),
        Some("admin")
    );

    let me = auth_client
        .get_me(authed(crate::api::grpc::pb::GetMeRequest {}, &token))
        .await
        .expect("GetMe 失败")
        .into_inner();
    assert_eq!(me.username, "admin");
    assert!(me.body_weight.is_some(), "体重应随 User 一起返回");

    // 密码错 → UNAUTHENTICATED（且提示与"用户不存在"一致，防用户名枚举）
    let bad = auth_client
        .login(crate::api::grpc::pb::LoginRequest {
            username: "admin".to_string(),
            password: "wrong".to_string(),
        })
        .await
        .expect_err("错密码居然登录成功？");
    assert_eq!(bad.code(), tonic::Code::Unauthenticated);

    // ============================================================
    // 2. 阶段：创建 → 列表 → 详情 → 归档（一元 RPC + 派生字段 days）
    // ============================================================
    let mut phase_client =
        crate::api::grpc::pb::phase_service_client::PhaseServiceClient::connect(url.clone())
            .await
            .expect("连接失败");

    let phase = phase_client
        .create_phase(authed(
            crate::api::grpc::pb::CreatePhaseRequest {
                name: "M9 测试阶段".to_string(),
                note: "冒烟".to_string(),
                start_date: Some("2026-08-01".to_string()),
            },
            &token,
        ))
        .await
        .expect("创建阶段失败")
        .into_inner();
    assert!(phase.id > 0);
    assert!(phase.days >= 0, "days 是服务器算好的派生字段");

    let phases = phase_client
        .list_phases(authed(crate::api::grpc::pb::ListPhasesRequest {}, &token))
        .await
        .expect("阶段列表失败")
        .into_inner();
    assert_eq!(phases.phases.len(), 1);

    // PATCH 语义：只传 note，name 必须保持原值（optional 的意义）
    let patched = phase_client
        .update_phase(authed(
            crate::api::grpc::pb::UpdatePhaseRequest {
                id: phase.id,
                name: None,
                note: Some("改过备注".to_string()),
                start_date: None,
            },
            &token,
        ))
        .await
        .expect("更新阶段失败")
        .into_inner();
    assert_eq!(patched.name, "M9 测试阶段", "没传的字段不能被清空");
    assert_eq!(patched.note, "改过备注");

    // ============================================================
    // 3. 动作：创建 → 列表（按部位筛选）→ 详情
    // ============================================================
    let mut ex_client =
        crate::api::grpc::pb::exercise_service_client::ExerciseServiceClient::connect(url.clone())
            .await
            .expect("连接失败");

    let ex = ex_client
        .create_exercise(authed(
            crate::api::grpc::pb::CreateExerciseRequest {
                name: "深蹲".to_string(),
                body_part: "腿".to_string(),
                // 全部不传 → 服务器默认值（复用了 REST 层的 default_* 函数）
                default_mode: None,
                bar_weight: None,
                default_unit: None,
                default_sets: None,
                default_reps: None,
                key_points: None,
            },
            &token,
        ))
        .await
        .expect("创建动作失败")
        .into_inner();
    assert_eq!(
        ex.default_mode, "bar",
        "默认值应来自 REST 层的 default_mode()"
    );
    assert_eq!(ex.bar_weight, 20.0);
    assert_eq!(ex.default_sets, 3);
    assert_eq!(ex.best_1rm, None, "还没有记录 → 不传 best_1rm");

    let filtered = ex_client
        .list_exercises(authed(
            crate::api::grpc::pb::ListExercisesRequest {
                body_part: Some("腿".to_string()),
            },
            &token,
        ))
        .await
        .expect("动作列表失败")
        .into_inner();
    assert_eq!(filtered.exercises.len(), 1);

    let none_found = ex_client
        .list_exercises(authed(
            crate::api::grpc::pb::ListExercisesRequest {
                body_part: Some("胸".to_string()),
            },
            &token,
        ))
        .await
        .expect("动作列表失败")
        .into_inner();
    assert!(none_found.exercises.is_empty(), "按部位筛选应生效");

    // ============================================================
    // 4. 计划：创建（嵌套 items）→ 详情 → 列表
    // ============================================================
    let mut plan_client =
        crate::api::grpc::pb::plan_service_client::PlanServiceClient::connect(url.clone())
            .await
            .expect("连接失败");

    let today = sqlx::query_scalar::<_, String>("SELECT date('now', 'localtime')")
        .fetch_one(&_pool)
        .await
        .expect("取今天失败");

    let plan = plan_client
        .create_plan(authed(
            crate::api::grpc::pb::CreatePlanRequest {
                phase_id: phase.id,
                date: today.clone(),
                note: "腿日".to_string(),
                items: vec![
                    crate::api::grpc::pb::PlanItemInput {
                        exercise_id: ex.id,
                        plan_sets: Some(5),
                        plan_reps: Some(5),
                        plan_weight: Some(60.0),
                        plan_rest: Some(180),
                        plan_key_points: Some("核心收紧".to_string()),
                        plan_note: None,
                    },
                    crate::api::grpc::pb::PlanItemInput {
                        exercise_id: ex.id,
                        plan_sets: Some(3),
                        plan_reps: Some(10),
                        plan_weight: None,
                        plan_rest: None,
                        plan_key_points: None,
                        plan_note: Some("收尾组".to_string()),
                    },
                ],
            },
            &token,
        ))
        .await
        .expect("创建计划失败")
        .into_inner();
    assert_eq!(plan.items.len(), 2);
    // 【教学：repeated 的顺序 = 数组顺序（对比 REST 表单多选的 §2.1 坑）】
    assert_eq!(plan.items[0].plan_sets, Some(5));
    assert_eq!(plan.items[1].plan_sets, Some(3));
    assert_eq!(plan.items[0].exercise_name, "深蹲", "服务器 JOIN 出动作名");

    // ============================================================
    // 5. 今日卡片（GetToday：三层嵌套转换 + Option 空态）
    // ============================================================
    let mut rec_client =
        crate::api::grpc::pb::record_service_client::RecordServiceClient::connect(url.clone())
            .await
            .expect("连接失败");

    let today_view = rec_client
        .get_today(authed(crate::api::grpc::pb::GetTodayRequest {}, &token))
        .await
        .expect("GetToday 失败")
        .into_inner();
    assert_eq!(today_view.date, today);
    let view_phase = today_view.phase.as_ref().expect("应有进行中阶段");
    assert_eq!(view_phase.name, "M9 测试阶段");
    let view_plan = today_view.plan.as_ref().expect("应有今日计划");
    assert_eq!(view_plan.items.len(), 2);
    assert!(
        view_plan.items[0].last_record.is_none(),
        "还没练过 → last_record 不传"
    );

    // ============================================================
    // 6. 记录 upsert（一元写 + 复用 REST 的 upsert 语义）
    // ============================================================
    let item_id = view_plan.items[0].id;
    let saved = rec_client
        .upsert_record(authed(
            crate::api::grpc::pb::UpsertRecordRequest {
                plan_id: plan.id,
                plan_item_id: item_id,
                weight: 62.5,
                sets: 5,
                reps: 5,
                rest: 180,
                feeling: "轻松".to_string(),
                strategy: "下次加 2.5".to_string(),
                key_points: "腰别塌".to_string(),
                completed: true,
            },
            &token,
        ))
        .await
        .expect("记录失败")
        .into_inner();
    assert!(saved.id > 0);
    assert!(saved.one_rm > 62.5, "1RM 是实时算出来的（Epley）");
    assert!(saved.completed);

    // 再 upsert 一次同一条 → 应该是 UPDATE（id 不变），不是新增
    let again = rec_client
        .upsert_record(authed(
            crate::api::grpc::pb::UpsertRecordRequest {
                plan_id: plan.id,
                plan_item_id: item_id,
                weight: 65.0,
                sets: 5,
                reps: 5,
                rest: 180,
                feeling: String::new(),
                strategy: String::new(),
                key_points: String::new(),
                completed: true,
            },
            &token,
        ))
        .await
        .expect("二次记录失败")
        .into_inner();
    assert_eq!(again.id, saved.id, "同一天同动作应更新而非新增");

    // 当天记录（含部位 → 转换来源是 RecordRow）
    let day = rec_client
        .get_day_records(authed(
            crate::api::grpc::pb::GetDayRecordsRequest {
                date: today.clone(),
            },
            &token,
        ))
        .await
        .expect("当天记录失败")
        .into_inner();
    assert_eq!(day.records.len(), 1);
    assert_eq!(day.records[0].body_part, "腿", "RecordRow 来源应带部位");
    assert_eq!(day.records[0].weight, 65.0);

    // ============================================================
    // 7. 服务器流：动作 1RM 序列
    // ============================================================
    let mut series = ex_client
        .stream_exercise_series(authed(
            crate::api::grpc::pb::ExerciseSeriesRequest {
                exercise_id: ex.id,
                from_date: None,
                to_date: None,
            },
            &token,
        ))
        .await
        .expect("订阅序列失败")
        .into_inner();
    let mut points = Vec::new();
    while let Some(point) = series.message().await.expect("读流失败")
    {
        points.push(point);
    }
    assert_eq!(points.len(), 1, "目前只有一条训练记录");
    assert_eq!(points[0].exercise_id, ex.id);
    assert!(points[0].one_rm > 0.0);

    // 区间过滤（把这次记录排除掉）
    let mut series_empty = ex_client
        .stream_exercise_series(authed(
            crate::api::grpc::pb::ExerciseSeriesRequest {
                exercise_id: ex.id,
                from_date: Some("2000-01-01".to_string()),
                to_date: Some("2000-12-31".to_string()),
            },
            &token,
        ))
        .await
        .expect("订阅序列失败")
        .into_inner();
    assert!(
        series_empty.message().await.expect("读流失败").is_none(),
        "区间外应没有数据"
    );

    // ============================================================
    // 8. 客户端流：批量提交（训练结束一次性上传）
    // ============================================================
    let second_item = view_plan.items[1].id;
    let batch = vec![
        crate::api::grpc::pb::UpsertRecordRequest {
            plan_id: plan.id,
            plan_item_id: item_id,
            weight: 70.0,
            sets: 3,
            reps: 5,
            rest: 180,
            feeling: String::new(),
            strategy: String::new(),
            key_points: String::new(),
            completed: true,
        },
        crate::api::grpc::pb::UpsertRecordRequest {
            plan_id: plan.id,
            plan_item_id: second_item,
            weight: 40.0,
            sets: 3,
            reps: 10,
            rest: 90,
            feeling: String::new(),
            strategy: String::new(),
            key_points: String::new(),
            completed: true,
        },
    ];
    let summary = rec_client
        .submit_workout(authed(tokio_stream::iter(batch), &token))
        .await
        .expect("批量提交失败")
        .into_inner();
    assert_eq!(summary.saved_count, 2);
    assert!(summary.total_volume > 0.0, "总容量应被累计");
    assert!(
        summary.best_1rm.unwrap_or(0.0) > 80.0,
        "70kg*5 → 1RM 约 81.7"
    );

    // ============================================================
    // 9. 双向流：一组一上报、一组一回执
    // ============================================================
    let (tx, rx) = tokio::sync::mpsc::channel(4);
    tokio::spawn(async move {
        for weight in [80.0f64, 85.0]
        {
            let item = crate::api::grpc::pb::UpsertRecordRequest {
                plan_id: plan.id,
                plan_item_id: item_id,
                weight,
                sets: 1,
                reps: 5,
                rest: 120,
                feeling: String::new(),
                strategy: String::new(),
                key_points: String::new(),
                completed: true,
            };
            if tx.send(item).await.is_err()
            {
                return;
            }
        }
        // 发完让 tx 落地（drop）→ 服务器看到 half-close → 结束响应流
    });
    let mut live = rec_client
        .live_session(authed(
            tokio_stream::wrappers::ReceiverStream::new(rx),
            &token,
        ))
        .await
        .expect("建立双向流失败")
        .into_inner();

    let mut feedbacks = Vec::new();
    while let Some(fb) = live.message().await.expect("读回执失败")
    {
        feedbacks.push(fb);
    }
    assert_eq!(feedbacks.len(), 2, "两组应各回一条回执");
    assert_eq!(feedbacks[0].completed_count, 1);
    assert_eq!(feedbacks[1].completed_count, 2);
    assert!(feedbacks[1].new_pr, "85kg 是本动作的新纪录");

    // ============================================================
    // 10. 统计：日历 + 流式导出
    // ============================================================
    let mut stats_client =
        crate::api::grpc::pb::stats_service_client::StatsServiceClient::connect(url.clone())
            .await
            .expect("连接失败");

    let calendar = stats_client
        .get_calendar(authed(
            crate::api::grpc::pb::GetCalendarRequest {
                year: None,
                month: None,
            },
            &token,
        ))
        .await
        .expect("日历失败")
        .into_inner();
    assert!(
        calendar.train_days.contains(&today),
        "今天有训练，日历里应该有它"
    );

    let mut export = stats_client
        .stream_records(authed(
            crate::api::grpc::pb::StreamRecordsRequest {
                from_date: None,
                to_date: None,
                exercise_id: Some(ex.id),
            },
            &token,
        ))
        .await
        .expect("导出失败")
        .into_inner();
    let mut exported = 0;
    while let Some(rec) = export.message().await.expect("读导出流失败")
    {
        assert_eq!(rec.exercise_id, ex.id);
        exported += 1;
    }
    assert!(exported >= 1, "导出应至少有一条记录");

    // ============================================================
    // 11. 数据隔离：另一个用户看不到这些数据（NOT_FOUND 而不是返回别人的）
    // ============================================================
    let hash = crate::auth::hash_password("other123").expect("哈希失败");
    let other_id: i64 = sqlx::query_scalar(
        "INSERT INTO users (username, password_hash, is_admin) VALUES ('other', ?, 0) RETURNING id",
    )
    .bind(&hash)
    .fetch_one(&_pool)
    .await
    .expect("插入第二个用户失败");
    let other_token = crate::auth::create_session(&_pool, other_id)
        .await
        .expect("建会话失败");

    let err = phase_client
        .get_phase(authed(
            crate::api::grpc::pb::GetPhaseRequest { id: phase.id },
            &other_token,
        ))
        .await
        .expect_err("别的用户居然看到了我的阶段？");
    assert_eq!(err.code(), tonic::Code::NotFound);

    // ============================================================
    // 12. 登出后 token 失效
    // ============================================================
    let _ = auth_client
        .logout(authed(crate::api::grpc::pb::LogoutRequest {}, &token))
        .await
        .expect("登出失败")
        .into_inner();
    let err = auth_client
        .get_me(authed(crate::api::grpc::pb::GetMeRequest {}, &token))
        .await
        .expect_err("登出后 token 还能用？");
    assert_eq!(err.code(), tonic::Code::Unauthenticated);

    println!("gRPC 冒烟测试全部通过：一元 / 服务器流 / 客户端流 / 双向流 + 认证 + 隔离");
}
