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

Ubuntu 24.04 + Rust 工具链（nightly，edition 2024）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup toolchain install nightly
```

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
