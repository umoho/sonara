# Sonara 音乐系统演进计划

## 1. 目标

这份计划面向当前讨论过的三类音乐能力：

- `[1]` 记忆切出/切入点
  - 每个音乐状态或片段保留自己的播放进度
  - 短时间内切回时从上次进度继续播放
  - 允许配置超时后重置为默认入口
- `[2]` 基于用户配置切点的同步切换
  - 当前片段不立刻切走
  - 等到源片段的下一个合法退出点
  - 可选播放过渡片段，再进入目标片段
- `[3]` 同步结构的多版本音乐切换
  - 例如昼/夜两个版本、探索/战斗两个配器版本
  - 保持结构同步，在相同音乐位置切换到另一条
  - 也应为后续的 stem 开关预留空间

目标不是把 Sonara 变成完整 timeline 音乐编辑器，而是在现有 `Event + Bank + Runtime + Firewheel + Bevy` 主线上，补出一层足够稳定的音乐语义。

## 2. 当前代码基线

结合当前仓库实现，可以明确得到以下约束：

- `AuthoringProject` 当前只有：
  - `assets`
  - `parameters`
  - `buses`
  - `snapshots`
  - `events`
  - `banks`
- `Event` 叶子节点当前只有 `SamplerNode { asset_id }`
  - 还没有 `Clip` / `Cue` / `ResumeSlot` 之类的对象
- `PlaybackPlan` 当前只有：
  - `event_id`
  - `emitter_id`
  - `asset_ids`
- `SonaraRuntime` 当前只负责：
  - 参数解析
  - 事件树解析
  - 活动实例登记
  - 不负责播放头、切点等待、异步状态机
- `FirewheelBackend` 当前只负责：
  - 资源准备
  - 从头播放 sample resource
  - `PendingMedia -> Playing`
  - 立即 stop
  - 还没有播放头查询、offset 起播、非立即 fade、定时切换
- `music_zone` 示例本质上是：
  - 修改全局参数 `music_state`
  - 立即 stop 旧实例
  - 重新 play 新实例
  - 不是异步音乐状态机

因此，功能 `[1][2][3]` 都不适合继续塞进当前的 `Switch(music_state) -> Sampler(asset_id)` 路径中补丁式演进。

## 3. 建议的总架构

### 3.1 两层抽象

建议把音乐能力拆成两层：

1. `transport` / `music-foundation` 层
   - 解决“从什么内容开始播、播到哪里、有哪些切点、怎么恢复、怎么对齐”
   - 这是较通用的底层能力
2. `music-graph` 层
   - 解决“有哪些音乐状态、从哪个状态到哪个状态怎么切”
   - 这是更偏音乐 authoring 的高层语义

### 3.2 不建议的方向

以下方案不推荐作为主线：

- 继续只用 `set_global_param("music_state")`
  - 无法表达 `desired_state != active_state`
  - 无法表达“等待下一个切点再生效”
- 把所有音乐能力都强塞进现有 `Event` 内容树
  - 会让 `Event` 同时承载 one-shot/SFX 和复杂音乐状态机
  - 容易让 runtime 请求模型变形
- 一开始就做完整 timeline system
  - 范围过大，且不符合 Sonara 当前阶段

### 3.3 推荐的总体决策

- 保留现有 `Event` 体系，用于：
  - one-shot
  - 一般持久音
  - 简单的参数驱动切换
- 新增并行的 `MusicGraph` 体系，用于：
  - `[1]` 播放头记忆
  - `[2]` cue 驱动切换
  - `[3]` 同步结构切换
- 底层先补 `Clip + Cue + Resume + Sync` 能力
- 高层再补 `MusicGraph + TransitionRule + MusicSession`

## 4. 推荐对象模型

### 4.1 Foundation 层

这一层不直接表达“预热/战斗”等业务状态，而是表达音乐 transport 语义。

建议新增对象：

- `Clip`
  - 一个可播放音乐片段
  - 引用底层 `AudioAsset`
  - 支持文件内选段
  - 支持可选 loop 区间
- `CuePoint`
  - 用户配置的切点
  - 包含时间位置和一个或多个 tag
- `ResumeSlot`
  - 播放头记忆槽
  - 默认可与某个状态一一对应
  - 也允许多个状态共享
- `SyncDomain`
  - 一组结构可对齐的内容
  - 供 `[3]` 使用
  - 用于表达“切到另一条时沿用同一个音乐位置”

建议的数据形态：

```text
AudioAsset
└── Clip
    ├── source range
    ├── loop range
    ├── cues[]
    └── optional sync_domain
```

### 4.2 Music 层

这一层表达音乐状态图。

建议新增对象：

- `MusicGraph`
  - 一个音乐状态机/状态图
- `MusicStateNode`
  - 图中的一个状态节点
- `TransitionRule`
  - `from_state -> to_state` 的切换规则
- `PlaybackTarget`
  - 节点最终绑定的播放目标

`PlaybackTarget` 不要只限定为“单个 clip”，建议预留成可扩展枚举：

- `Clip`
- `SyncVariant`
- `StemSet`

这样 `[3]` 可以在不推翻前面设计的前提下演进。

### 4.3 Memory 策略

建议不要把“按状态记”或“按 clip 记”写死，而是使用显式 `memory_slot`：

- 默认：`memory_slot = state_id`
- 高级用法：
  - 多个状态共用同一个记忆槽
  - 多个状态故意不共享记忆

同时建议引入 `MemoryPolicy`：

- `ttl`
  - 超过多久未切回，则丢弃旧播放头
- `reset_to`
  - 失效后从哪里重新进入

这可直接覆盖类似《原神》那种：

- 短时间脱战再入战斗，沿用旧进度
- 过了较久，再次进入则重新开始

## 5. 推荐 API 方向

### 5.1 保留现有通用 API

现有 API 继续保留：

- `load_bank`
- `play`
- `play_on`
- `stop`
- `set_global_param`
- `set_emitter_param`

这些 API 仍然适合：

- SFX
- 环境声
- 简单持久音
- 不需要异步切换状态机的音乐

### 5.2 新增音乐专用 API

建议不要把复杂音乐语义塞进 `RuntimeRequest`，而是增加一套更高层的音乐 API。

建议形状：

```text
play_music_graph(graph_id, initial_state) -> MusicSessionId
request_music_state(session_id, target_state)
stop_music_session(session_id, fade)
music_status(session_id) -> MusicStatus
reset_music_memory(memory_slot)
```

`MusicStatus` 至少应包含：

- `desired_state`
- `active_state`
- `phase`
- `current_target`
- `pending_transition`

其中 `phase` 建议至少有：

- `Stable`
- `WaitingExitCue`
- `PlayingBridge`
- `EnteringDestination`
- `Stopped`

### 5.3 不再推荐的调用方式

对于复杂音乐，后续不再推荐直接让游戏侧做下面这种编排：

```text
set_global_param(music_state)
stop(old_instance)
play(new_event)
```

原因是这会把以下逻辑散落到游戏层：

- 切换请求和真正生效时间的分离
- 等待源段退出点
- 过渡段插入
- 目标段恢复策略
- 播放头记忆

这些都更适合由 Sonara 统一执行。

## 6. Runtime / Backend 分工建议

### 6.1 runtime 负责

- 维护 `MusicSession`
- 保存 `ResumeMemory`
- 解析 `TransitionRule`
- 根据当前播放状态计算：
  - 是否正在等待退出点
  - 下一个合法 cue 是什么
  - 目标状态如何进入
- 管理 `desired_state` 与 `active_state`
- 生成给 backend 的 transport 指令

### 6.2 backend 负责

- 准备媒体资源
- 从指定 offset 起播
- 在 clip 的 loop 区间内循环
- 报告当前播放头
- 执行 fade / crossfade
- 在指定时刻或指定 transport 条件下切换

### 6.3 Bevy 负责

- 暴露简单稳定的调用口
- 每帧推进 backend
- 查询并展示 `MusicStatus`
- 不承载音乐切换状态机本体

## 7. 推荐依赖基线

沿用 `ARCHITECTURE.md` 里的基础依赖之外，音乐演进阶段可优先复用的依赖：

- `firewheel`
  - 继续作为执行后端
  - 阶段 2 优先启用 `scheduled_events` 和 `musical_transport`
  - 直接复用 `schedule_event_for`、`sync_transport`、`audio_clock_instant`
- `firewheel-pool`
  - 继续作为 worker/pool 抽象
  - 适合 persistent music session 持有和轮换 sampler worker
- `firewheel-symphonium`
  - 继续作为 Firewheel 与文件解码链之间的桥接
  - 先复用现有 `load_audio_file` 路径，不要重新造一套加载器
- `symphonium`
  - 继续承担离线解码和重采样
  - 适合 clip 裁切、波形缓存、预分析、导出前预处理
- `symphonia`
  - 继续作为更底层的 demux / codec 能力来源
  - 如果后续需要更细粒度的媒体元数据或逐段解码，可直接下探
- `hound`
  - 继续用于测试 fixture、离线导出、波形校验
  - 这类基础 PCM I/O 不值得自造
- `petgraph`（可选）
  - 适合 `MusicGraph` 的可达性校验、循环检测、编辑器图视图导出
  - 不建议进入 runtime audio hot path
- `rangemap`（可选）
  - 适合索引 cue、loop、sync、TTL 生效区间这类时间范围数据
  - 比手写区间查找更稳
- `realfft` / `rustfft`（可选）
  - 适合 editor 波形、频谱、瞬态候选、拍点候选分析
  - 如果只处理实值音频输入，优先 `realfft`
- `rubato`（可选）
  - 适合离线重采样、sample-rate 对齐、同步变体导出前预处理
  - 不建议先把它拉进 runtime 热路径
- `creek`（可选）
  - 如果后续证明“整文件 decode + 预热”不足以支撑大文件、低内存、局部循环 streaming，可评估引入
  - 这一项应在 Firewheel 现有 sampler 能力摸清之后再决定
- `ebur128`（可选）
  - 适合 authoring 侧 loudness 扫描和归一化建议
  - 对“不同 mp3 响度差异很大”的素材尤其有价值

不建议重复造轮子的部分：

- 音频线程精确定时事件调度
- sampler 播放头查询、pause/resume、offset 起播
- 动态 BPM / 共享 transport
- 图结构可达性和循环校验
- FFT / 频谱分析
- 响度计量

现阶段不建议额外引入一套并列音频引擎。
本次演进应优先吃满 Firewheel 现有能力，只在确认 `Clip` 子区间循环或 streaming 能力不足时，再补可选依赖或自定义 node。

## 8. 分阶段演进计划

### 阶段 0：能力验证 Spike

目标：

- 不改 schema，先验证 backend 侧最核心的 transport 能力缺口

要确认的点：

- Firewheel 当前 sampler 路径是否支持：
  - 非零起播 offset
  - loop 区间
  - 读取播放头
  - 平滑 fade / crossfade
- 如果底层 API 不直接暴露：
  - Sonara 是否能用“已知 sample rate + 启播时间 + 累积偏移”先做一版 cursor 估计
  - 哪些能力必须通过更底层的自定义节点补出来

产出：

- 一份 backend 能力清单
- 一份需要改造 `sonara-firewheel` 的结论

### 阶段 1：Foundation 模型与编译产物

目标：

- 在不打破现有 `Event` 主线的前提下，引入音乐 foundation 对象

新增对象：

- `ClipId`
- `CueId`
- `ResumeSlotId`
- `SyncDomainId`
- `MusicGraphId`
- `MusicStateId`

新增定义：

- `Clip`
- `CuePoint`
- `ResumeSlot`
- `SyncDomain`
- `MusicGraph`
- `MusicStateNode`
- `TransitionRule`

推荐改动位置：

- `sonara-model/src/ids.rs`
- `sonara-model/src/lib.rs`
- 新增：
  - `sonara-model/src/transport.rs`
  - `sonara-model/src/music.rs`
- `sonara-model/src/project.rs`
- `sonara-model/src/bank.rs`
- `sonara-build/src/lib.rs`

重要原则：

- 第一阶段不要把现有 `SamplerNode` 直接替换成 `ClipRefNode`
- 保持 `Event` 路径兼容
- 音乐对象先以并行 schema 引入

编译产物建议：

- `AuthoringProject` 新增：
  - `clips`
  - `music_graphs`
  - `sync_domains`
- `CompiledBankPackage` 新增：
  - `clips`
  - `music_graphs`
  - `sync_domains`
- `BankObjects` 新增：
  - `music_graphs`
  - 可选 `clips`

### 阶段 2：Runtime / Backend Transport 基础

目标：

- 让 Sonara 真正拥有播放头和 clip 级 transport 能力

runtime 侧新增：

- `ClipCursor`
- `ResumeMemoryStore`
- `TransportCommand`
- `TransportStatus`

backend 侧新增：

- `start_clip(asset_id, offset, loop_range)`
- `query_cursor(handle)`
- `fade_to(...)`
- `schedule_handoff(...)`

建议改动位置：

- `sonara-runtime/src/lib.rs`
  - 初期可先在同文件中引入模块化结构
  - 后续可拆出 `music_runtime.rs`
- `sonara-firewheel/src/lib.rs`
  - 先补 transport primitives
  - 再补更高层音乐切换

阶段完成标志：

- 可以播放一个 clip 的局部区间
- 可以从非零 offset 恢复
- 可以读取播放头
- 可以 loop 指定区间
- 可以做最小非即时 fade

### 阶段 3：实现功能 `[1]` 和 `[2]`

目标：

- 建立 `MusicSession` 和 `PendingTransition`
- 让 Sonara 自己接管“等待切点再切”的状态机

这一阶段解决：

- `[1]` 每个 `memory_slot` 的播放头记忆
- `[1]` 基于 `MemoryPolicy.ttl` 的过期重置
- `[2]` 源段等待下一个合法 `Exit Cue`
- `[2]` 可选 `bridge clip`
- `[2]` 过渡结束后进入目标状态

建议运行时状态：

```text
Stable
WaitingExitCue
PlayingBridge
EnteringDestination
Stopped
```

建议改动位置：

- `sonara-runtime`
  - 新增 `MusicSession`
  - 新增 `PendingTransition`
  - 新增 `MusicStatus`
- `sonara-bevy`
  - 新增音乐 session API
  - 新增示例状态展示

这一阶段完成后，`music_zone` 应迁移为：

- 不是直接改参数 + stop/play
- 而是：
  - `play_music_graph(...)`
  - `request_music_state(...)`

### 阶段 4：实现功能 `[3]`

目标：

- 支持结构同步的版本切换

这一阶段建议拆两步：

1. 同步变体切换
   - 两条或多条结构相似的 clip
   - 按相同的同步位置切换
2. stem 分层
   - 同一音乐中按状态开关不同音轨或不同配器层

核心对象：

- `SyncDomain`
- `SyncVariantSet`
- `StemSet`

关键进入策略：

- `SameSyncPosition`

运行时需要新增：

- `SyncCursor`
- 从当前活跃内容映射到目标变体的同步位置

这一阶段完成后，才能比较自然地表达：

- 《八方旅人》昼夜切换
- 《原神》某些区域的探索/战斗同步版本切换

### 阶段 5：编辑器与 authoring 工具

目标：

- 把音乐能力真正交给音频师 authoring，而不是继续靠手写 JSON

建议编辑器能力：

- 文件选段编辑
- cue 编辑
- loop 区间编辑
- graph 可视化
- transition rule inspector
- memory policy 配置
- sync domain 配置

建议改动位置：

- `sonara-editor`
- `sonara-app`
- 对应 demo 资产与编译导出流程

## 9. `music_zone` 示例的迁移建议

当前 `music_zone` 的价值是：

- 演示 compiled bank 加载
- 演示 streaming 预热
- 演示 `PendingMedia`
- 演示全局参数驱动的最小 Persistent 音乐切换

不建议立刻删除它。更好的迁移方式是：

1. 保留当前 `music_zone`
   - 作为“最小 Persistent + Switch + Streaming”示例
2. 新增一个独立示例
   - 例如 `music_graph_zone`
   - 专门演示 `[1]` 和 `[2]`
3. 等 `MusicGraph` 主线稳定后，再考虑是否把 `music_zone` 升级为新语义

这样能避免：

- 在一个示例里混杂两套 API
- 让现有最小音乐主线失去回归价值

## 10. 测试与验证计划

每个阶段都应补对应测试，而不是只靠 demo。

建议新增测试类型：

- `sonara-model`
  - JSON round-trip
  - graph/clip/cue 引用关系
- `sonara-build`
  - 缺失 cue/tag/clip 的校验
  - bank 编译对象清单正确性
- `sonara-runtime`
  - resume memory 写入与恢复
  - ttl 超时后重置
  - pending transition 等待 cue
  - bridge clip 完成后进入目标
  - `desired_state != active_state`
  - sync-domain 切换位置保持
- `sonara-firewheel`
  - 非零 offset 起播
  - loop 区间
  - fade/crossfade
  - streaming 预热后按计划起播
- `sonara-bevy`
  - session API 集成测试
  - HUD 状态反馈

## 11. 关键风险

### 10.1 backend 精度风险

如果 Firewheel 当前 sampler 路径不能直接提供：

- 准确播放头
- 非零 offset 起播
- loop 区间
- 平滑 fade

则 Sonara 需要额外的 backend 改造，甚至可能需要自定义节点或更底层的 worker 控制。

### 10.2 streaming 与切点时序

对于 `[2]` 和 `[3]`，目标内容如果在切点到来前未完成预热，会直接破坏音乐切换的可靠性。

因此需要为音乐专门补一层策略：

- 目标状态预热
- 过渡段优先常驻
- 关键目标 clip 提前准备

### 10.3 schema 膨胀风险

如果一上来就把：

- clip
- graph
- sync
- stem
- stinger
- transition timeline

全部一次性加进 schema，复杂度会快速失控。

建议严格按阶段推进。

## 12. 结论

对 Sonara 来说，更合理的演进路线不是“把现有 `music_state` 参数继续做复杂”，而是：

1. 先在底层补 `Clip + Cue + Resume + Sync`
2. 再在高层补 `MusicGraph + TransitionRule + MusicSession`
3. 保持现有 `Event` 主线不被音乐状态机绑架

这条路线有几个好处：

- 既能支持当前讨论的 `[1][2][3]`
- 又不会把 Sonara 锁死在“预热 -> 过渡 -> 战斗”这一种模式
- 同时保留现有 one-shot / Persistent Event 主线的稳定性
