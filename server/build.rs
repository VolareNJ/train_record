// ============================================================
// build.rs —— 构建脚本（M9 新增）：把 .proto 编译成 Rust 代码
// ============================================================
// 【教学：build.rs 是什么？】
// Cargo 在**编译本 crate 之前**会先编译并运行 build.rs（构建脚本）。
// 它可以：生成代码、设置编译参数、探测系统库……
// 我们用它做一件事：调用 protoc，把 proto/train_record.proto
// 编译成 Rust 源码，写进 OUT_DIR（target/debug/build/train_record-*/out/）。
// 之后 src/api/grpc/mod.rs 里用 `tonic::include_proto!` 把它包含进来。
//
// 完整链路（一次 cargo build）：
//   1. Cargo 编译并运行 build.rs
//   2. build.rs 调 protoc 解析 .proto → 生成 Rust 代码（消息 struct + 服务 trait）
//   3. Cargo 编译 src/ 里的代码，include_proto! 把生成的代码"粘"进模块树
//
// 【教学：为什么要有 build.rs，而不是把生成代码提交进仓库？】
//   生成代码提交进 git 会出现"proto 改了但忘了重新生成"的漂移（还污染 diff）。
//   用 build.rs 后，proto 是唯一事实来源：改 proto → 自动重编（下面的
//   rerun-if-changed 就是告诉 Cargo "这个文件变了要重新跑我"）。
//
// 【教学：protoc 从哪来？ 这是 gRPC 项目最常见的环境坑】
// protoc 是 Google 的 protobuf 编译器（C++ 写的二进制），tonic 自己不含它。
// 三种搞到它的办法：
//   a. 系统安装（apt install protobuf-compiler）→ 换机器就忘了装，CI 也常漏
//   b. 提交进仓库           → 二进制进 git（体积大、平台相关），不优雅
//   c. 用 protoc-bin-vendored crate（ 本项目采用）
//      → 它把各平台的 protoc 二进制打包成 crate，cargo 自动下载当前平台的
// 于是"克隆仓库 → cargo build"就能跑，零前置条件。
//
// 【教学：为什么不写 std::env::set_var("PROTOC", ...)？】
// 让 prost-build 找到 protoc 的"教科书写法"是设 PROTOC 环境变量。
// 但本项目 rust edition 2024 + 项目纪律「禁止 unsafe」：
// 从 Rust 2024 起 `std::env::set_var` 是 unsafe 的（多线程下改环境变量不安全），
// 所以改用 prost-build 提供的 API `Config::protoc_executable(path)` 显式传路径——
// 效果一样，且没有 unsafe。（这也是"读懂库的 API 比背偏方更重要"的例子。）
// ============================================================

fn main() -> Result<(), Box<dyn std::error::Error>>
{
    // 【1. 找到 protoc 二进制】
    // 打包在 crate 里（protoc-bin-vendored-linux-x86_64 等平台子 crate），
    // 返回当前平台的绝对路径。找不到（罕见平台）会返回 Err → 编译期报错，
    // 报错信息比"运行时才发现没装 protoc"友好得多。
    let protoc = protoc_bin_vendored::protoc_bin_path()?;

    // 【2. 让 prost-build 用它，而不是去 PATH 里找】
    let mut config = prost_build::Config::new();
    config.protoc_executable(protoc);

    // 【3. 告诉 tonic-prost-build 生成什么】
    //   build_server(true) → 生成 XxxServer<T> + `Xxx` trait（我们要实现的）
    //   build_client(true) → 生成 XxxClient<Channel>（给 examples/grpc_smoke.rs
    //                        和 M10 的 iced 客户端用；同仓库里有客户端才好自测）
    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        // 【教学：不要把 build_transport(true) 之外的默认值改掉】
        // 生成代码里服务端/客户端都走 tonic 自带传输层（HTTP/2），
        // 我们只需要能起服务器 + 写个本地测试客户端。
        //
        // 【教学：compile_with_config vs compile_protos】
        // compile_protos 内部会自己造一个 prost_build::Config（protoc 走 PATH），
        // 我们要自定义 protoc 路径，所以用 compile_with_config 把自己的 config 传进去。
        .compile_with_config(
            config,
            &["proto/train_record.proto"],
            // include 路径：proto 文件里 `import` 的相对根目录。
            // 本项目只有一个 proto 且无 import，写 ["proto"] 即可；
            // 将来拆多个 proto 文件互相 import 时，这里是"搜索根"。
            &["proto"],
        )?;

    // 【4. 增量编译提示】
    // 告诉 Cargo：这个文件变了就重新跑我（否则改了 proto 不会重新生成代码）。
    println!("cargo:rerun-if-changed=proto/train_record.proto");
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}
