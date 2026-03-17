# Sonara 模块拆分重构计划

## 1. 目标

这份文档专门记录本次 `lib.rs` 过长问题的模块拆分重构计划。

本次重构的目标不是改变 Sonara 的功能语义，而是把已经过长、职责混杂的 crate 入口文件拆成更清晰的模块边界，为后续 `Bus + Effect + Automation`、音乐系统、backend 扩展继续演进提供更稳定的结构基础。

本次重构的核心目标：

- 按职责拆分超长 `lib.rs`
- 尽量保持现有公开 API 稳定
- 不引入新的运行时语义变化
- 为后续 `mix / bus / effect / automation` 留出自然模块边界
- 让后续新增能力不再继续堆回单个超长文件

## 2. 当前基线

当前仓库里，以下 crate 的 `lib.rs` 已经明显过长：

- `sonara-runtime/src/lib.rs`
  - 约 `3963` 行
- `sonara-firewheel/src/lib.rs`
  - 约 `2079` 行
- `sonara-build/src/lib.rs`
  - 约 `1666` 行
- `sonara-bevy/src/lib.rs`
  - 约 `1241` 行

这些文件的共同问题是：

- 同时承载公开类型、错误、帮助函数、核心逻辑、测试
- event / music / bus / snapshot / backend / build 流程混在一起
- 单文件阅读和评审成本过高
- 后续再加入 `Effect` / `Automation` 时，容易继续放大结构混乱

当前 `sonara-model/src/lib.rs` 体量很小，仍然只是模型 re-export 入口，因此不在本次主拆分范围内。

## 3. 重构原则

### 3.1 行为优先稳定

本次重构以“结构调整”为主，不以修改功能语义为目标。

除非拆分过程暴露出必须顺手修正的问题，否则应默认：

- 运行时行为不变
- backend 行为不变
- 已有 demo / test 预期不变

### 3.2 按职责拆，不按长度拆

不采用“平均切成几个文件”的做法。

更合理的原则是：

- 公开类型一组
- 错误一组
- 命令缓冲或 facade 一组
- event / music / mix / build 等核心域逻辑分别成组

### 3.3 保持 crate 外部 API 稳定

理想状态下，crate 对外公开路径尽量不变。

也就是说：

- 继续从 crate 根导出主要类型
- 把模块拆分留在 crate 内部
- 避免让这次重构把使用方一起拖进大规模路径改名

### 3.4 先拆出长期边界，再谈后续能力

本次重构不是纯粹“整理代码”，而是为下一阶段演进服务。

特别是 `sonara-runtime` 和 `sonara-firewheel`，本次拆分要优先给这些长期方向留边界：

- `mix`
- `bus`
- `snapshot`
- `effect`
- `automation`

即使本次还没有把后几项全部实现，也不应再把它们继续塞回单个超长文件。

### 3.5 每一步都应可独立验证

模块拆分不应形成“最后一次性收尾”的大爆炸式改动。

更稳妥的方式是：

- 每个 crate 的拆分都应单独通过编译
- 优先用已有测试守住行为
- 需要时补最小测试，确保重构没有引入回归

## 4. 本次重构范围

### 4.1 纳入范围

本次计划纳入以下 crate：

- `sonara-runtime`
- `sonara-firewheel`
- `sonara-bevy`
- `sonara-build`

### 4.2 暂不纳入范围

本次不作为主目标处理：

- `sonara-model`
- `sonara-app`
- demo bank JSON 结构重排
- example 行为改写

如果拆分过程中需要极小的配套改动，应以兼容现有行为为前提。

## 5. 模块拆分草案

### 5.1 sonara-runtime

`sonara-runtime` 是本次拆分的第一优先级。

当前它同时承担：

- 公开 ID 和状态类型
- 请求缓冲
- 错误类型
- bank/object 加载
- event 规划
- music session 状态机
- bus / snapshot / mix 逻辑
- 大量内部辅助函数和测试

建议拆分为：

- `ids.rs`
  - `EventInstanceId`
  - `SnapshotInstanceId`
  - `MusicSessionId`
  - `EmitterId`
- `types.rs`
  - `Fade`
  - `PlaybackPlan`
  - `EventInstanceState`
  - `ActiveEventInstance`
  - `ActiveSnapshotInstance`
  - `MusicPhase`
  - `PendingMusicTransition`
  - `ActiveMusicSession`
  - `TrackGroupState`
  - `MusicStatus`
  - `ResumeMemoryEntry`
  - `ResolvedMusicPlayback`
  - `NextCueMatch`
- `commands.rs`
  - `RuntimeRequest`
  - `RuntimeRequestResult`
  - `AudioCommandBuffer`
  - `AudioCommandOutcome`
  - `QueuedRuntime`
- `error.rs`
  - `RuntimeError`
- `bank.rs`
  - bank/object load/unload
  - object lookup
- `events.rs`
  - event planning
  - switch resolution
  - active instance lifecycle
- `music.rs`
  - music session lifecycle
  - transition resolution
  - entry/resume/cue logic
- `mix.rs`
  - bus gain
  - snapshot bookkeeping
  - 后续 effect / automation 的自然落点
- `tests.rs`
  - 尽量统一承接 crate 级测试

重构后的 `lib.rs` 只负责：

- 声明模块
- re-export 公共类型
- 保持 crate 外 API 入口稳定

### 5.2 sonara-firewheel

`sonara-firewheel` 是第二优先级。

当前它同时承担：

- backend 对外 API
- 资源加载
- worker 创建/释放
- event 播放
- music 调度
- live bus 同步
- 一系列 Firewheel 适配辅助函数

建议拆分为：

- `error.rs`
  - `FirewheelBackendError`
- `types.rs`
  - `InstancePlayhead`
  - pending playback 状态类型
- `assets.rs`
  - manifest 注册
  - 资源解码/缓存
  - streaming 相关
- `events.rs`
  - event playback 路径
  - event worker 管理
- `music.rs`
  - music playback 路径
  - transition / exit cue / node completion 调度
- `workers.rs`
  - worker 绑定、停止、清理
  - live bus gain 同步
  - 后续 bus effect 接线的自然位置
- `backend.rs`
  - `FirewheelBackend` 主 struct
  - 顶层协调入口
- `tests.rs`
  - 如后续补测试，集中放置

重构后的 `lib.rs` 继续作为 crate 根入口，对外 re-export 主 backend 类型和错误类型。

### 5.3 sonara-bevy

`sonara-bevy` 的问题主要不是算法复杂，而是 facade 与测试混在一起。

建议拆分为：

- `plugin.rs`
  - `SonaraPlugin`
  - `SonaraFirewheelPlugin`
  - update system
- `audio.rs`
  - `SonaraAudio`
  - `AudioUpdate`
  - 统一对外 facade API
- `components.rs`
  - `AudioEmitter`
  - `AudioListener`
- `error.rs`
  - `AudioBackendError`
- `prelude.rs`
  - crate 预导出
- `tests.rs`
  - Bevy 集成测试

如果后续 facade 继续扩大，也可以再细分 runtime-only 和 firewheel-only 适配逻辑，但本次不追求过度拆分。

### 5.4 sonara-build

`sonara-build` 当前混合了：

- build 错误
- compiled bank package
- JSON IO
- event / music graph 校验
- bank 构建
- project 编译入口
- export 到文件的包装函数

建议拆分为：

- `error.rs`
  - `BuildError`
  - `CompiledBankFileError`
  - `ExportBankError`
  - `ProjectBuildError`
  - `ProjectExportBankError`
- `package.rs`
  - `CompiledBankPackage`
  - JSON 文件读写
- `validate.rs`
  - `validate_event`
  - `validate_music_graph`
  - 各种 `validate_ref*`
- `compile.rs`
  - `build_bank`
  - `build_bank_from_definition`
  - `compile_bank_definition`
- `project.rs`
  - `compile_project_bank`
  - `compile_project_bank_file`
  - `*_to_file`
- `media.rs`
  - media residency
  - manifest 资产登记

`sonara-build` 的拆分优先级低于 runtime/backend，但应纳入同一轮重构计划，因为它会直接承接后续 `Bus / Effect / Automation` 进入 authoring/build 流程时的结构扩张。

## 6. 建议实施顺序

本次重构建议按以下顺序推进：

### 阶段 0：计划文档

- 写明目标、范围、边界、模块草案
- 作为后续拆分的判断基线

### 阶段 1：sonara-runtime

- 先拆 `runtime`
- 保持对外 API 稳定
- 优先把 `mix` 边界拆出来

这是后续 `Bus + Effect + Automation` 主线最重要的基础。

### 阶段 2：sonara-firewheel

- 再拆 backend
- 把 assets / events / music / workers 分开
- 给 live bus、后续 bus effect 留清晰落点

### 阶段 3：sonara-bevy

- 最后拆 facade
- 重点是把 plugin、audio entry、components、tests 分开

### 阶段 4：sonara-build

- 在 runtime/backend 主路径拆稳后，再拆 build
- 给未来 authoring/build 校验扩展留边界

## 7. 风险与注意事项

本次重构需要特别注意：

- 不要在拆模块时顺手改动过多功能逻辑
- 不要让 crate 外部公开路径大规模漂移
- 不要把测试遗漏在旧文件里，造成后续维护断裂
- 不要把 `mix`、`music`、`events` 再次搅回同一模块

尤其是：

- `sonara-runtime::mix`
- `sonara-firewheel::workers`
- `sonara-build::validate`

这几个模块未来很可能继续长功能，本次边界要尽量立正。

## 8. 验证要求

每个阶段完成后，至少应执行：

- `cargo fmt`
- `cargo test -p sonara-runtime -p sonara-firewheel -p sonara-bevy`

在涉及 `sonara-build` 的阶段，还应补跑：

- `cargo test -p sonara-build`

如拆分影响 Bevy 侧集成或示例编译，可额外执行：

- `cargo check -p sonara-bevy --examples`

## 9. 本次重构的完成标准

可以认为本次模块拆分重构完成的标志是：

- 目标 crate 的超长 `lib.rs` 已降为薄入口
- 各 crate 内部职责边界比当前清晰
- 公开 API 基本保持稳定
- `Bus + Effect + Automation` 后续演进不再需要继续堆回单文件
- 现有测试和示例编译保持通过
