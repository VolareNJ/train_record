> AI 生成

# 训练记录 WASM 客户端

手机优先的 iced 0.14 客户端，编译为 WebAssembly，与 `server` 同源部署。

## 构建

前置：

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

构建：

```bash
cd client
trunk build --release
```

输出目录由 `Trunk.toml` 指定为 `../server/static/app`。构建完成后：

```text
http://<服务器>:<PORT>/static/app/
```

`Trunk.toml` 的 `public_url` 固定为 `/static/app/`，故资源引用均在同源路径下，
无需修改服务端路由。

## 本地开发

```bash
# 终端一：后端
cd server
PORT=8080 cargo run

# 终端二：前端热重载（/api 代理到 8080）
cd client
trunk serve
```

打开 `http://127.0.0.1:8081/static/app/`。代理使浏览器视 `/api` 为同源，
Cookie 与 Bearer 认证均可用。

## 认证

1. 登录 `POST /api/v1/login`，响应体含 `token`，同时写入 HttpOnly Cookie。
2. WebAssembly 端 Cookie 由浏览器自动携带；内存中的 token 以
   `Authorization: Bearer <token>` 发送（服务端 API 守卫已支持）。
3. 刷新页面后浏览器 Cookie 维持登录态；原生开发版把 token 存到
   `~/.config/train_record_client/token`。

## 手机适配

- `index.html` 设 viewport、禁用缩放、`viewport-fit=cover`，canvas 铺满全屏。
- 单列布局，内容最大宽度 640，底部四栏导航（今日 / 历史 / 动作 / 我的）。
- 输入框与按钮高度按触屏设定；滚动容器占满剩余高度。
- 内嵌 Carlito（拉丁）与 Droid Sans Fallback（中日韩）两套字体；
  iced 在 WebAssembly 下不读取浏览器系统字体，故必须内嵌。
- 默认深色主题，主色 `#4d8dff`。

## 离线与体积

- 字体使 WASM 约 10MB；首次加载后由浏览器缓存。
- `--release` 构建默认开启 `wasm-opt`（trunk 自动下载）。
- 若需减小体积，可用 `pyftsubset` 按实际用字裁剪
  `assets/DroidSansFallbackFull.ttf`；用户可输入任意中文名，裁剪范围需留足。
