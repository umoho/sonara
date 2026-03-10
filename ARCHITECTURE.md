# Sonara v0 架构草案

## 产品定位

Sonara 是一个面向游戏的、Rust-first 的开源交互音频中间件（interactive audio middleware）。

当前产品方向：

- Rust-first
- 以 Firewheel 作为执行后端（backend）
- 优先集成 Bevy
- 开源的 authoring + runtime + bank pipeline
- 首版编辑器使用 `egui`

Sonara 不打算在第一天就完整替代 FMOD/Wwise。
Sonara v0 的目标是证明下面几件事：

- 内容可以被创作（authoring）
- 内容可以被编译成 bank
- 内容可以在游戏里运行，并且程序员侧工作流可用
- 内容可以被试听、调试和验证

## v0 范围

### 纳入范围（In Scope）

- 事件驱动（event-driven）的音频创作
- 事件容器（event containers）：
  - `Sampler`
  - `Random`
  - `Sequence`
  - `Layer`
  - `Switch`
  - `Loop`
- 参数（parameter）类型：
  - `Float`
  - `Bool`
  - `Enum`
- 参数作用域（parameter scope）：
  - `Global`
  - `Emitter`
  - `EventInstance`
- Bus 树（bus tree）
- 基础快照（snapshot）
- 基础 2D / 3D 音频
- Bank 构建与加载流程（bank build/load pipeline）
- `egui` 编辑器
- Firewheel 运行时后端
- Bevy 集成
- 基础诊断与试听能力

### 不纳入范围（Out of Scope）

- 音乐时间线系统（music timeline system）
- 对白/语音管线（dialogue/voice pipeline）
- 插件 SDK（plugin SDK）
- 多运行时后端（multiple runtime backends）
- 远程协作（remote collaboration）
- 完整的 DSP patching 编辑器
- 复杂行为虚拟机（complex behavior VM）
- 按拍切换（beat-synchronous）音乐系统

## 核心产品原则

对程序员来说，Sonara 最主要的操作对象应该是：

- `Event`
- `Parameter`
- `Bus`
- `Snapshot`
- `Bank`

程序员不应该需要直接操作底层音频资源（audio asset）或运行时图内部结构（runtime graph internals）。

## 核心概念

### Event（事件）

`Event` 是面向游戏逻辑的主音频行为定义（gameplay-facing audio behavior definition）。

建议字段：

- `id`
- `name`
- `kind`
  - `OneShot`
  - `Persistent`
- `root`
- `default_bus`
- `spatial`
  - `None`
  - `TwoD`
  - `ThreeD`
- `default_parameters`
- `voice_limit`（可选）
- `steal_policy`（可选）

语义建议：

- `OneShot`
  - 适合脚步、爆炸、UI 音这类短生命周期声音
- `Persistent`
  - 适合环境声（ambience）、音乐（music）这类长生命周期声音

### Event 内容树（Event Content Tree）

Sonara v0 使用内容树（content tree），而不是完全自由的图（free-form graph）。

节点分类：

- 容器节点（Container）
  - `Random`
  - `Sequence`
  - `Layer`
  - `Switch`
  - `Loop`
- 叶子节点（Leaf）
  - `Sampler`

说明：

- `Switch` 是一种容器（container）
- v0 中 `Switch` 只由 `Enum` 参数驱动
- v0 中 `Sampler` 是唯一的叶子节点

### Parameter（参数）

参数用于驱动事件变化（event variation）和路由决策（routing decision）。

参数类型：

- `Float`
- `Bool`
- `Enum`

参数作用域：

- `Global`
- `Emitter`
- `EventInstance`

建议字段：

- `id`
- `name`
- `scope`
- `kind`
- `default_value`

`Float` 类型额外字段：

- `min`
- `max`
- `smoothing`（可选）

`Enum` 类型额外字段：

- `variants`

### Bus（总线）

`Bus` 是混音层级（mixer hierarchy）的基本单位。

v0 建议支持：

- 树状层级（tree-based hierarchy）
- 音量（volume）
- 静音（mute）
- 独奏（solo）
- 基础电平表（basic metering）

### Snapshot（快照）

`Snapshot` 是一组混音覆盖（mixer overrides）。

v0 建议支持：

- 修改 bus 音量目标值
- 淡入（fade in）
- 淡出（fade out）

v0 初始范围：

- snapshot 只影响 bus
- snapshot 不改写事件结构

### AudioAsset（音频资源）

`AudioAsset` 是导入后的底层音频资源定义（low-level imported audio resource）。

建议字段：

- `id`
- `path`
- `import_settings`
- `streaming`
- `loop_region`（可选）
- 分析元数据（analysis metadata，可选）

### Bank（内容包）

`Bank` 是运行时加载单元（runtime loading unit），而不只是编辑器中的文件夹。

建议职责：

- 容纳已编译的事件元数据（compiled event metadata）
- 引用常驻媒体（resident media）
- 引用流式媒体（streaming media）
- 定义加载/卸载边界

### Emitter（发声体）

`Emitter` 是绑定到世界实体（entity）的运行时发声源（runtime sound source）。

在 Bevy 中，它应主要表现为一个组件（component）。

## Bank 与资源策略

Sonara 应该学习 FMOD、Wwise、CRIWARE，以及 glTF 风格的封装思路。

推荐结构：

- 创作态数据（authoring data）使用人类可读格式
- 运行时编译数据（compiled runtime data）使用二进制格式
- 事件结构与音频媒体从编译阶段开始就分离

推荐产物拆分：

- 全局配置产物（global config output）
- bank 元数据（bank metadata）
- 常驻媒体 blob（resident media blob）
- 流式媒体 blob（streaming media blob，可选）

关键原则：

不要把 bank 从一开始就设计成单一、不透明的大文件。
要从第一天开始把“逻辑/元数据”和“音频媒体”分开。

## v0 中的音乐系统

v0 不应引入单独的音乐时间线系统。

而应把音乐表达为：

- `Persistent Event`
- 全局参数（global parameters）
- 基于 layer 的 stem 混合
- 淡入 / 淡出 / 交叉淡变（crossfade）

这样可以覆盖：

- 战斗 / 非战斗切换
- 地图 / 区域切换
- 基于配器层（instrumentation/stem）的强度变化

v0 明确不做：

- 节拍感知切换（tempo-aware transitions）
- 过渡片段（transition segments）
- 音乐时间线编辑（music timeline authoring）

## Crate 划分

推荐主 crate：

- `sonara-model`
  - 核心数据模型（core data model）
- `sonara-build`
  - authoring 校验与 bank 构建流程
- `sonara-runtime`
  - 面向游戏逻辑的运行时 API 与实例管理
- `sonara-firewheel`
  - Firewheel 执行后端适配层
- `sonara-bevy`
  - Bevy 集成层
- `sonara-editor`
  - 基于 `egui` 的编辑器 UI
- `sonara-app`
  - 编辑器应用程序二进制入口

依赖方向：

- `sonara-model` 是最底层基础
- `sonara-build` 依赖 `sonara-model`
- `sonara-runtime` 依赖 `sonara-model`
- `sonara-firewheel` 依赖 runtime 和 Firewheel
- `sonara-bevy` 依赖 runtime 和 Bevy 集成类型
- `sonara-editor` 依赖 model 和 build
- `sonara-app` 负责组装 editor/runtime 服务

## 推荐依赖基线

可优先复用的依赖：

- `serde`
- `ron`
- `uuid`
- `indexmap`
- `slotmap`
- `smol_str`
- `camino`
- `thiserror`
- `tracing`
- `tracing-subscriber`
- `symphonia`
- `rubato`
- `memmap2`
- `crc32fast`
- `egui`
- `egui_dock`
- `rfd`
- `notify`
- `directories`

不建议重复造轮子的部分：

- 音频解码（audio decoding）
- 重采样（resampling）
- 文件监听（file watching）
- 原生文件对话框（native file dialogs）
- 基础序列化（basic serialization）

## 程序员侧体验（Programmer Experience）

### Bevy 优先工作流

程序员的主心智模型应是：

- 在 startup system 中初始化音频并加载 bank
- 在 update system 中触发 event 并同步参数
- Emitter / Listener 通过组件（component）挂到实体上
- 音频运行时通过 Bevy 的 resource 访问

推荐对外对象：

- `SonaraPlugin`
- `Res<SonaraAudio>`
- `AudioEmitter`
- `AudioListener`

推荐 API 形状：

- `load_bank(...) -> BankId`
- `unload_bank(bank_id)`
- `play(event_id) -> EventInstanceId`
- `play_on(entity, event_id) -> EventInstanceId`
- `play_persistent(event_id) -> EventInstanceId`
- `play_persistent_on(entity, event_id) -> EventInstanceId`
- `stop(instance_id, fade)`
- `set_global_param(param_id, value)`
- `set_emitter_param(entity, param_id, value)`
- `set_instance_param(instance_id, param_id, value)`
- `set_bus_volume(bus_id, value)`
- `push_snapshot(snapshot_id, fade)`
- `pop_snapshot(snapshot_instance_id, fade)`

推荐 Bevy 侧所有权模型：

- 全局控制通过 `Res<SonaraAudio>`
- 世界中的发声绑定通过 `AudioEmitter`
- 监听绑定通过 `AudioListener`

推荐行为：

- `load_bank` 返回 `BankId`
- 用户可以把 `BankId` 存进自定义 `Resource`
- `play_on(entity, ...)` 要求 entity 上存在 `AudioEmitter`
- 触发 event 时不应要求程序员手动传 `BankId`

## 音频师侧体验（Audio Designer Experience）

主工作流应是：

1. 导入音频资源
2. 创建事件（event）
3. 选择事件类型
4. 使用容器组织内容
5. 绑定参数
6. 路由到 bus
7. 试听（preview）
8. 构建 bank
9. 在运行中的游戏里验证

### 编辑器第一页

推荐布局：

- 左侧：项目浏览器（project browser）
  - `Audio Assets`
  - `Events`
  - `Parameters`
  - `Buses`
  - `Snapshots`
  - `Banks`
- 中间：主编辑区（main editor）
- 右侧：属性检查器（inspector）
- 底部：试听与诊断（preview and diagnostics）

对于空项目，中间区域应优先提供：

- `Import Audio`
- `Create Event`
- `Create Bus`

### 脚步声事件示例

示例事件：

`player.footstep`

建议结构：

```text
player.footstep
└── Switch(surface)
    ├── wood
    │   └── Random
    │       ├── footstep_wood_01
    │       └── footstep_wood_02
    ├── stone
    │   └── Random
    │       ├── footstep_stone_01
    │       └── footstep_stone_02
    └── grass
        └── Random
            ├── footstep_grass_01
            └── footstep_grass_02
```

这个例子直接证明 Sonara 需要：

- `Event`
- `Switch`
- `Random`
- `Enum Parameter`
- `Emitter`
- `Bus`

## 第一条纵向切片（First Vertical Slice）

第一阶段实现目标建议是一条完整的脚步声纵向切片：

1. 导入几条脚步音频资源
2. 创建 `player.footstep`
3. 创建 `surface`，类型为 `Enum`，作用域为 `Emitter`
4. 通过 `Switch(surface)` 组织事件
5. 构建 bank
6. 在 Bevy 中加载 bank
7. 在带有 `AudioEmitter` 的 entity 上播放该事件
8. 根据不同地面材质听到不同脚步变体

如果这条切片跑通，Sonara 就从讨论阶段进入可运行的中间件原型阶段。

## 立即下一步

建议接下来的行动：

1. 把这份草案继续收敛成 workspace 计划
2. 创建 Cargo workspace 和初始 crates
3. 在 `sonara-model` 中形式化核心类型
4. 实现最小 bank 构建路径
5. 通过 Firewheel 接上最小 Bevy 播放路径
