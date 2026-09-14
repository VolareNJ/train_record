# 部署文档（systemd）

train_record 是 Rust 单二进制 Web 应用，使用 systemd 管理生命周期。
本文档从零部署 + 日常升级流程。

---

## 一、目录结构（生产）

```
/opt/train_record/          # 工作目录（WorkingDirectory）
├── train_record            # release 二进制
├── sw.js                   # Service Worker
└── static/                 # 静态资源（CSS/JS/图标/manifest）
    ├── style.css
    ├── weight_converter.js
    ├── icon-192.png
    ├── icon-512.png
    └── manifest.json

/var/lib/train_record/      # 数据目录
└── train_record.db         # SQLite 数据库
```

二进制和静态资源放 `/opt/train_record/`，数据库放 `/var/lib/train_record/`
（`DATABASE_PATH` 环境变量指定），升级时两者互不干扰。

---

## 二、从零部署

### 1. 安装依赖

Ubuntu 24.04 + Rust 工具链（**stable**，edition 2024）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# 构建/测试只需 stable（rustup 默认就是它，不必额外装）
rustup toolchain install stable
# 只有格式化需要 nightly：rustfmt.toml 里的 brace_style（大括号换行）
# 是 nightly 才支持的 unstable 选项
rustup toolchain install nightly
```

> 项目**不锁工具链**（已删掉 `rust-toolchain.toml`）：构建默认走 stable，
> 格式化时用 `cargo +nightly fmt` 显式指定 nightly。

### 2. 构建 release 版

```bash
cd <源码目录>
cargo build --release
```

产物：`target/release/train_record`

### 3. 准备目录

```bash
sudo mkdir -p /opt/train_record /var/lib/train_record
sudo cp target/release/train_record /opt/train_record/
sudo cp -r static sw.js /opt/train_record/
```

### 4. 创建 systemd unit

`/etc/systemd/system/train_record.service`：

```ini
[Unit]
Description=train_record web server
After=network.target

[Service]
WorkingDirectory=/opt/train_record
ExecStart=/opt/train_record/train_record
Environment=PORT=80
# M9：gRPC 出口监听端口（与 HTTP 同一个进程、并行监听）
# 本机/内网使用无需放行；外网客户端访问需在防火墙/安全组放行 50051
Environment=GRPC_PORT=50051
# 【可选】gRPC 启用 TLS：两个变量必须**成对**出现在这里
# （只写一个会直接启动失败——静默降级成明文比报错危险得多）
# 证书怎么生成、客户端怎么信任，见「六、gRPC 启用 TLS」
#Environment=GRPC_TLS_CERT=/etc/train_record/tls/cert.pem
#Environment=GRPC_TLS_KEY=/etc/train_record/tls/key.pem
Environment=DATABASE_PATH=/var/lib/train_record/train_record.db
Environment=BODY_PART_ORDER=腿,背,胸,核心,手臂,肩
Restart=on-failure
RestartSec=3

[Install]
WantedBy=multi-user.target
```

### 5. 启动并开机自启

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now train_record
```

验证：

```bash
systemctl status train_record        # active (running)
curl -I http://localhost/            # HTTP/1.1 200（或 302 → /login）
journalctl -u train_record -f        # 实时日志
```

---

## 三、日常升级流程

> 核心原则：**先备份，再升级，随时可回滚**。

```bash
# 1. 备份数据库 + 旧二进制
sudo cp /var/lib/train_record/train_record.db /var/lib/train_record/train_record.db.bak.$(date +%Y%m%d_%H%M%S)
sudo cp /opt/train_record/train_record /opt/train_record/train_record.bak.$(date +%Y%m%d)

# 2. 构建新版本（开发机或服务器本地）
cd <源码目录>
cargo build --release

# 3. 拷贝二进制 + 静态资源（静态资源变了才拷）
sudo cp target/release/train_record /opt/train_record/train_record
sudo cp -r static sw.js /opt/train_record/

# 4. 重启
sudo systemctl restart train_record

# 5. 验证
systemctl status train_record
curl -I http://localhost/today
```

回滚：把备份的二进制/数据库拷回去再 `systemctl restart`。

---

## 四、运维命令速查

| 操作 | 命令 |
|------|------|
| 查看状态 | `systemctl status train_record` |
| 实时日志 | `journalctl -u train_record -f` |
| 最近 100 行日志 | `journalctl -u train_record -n 100` |
| 重启 | `sudo systemctl restart train_record` |
| 停止 | `sudo systemctl stop train_record` |
| 开机自启 | `sudo systemctl enable train_record` |
| 取消自启 | `sudo systemctl disable train_record` |

---

## 五、注意事项

- **端口 80 需要 root**：systemd 服务默认以 root 运行（本部署未降权），
  若要以低权限用户运行，需要授予 `CAP_NET_BIND_SERVICE` 或用 8080 等高位端口。
- **M9 gRPC 端口（`GRPC_PORT`，默认 50051）**：程序启动时会同时监听 HTTP 与 gRPC。
  验证是否在听：`ss -ltnp | grep 50051`；启动日志里也有一行
  `gRPC 服务监听 0.0.0.0:50051（...共 6 个 service）`。
  gRPC 默认明文（无 TLS），只建议内网/本机使用；公网暴露前请启用 TLS
  （设 `GRPC_TLS_CERT` / `GRPC_TLS_KEY` 两个环境变量，见「六」）。
  启动日志会明说当前是 TLS 还是明文，不用猜。
- **数据库备份优先于二进制**：数据不可再生，二进制随时可重编译。
- **静态资源版本化**：`weight_converter.js` 用 `?v=` 查询串更新，
  升级后如客户端仍显示旧 JS，强制刷新即可（SW 缓存由版本号自动清理）。
- **SW 缓存版本**：`sw.js` 的 `CACHE` 常量升级时 +1，`activate` 自动清旧缓存。
- **静态资源 Cache-Control**：服务器对 `/static/` 统一返回
  `Cache-Control: no-cache`（每次用 ETag 重新验证）。**不要改成**
  `max-age` 长缓存——否则更新 manifest.json / CSS 后，浏览器可能
  命中启发式缓存仍显示旧文件（实测踩坑：无缓存头时浏览器按
  `(now - Last-Modified) * 10%` 猜新鲜度，SW install 的 addAll
  也会缓存陈旧响应）。

---

## 六、gRPC 启用 TLS（公网暴露前必做）

### 为什么必须做

明文 gRPC 在公网上会漏两样东西：

1. **metadata 里的 token**（`authorization: Bearer <token>`）——中间人拿到就能冒充你
2. **请求/响应正文**——`Login` 请求里就是账号密码

### 1. 生成证书（本项目目前无域名 → 自签）

```bash
# 在工作目录外生成（别提交进 git），SAN 必须写客户端实际访问的地址
mkdir -p /etc/train_record/tls && cd /etc/train_record/tls

openssl req -x509 -newkey rsa:2048 -days 3650 -nodes \
  -keyout key.pem -out cert.pem \
  -subj "/CN=train_record" \
  -addext "subjectAltName=IP:<服务器公网 IP>" \
  -addext "extendedKeyUsage=serverAuth"

chmod 600 key.pem      # 私钥只给属主读
chmod 644 cert.pem
```

注意：

- **必须写 SAN**（Subject Alternative Name）：现代 TLS 实现（rustls 也是）
  只看 SAN、不看 CN，只写 CN 的证书会被客户端直接拒。
  客户端用域名访问就写 `DNS:your.domain`，用 IP 就写 `IP:1.2.3.4`（可写多个）。
- 自签证书**不会**被系统/浏览器自动信任——客户端要显式信任这张证书（见第 3 步）。
- 如果以后有域名：改用 Let's Encrypt（certbot / caddy 自动续期），
  客户端信任系统根证书即可，就不用第 3 步了。

### 2. 让服务启用 TLS

在 systemd unit 里取消注释那两行（见「二、从零部署」第 4 步），或手动启动时：

```bash
GRPC_TLS_CERT=/etc/train_record/tls/cert.pem \
GRPC_TLS_KEY=/etc/train_record/tls/key.pem \
/opt/train_record/train_record
```

```bash
sudo systemctl restart train_record
journalctl -u train_record -n 20 | grep gRPC
# 应看到：gRPC 已启用 TLS（证书 /etc/train_record/tls/cert.pem）
# 没配证书时会看到：gRPC 明文模式（未设置 GRPC_TLS_CERT / GRPC_TLS_KEY）……
```

### 3. 验证与客户端信任

服务器侧（验证证书确实在服务）：

```bash
openssl s_client -connect 127.0.0.1:50051 -CAfile /etc/train_record/tls/cert.pem -brief
# 期望：Protocol version: TLSv1.3 / Verification: OK
```

客户端侧（grpcurl 人工调试）：

```bash
grpcurl -cacert /etc/train_record/tls/cert.pem -proto proto/train_record.proto \
  -import-path . -d '{"username":"admin","password":"***"}' \
  <服务器地址>:50051 train_record.v1.AuthService/Login
```

（对比：明文时用的是 `-plaintext`，换 TLS 后改成 `-cacert`。）

M10 的 iced 客户端（tonic）：

```rust
let tls = tonic::transport::ClientTlsConfig::new()
    .ca_certificate(tonic::transport::Certificate::from_pem(ca_pem))
    // 证书 SAN 里写的地址要与这里一致，否则主机名校验不过
    .domain_name("<服务器 IP 或域名>");
let channel = tonic::transport::Channel::from_static("https://<服务器地址>:50051")
    .tls_config(tls)?
    .connect()
    .await?;
```

### 4. 证书轮换

tonic 不支持热重载证书：换证书（自签到期 / 续签）后需要 `sudo systemctl restart train_record`。
自签 10 年有效期下这件事几乎不会遇到；将来上了 Let's Encrypt（90 天）
再考虑“续签后自动重启”的钩子（certbot 的 `--deploy-hook`）。

---

## 七、网页版（HTTP）的 HTTPS：已决定暂缓

> **当前决定（M9 收尾）**：暂不做——不为它买域名。保持明文，代价见下；
> 将来要做的话，不买域名只能走“进程内 TLS + 自签证书”（浏览器需手动信任例外）。

现状：网页版是**明文 HTTP**（`axum::serve`，生产跑在 80 端口），没有反代。
两个后果：

1. 登录密码与会话 cookie 在公网上是明文（比 gRPC 更早暴露在人眼前）；
2. **PWA / Service Worker 装不上**：浏览器只在 HTTPS（或 localhost）下允许注册 SW，
   所以 `sw.js`/manifest 在公网 IP + HTTP 下不会生效。

两条路线（将来选一条，记在 `docs/todo.md` §1.10。注意：当前决定是暂缓）：

- **买域名 + 反代（推荐）**：Caddy/nginx 终结 TLS，自动签发续期（Caddy 只需三行配置），
  同时解决 HTTP 与 gRPC（`reverse_proxy h2c://127.0.0.1:50051`），Rust 侧代码零改动。
- **进程内 TLS（无域名可行的方案）**：把 `axum::serve` 换成 `axum-server` + rustls，
  用同一张自签证书；代价是多一个依赖，且浏览器会报证书不受信任（需手动加例外）。
