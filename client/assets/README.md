> AI 生成

# 字体资源

`DroidSansFallbackFull.ttf` 取自系统 `google-droid-sans-fonts` 包，用于在
WebAssembly 环境显示中文。iced 的文本渲染不读取浏览器系统字体，故需内嵌。

- 字体名：Droid Sans Fallback
- 许可：Apache License 2.0
- 来源：Android `DroidSansFallbackFull.ttf`

若需减小 WASM 体积，可用 `fonttools` 的 `pyftsubset` 按实际用字裁剪字体，
再把裁剪后的文件放回本目录，并在 `src/main.rs` 更新文件名。
