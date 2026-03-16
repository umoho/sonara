# Sonara Licenses

Sonara 当前先统一使用 `MPL-2.0`。

这样做的目的很直接：

- 保持整个仓库的许可证简单一致
- 允许闭源游戏集成 runtime
- 要求对 Sonara 自身文件的修改在分发时继续开源
- 避免在 runtime / build / demo 依赖边界还没有彻底拆干净之前，过早引入多许可证结构

当前这些 crate 都使用 `MPL-2.0`：

- `sonara-model`
- `sonara-runtime`
- `sonara-build`
- `sonara-firewheel`
- `sonara-bevy`
- `sonara-editor`
- `sonara-app`

官方文本与 FAQ：

- <https://www.mozilla.org/en-US/MPL/2.0/>
- <https://www.mozilla.org/en-US/MPL/2.0/FAQ/>

## Historical note

Sonara 的早期公开 revision 曾在 `Cargo.toml` 中声明过 `MIT OR Apache-2.0`。

如果这些 revision 已经被第三方看到、复制、fork 或下载，就不应假设这些历史授权可以被自动追回。
当前文件只说明仓库此后的许可证选择，不构成对历史公开 revision 的追溯撤销。
