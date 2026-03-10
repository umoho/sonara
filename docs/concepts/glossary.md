# Sonara 术语表

这页文档用于快速解释当前仓库里最常出现的名称和概念。

## `authoring`

内容创作和组织这一层。

它关心的是：

- 导入资源
- 创建事件
- 组织参数、bus、snapshot
- 定义 bank 应该包含哪些对象

它不直接关心运行时如何播放。

## `AuthoringProject`

项目级 authoring 根对象。

当前用于承载一整个音频项目里的内容集合：

- `assets`
- `parameters`
- `buses`
- `snapshots`
- `events`
- `banks`

## `AudioAsset`

导入后的底层音频资源定义。

它更偏 authoring/import 语义，包含：

- 资源路径
- 导入设置
- streaming 策略
- 可选分析信息

当前 backend 加载 compiled bank 时，已经不再直接以 `AudioAsset` 作为主路径。

## `Event`

面向游戏逻辑的音频行为定义。

它解决的是：

- 触发一次事件时，应该播什么
- 根据参数如何选择不同分支
- 最终解析出哪些 `asset_id`

例如：

- `player.footstep`
- `weapon.fire`
- `ui.click`

## `Emitter`

运行时发声体。

它代表一个可绑定到游戏实体上的音频上下文，常用于：

- 角色脚步
- 世界中的音源
- 带局部参数的播放对象

在当前 Bevy 集成里，它主要表现为 `AudioEmitter` 组件。

## `EmitterId`

运行时 emitter 的稳定标识。

有了 `EmitterId`，同一个事件就可以根据不同 emitter 的参数解析出不同结果。

## `Parameter`

驱动事件变化和分支解析的参数。

当前支持：

- `Float`
- `Bool`
- `Enum`

当前主要作用域：

- `Global`
- `Emitter`
- `EventInstance`

## `Bus`

混音层级单位。

它不是具体播放内容，而是 mixer 层的节点。

当前模型里最重要的字段是：

- `name`
- `parent`
- `default_volume`

## `Snapshot`

一组针对 bus 的临时混音覆盖规则。

它不决定“播什么内容”，而决定“当前混音状态怎么变”。

例如：

- `combat`
- `underwater`
- `pause_menu`

当前 runtime 已经有最小 snapshot 状态，但还没真正接到 backend mixer。

## `BankDefinition`

authoring 层的 bank 定义。

它描述的是：

- 哪些 `Event` 应该被打进这个 bank
- 哪些 `Bus` 应该被打进这个 bank
- 哪些 `Snapshot` 应该被打进这个 bank

它表达的是“编译输入”，不是运行时最终产物。

## compiled `Bank`

构建后的 runtime 加载单元。

它是由 `sonara-build` 从 authoring 数据构建出来的产物。

当前分成两部分：

- `BankObjects`
- `BankManifest`

## `BankObjects`

compiled bank 中的高层对象清单。

当前包括：

- `events`
- `buses`
- `snapshots`

runtime 当前主要消费它。

## `BankManifest`

compiled bank 中的媒体清单。

当前包括：

- `assets`
- `resident_media`
- `streaming_media`

backend 当前主要消费它。

## `PlaybackPlan`

runtime 在一次事件触发后生成的最小播放计划。

它描述的是：

- 是哪个事件
- 绑定了哪个 emitter
- 最终解析出哪些 `asset_id`

它是 runtime 和 backend 之间最关键的桥梁之一。

## `QueuedRuntime`

纯 runtime 模式下的排队执行前端。

它把：

- `SonaraRuntime`
- 请求缓冲区
- `queue_*`
- `apply_requests`

收在一起，避免这些逻辑继续堆在 Bevy 集成层。

## `sonara-build`

负责 authoring 数据到 compiled bank 的构建层。

当前关键入口：

- `validate_event(...)`
- `build_bank(...)`
- `build_bank_from_definition(...)`

## `sonara-runtime`

负责高层运行时语义：

- 参数
- emitter
- 事件解析
- 实例管理
- `PlaybackPlan`
- snapshot 的最小运行时状态

它决定“该播什么”，不直接负责真实音频输出。

## `sonara-firewheel`

Firewheel backend 适配层。

它负责：

- 加载 compiled `BankManifest`
- 解码资源
- 创建真实 worker
- 把 `PlaybackPlan` 变成声音

它决定“怎么真的播出来”。

## `sonara-bevy`

Bevy 集成层。

它负责：

- 插件入口
- `SonaraAudio`
- `AudioEmitter`
- ECS/system 调用方式

它是集成层，不应该成为 Sonara 主线能力的主要承载者。
