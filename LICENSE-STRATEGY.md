# Sonara 许可证策略

这份文档记录 Sonara 当前采用的许可证策略。

重要说明：

- Sonara 之前的公开 revision 曾在 `Cargo.toml` 中声明过 workspace 级别的 `MIT OR Apache-2.0`
- 如果这些 revision 已经被第三方看到、复制、fork 或下载，就不应假设这些历史授权可以被自动追回
- 这份文档和当前仓库中的新声明，作用于后续继续公开和继续发布的版本

## 目标

Sonara 的核心愿望很简单：

- 世界上始终有一份开源免费的音频中间件可用
- 游戏开发者可以把 Sonara runtime 集成进闭源游戏
- 如果有人修改了 Sonara 自身文件并对外分发，修改过的代码不能永久闭源

## 当前选择

### `MPL-2.0`

当前整个仓库先统一使用 `MPL-2.0`：

- `sonara-model`
- `sonara-runtime`
- `sonara-build`
- `sonara-firewheel`
- `sonara-bevy`
- `sonara-editor`
- `sonara-app`

原因：

- `MPL-2.0` 对 Rust crate 形式的中间件比较容易理解和执行
- `MPL-2.0` 允许闭源游戏把这些 crate 作为更大作品的一部分使用
- 但如果分发方修改了 Sonara 中受 `MPL-2.0` 覆盖的文件，修改过的文件仍必须按 `MPL-2.0` 提供源码
- 在 runtime / build / demo / editor 的依赖边界还没有进一步拆清之前，统一 `MPL-2.0` 比混用多种许可证更稳妥

## 这套方案能做到什么

- Sonara 官方主线可以一直保持开源免费
- 商业公司可以收费分发或收费提供服务
- 但它们不能拿走已经公开版本的开源权利
- 它们如果修改了 `MPL-2.0` 覆盖的 Sonara 文件并对外分发，修改过的文件必须继续开源

## 这套方案做不到什么

- 不能禁止商业使用
- 不能禁止收费分发
- 不能要求“任何人都不能修改 runtime”
- 不能因为有人使用了 `MPL-2.0` runtime，就强制其整个游戏项目开源
- 不能保护 `Sonara` 这个名字和 logo

最后一点需要单独处理：

- 代码许可证管代码
- 项目名称、logo、官方身份应由单独的 `TRADEMARKS.md` 或商标政策处理

## 对 Sonara 的实际意义

如果未来有商业公司 fork Sonara：

- 它们可以收费
- 它们可以提供商业支持、托管、集成和发行服务
- 但它们拿不走 Sonara 官方主线已经公开的代码
- 它们也不能把分发出去的核心 runtime 修改永久藏起来

这正符合 Sonara 的公益目标：

- 不要求世界永远只存在 Sonara 官方版
- 但要求世界上始终存在一份真正开源、真正可得、真正可继续分发的音频中间件

## 后续演进

当前已经完成 manifest 级别的许可证切换。后续还应至少完成以下工作：

1. 在仓库根目录保留正式 `MPL-2.0` 许可证文本
2. 单独新增 `TRADEMARKS.md`，说明 `Sonara` 名称和 logo 不随开源许可证授权
3. 如果未来出现真正独立的托管服务或云端层，再重新评估是否需要引入 `AGPL-3.0-only`

## 参考

- MPL 2.0 FAQ: <https://www.mozilla.org/en-US/MPL/2.0/FAQ/>
- MPL 2.0 文本: <https://www.mozilla.org/en-US/MPL/2.0/>
- Open Source Definition: <https://opensource.org/osd>
