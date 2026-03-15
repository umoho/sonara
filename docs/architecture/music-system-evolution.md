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

### 4.4 Fade 与参数自动化

`fade` 不建议长期作为一个只服务音乐切换的临时补丁能力。

更合适的定位是：

- 在用户语义层，`fade` 是播放控制语义
  - `stop fade`
  - `music transition fade`
  - `bridge -> target` 的淡入淡出
  - `snapshot` / `bus` 的音量渐变
- 在 backend 执行层，`fade` 是参数自动化的一种特例
  - 本质上是“某个参数在一段时间内按某条曲线变化”

因此长期建议补的不是“更多 fade 特判”，而是一套更通用的参数自动化基础：

- `AutomationTarget`
  - 目标是谁
  - 例如 `InstanceGain`、`MusicSessionGain`、`BusVolume`、`NodeParam(...)`
- `AutomationCurve`
  - 变化曲线
  - 第一阶段建议只支持少量固定曲线：
    - `Step`
    - `Linear`
    - `EqualPower`
    - `EaseInOut`
- `AutomationSegment`
  - 从何时开始，到何时结束，起点值和终点值是什么

这样后续这些能力都可以共用同一底层：

- `stop(instance, fade)`
- `stop_music_session(..., fade)`
- `music transition fade`
- `bus volume ramp`
- `snapshot fade`
- effect 参数渐变

建议的长期边界是：

- runtime
  - 负责把高层语义翻译成 automation plan
- backend
  - 负责按音频时钟真正执行参数自动化

当前阶段的取舍：

- 先允许 `fade` 作为最小能力独立落地
- 但文档和后续实现都应把它视为“参数自动化系统的第一个用例”
- 不建议把用户 API 长期设计成“先手工加 gain effect，再自己拉参数”

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
play_music_graph(graph_id) -> MusicSessionId
play_music_graph_in_node(graph_id, initial_node) -> MusicSessionId
request_music_node(session_id, target_node)
stop_music_session(session_id, fade)
music_status(session_id) -> MusicStatus
reset_music_memory(memory_slot)
```

`MusicStatus` 至少应包含：

- `desired_target_node`
- `active_node`
- `phase`
- `current_target`
- `pending_transition`

其中 `phase` 建议至少有：

- `Stable`
- `WaitingExitCue`
- `WaitingNodeCompletion`
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
- 管理 `desired_target_node` 与 `active_node`
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

### 当前坐标（截至当前分支工作区）

- `阶段 0` 已完成
  - 已确认 Firewheel 现成提供：
    - `PlayFrom::Seconds / Resume`
    - `SamplerState` 播放头读取
    - `scheduled_events`
    - `musical_transport`
- `阶段 1` 已完成
  - `model/build/runtime` 已引入并贯通：
    - `Clip`
    - `CuePoint`
    - `ResumeSlot`
    - `SyncDomain`
    - `MusicGraph`
    - `Track`
    - `TrackBinding`
  - `MusicGraph` 已切到新的
    - `MusicNode + MusicEdge`
    数据结构
  - 现有 example / bank 已跟进到 `nodes + edges + bindings`
- `阶段 2` 进行中
  - 已完成：
    - 打开 Firewheel 的 `scheduled_events` / `musical_transport`
    - backend 播放头查询
    - backend 立即 seek
    - backend 延时 seek
    - Bevy 侧播放头查询入口
    - `Clip` 直连 Firewheel sampler 播放
    - `source_range.start` 起播和 `source_range.end` 提前停播
    - `MusicGraph -> MusicSession -> Clip` 的真实后端接线
    - 最小非即时 fade
      - `stop(instance, Fade::seconds(...))`
      - `stop_music_session(..., Fade::seconds(...))`
  - 未完成：
    - `loop_range` 子区间循环
    - `schedule_handoff(...)`
- `阶段 3` 已提前完成一部分逻辑骨架
  - runtime / facade 已有：
    - `MusicSession`
    - `PendingTransition`
    - `MusicStatus`
    - `Stable / WaitingExitCue / WaitingNodeCompletion / Stopped`
    - `play_music_graph(...)`
    - `request_music_node(...)`
    - `complete_music_exit(...)`
    - `complete_music_node_completion(...)`
    - `stop_music_session(...)`
    - Bevy facade 的音乐会话 API
  - 真实 backend 已接通：
    - `PlaybackTarget::Clip` 的初始播放
    - immediate 切换
    - bridge 完成后的目标 clip 接续
    - `ResumeSlot` 的播放头回写
    - `EntryPolicy::Resume` 的 offset 恢复
    - `MemoryPolicy.ttl_seconds` 过期回退到 `reset_to`
    - `WaitingExitCue` 的第一版自动等待
    - `EntryPolicy::EntryCue` 的入口 cue 解析
    - `[2]` 的按键触发试听 example
      - 已切到节点图示例：`intro -> warmup -> transition -> climax`
      - `intro -> warmup` 通过初始节点自动 `OnComplete` 前进
      - `warmup` 和 `climax` 通过 `node -> node [OnComplete]` 自循环
      - 用户在 `warmup` 期间请求 `climax` 时，会先走 `warmup -> transition`，再由 `transition -> climax`
    - `music_cue_trigger` 已迁到独立 compiled bank JSON
    - 示例已开始显式使用 `tracks + bindings`
      - `music_resume` 显式声明 `main` track
      - `cue_trigger.bank.json` 已切到新的 4 段 wav 音乐图
    - runtime/backend 已开始消费 `Track`
      - `ResolvedMusicPlayback` 会带上当前 `track_id`
      - Firewheel backend 已按 `session + track` 记住当前激活的音乐内容
    - 节点化重构已经开始落代码
      - `bridge/stinger` 旧特判模型已被节点图主线取代
      - runtime 现在支持请求型 `OnComplete` 边
      - runtime 的自动节点完成测试已迁到显式节点模型
    - `pending media` 延后启动路径已稳定
  - 仍未完成：
    - 基于 cue 的更精确定时切换
    - `ResumeNextMatchingCue`
    - `SyncDomain` 驱动的同步变体切换

已知问题（暂不修复）：

- 当前 `[2]` 的第一版实现中，如果在 `WaitingExitCue` 或 `WaitingNodeCompletion` 阶段反向请求状态切换，`sonara-firewheel` 可能出现音乐会话静音。
  - 更可能的原因是旧 worker 完成回调与新 worker 挂载时序冲突，而不是 `MusicGraph` 状态机本身出错。
  - 当前 MVP 先按《八方旅人》式语义处理：
    - `WaitingExitCue` 和 `WaitingNodeCompletion` 视为锁定阶段
    - example 层不再提供中途反向切换，而是通过重置会话回到 `preheat`

当前最接近的下一步：

- 继续推进更精确的 cue 对拍与 handoff
- 收口 `loop_range` 子区间循环
- 基于参数自动化重新设计 transition fade
- 最后再进入功能 `[3]`：`SyncDomain` 驱动的同步变体 / stem 切换

更精确的 `[2]` cue handoff 建议按以下顺序推进：

1. arm cue 时就计算绝对音频时刻
   - 不再只记录 `target_position_seconds`
   - 同时记录：
     - `target_audio_time_seconds`
     - `target_event_instant`
   - 这样 backend 后续不必完全依赖每帧 `playhead >= cue` 判断

2. 源段退出改成预定停止
   - 当前 source clip 的 stop 不再是“update 发现到了 cue 再 stop”
   - 而是在 cue arm 时就 schedule 到对应 `EventInstant`
   - 这也是未来 transition fade 的挂点

3. `preheat -> bridge` 改成预定启动
   - bridge 不再等 update 某一帧才启动
   - 而是在已知的 cue handoff 时刻直接 `start_time=Some(target_event_instant)`
   - 若资源尚未 ready，则保留当前 fallback 路径

4. `bridge -> combat` 改成预定启动
   - bridge 启动时即可算出它的结束时刻
   - 目标 state 的 clip 也应尽量在那个时刻预定启动
   - 不再主要依赖 `advance_pending_bridge_completions()` 的后验轮询

5. runtime 状态推进与真实音频执行解耦
   - runtime 继续维护：
     - `WaitingExitCue`
     - `WaitingNodeCompletion`
     - `Stable`
   - backend 新增更明确的 pending action / scheduled handoff bookkeeping
   - 音频动作先排好，runtime 只在时刻到达后推进 bookkeeping

6. 旧的 epsilon 轮询逻辑逐步降级为 fallback
   - `advance_waiting_exit_cues()`
   - `advance_pending_bridge_completions()`
   - 先保留容错，再逐步缩小职责

7. transition fade 最后再接回
   - 不和 cue handoff 精度这一轮一起做
   - 等 source stop / bridge start / combat start 三个时刻都稳定后
   - 再以参数自动化方式重新接回 fade/crossfade

### 阶段 0：能力验证 Spike（已完成）

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

当前结果：

- 已验证 Firewheel 现成支持：
  - 非零 offset 起播
  - 播放头读取
  - 未来时刻调度参数变更
  - 共享 musical transport
- 仍待确认或补足：
  - `Clip` 子区间 loop
  - 非立即 fade / handoff
  - `Clip` 级别的直接播放路径

### 阶段 1：Foundation 模型与编译产物（已完成）

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

当前结果：

- 已完成 `sonara-model` 的 ID、schema、导出
- 已完成 `sonara-build` 的 bank 编译、依赖校验、JSON round-trip
- 已完成 `sonara-runtime` / `sonara-firewheel` / `sonara-bevy` 的装载链贯通
- 当前仍保持现有 `Event` 路径兼容，未替换 `SamplerNode`

### 1.1 Track 抽象的长期方向

引入 `MusicGraph / TransitionRule / SyncDomain` 之后，Sonara 后续大概率还需要一层明确的 `Track` 抽象。

这里的 `Track` 不应简单理解为 DAW 编辑器里“时间轴上的一条轨道”，而更应理解为：

- 一条可独立调度的播放层
- 一条可独立混音与自动化的控制层
- 一条可承载不同音乐职责的运行时槽位

建议与 `Bus` 明确区分：

- `Track` 负责：
  - 内容怎么播
  - 谁能与谁并存
  - 谁会替换谁
  - 哪些内容适合独立自动化
- `Bus` 负责：
  - 声音最终流向哪
  - 整体混音、snapshot、send、效果链

这层抽象的价值主要来自三个方向：

- `[2]` 中的 `bridge -> combat` 可在未来更自然地接入 `stinger`
- `[3]` 中的同步变体 / stem 切换，更适合表达为“不同 track 上的内容切换或开关”
- 参数自动化落地后，`track gain` 会比“直接操纵当前所有 worker”更稳定

最小建议不是立刻做完整 timeline system，而是先为以下角色预留：

- `music_main`
- `music_bridge`
- `music_stinger`
- 未来可扩展到：
  - `music_layer/*`
  - `ui`
  - `sfx`

后续若引入 `TrackBinding`，更理想的高层表达会变成：

- `MusicStateNode` 不只绑定单一 `Clip`
- 而是绑定一个或多个 `TrackBinding`
- `TransitionRule` 也可以显式引用：
  - `bridge`
  - `stinger`
  - `stinger_timing`
  - `track_policy`

可借鉴的外部模型：

- Wwise：`Music Segment` 下显式拥有 `Music Track`，并由 `Transition Matrix` 与 `Stinger` 驱动过渡
- CRIWARE / ADX2：`Sequence` 下显式拥有多个 `Track`，并支持 `Track transition by selector` 与 `Block playback`
- FMOD：虽有 `logic tracks / transition timeline`，但更偏“单 event 单时间线”的 cursor 跳转，不宜作为 Sonara 的长期上层模型

关于“容器内放置 Track”这一路线，可进一步收敛为：

- CRIWARE / ADX2 的做法最接近 Sonara 当前想走的方向：
  - `Cue / Sequence` 之内显式包含多个 `Track`
  - 运行时通过 selector、beat sync、crossfade 在 track 间切换
  - 这说明“容器拥有多个 Track”本身是成熟且可行的方案
- Wwise 的分层更细：
  - 更像 `Container -> Segment -> Track`
  - 也就是说，Track 的直接父对象不一定是最高层容器，而可能是中间的 `Segment`
- FMOD 虽然也有 `Track`，但更多是时间线轨道：
  - 更接近未来可能的 `Lane / Arrangement Track`
  - 不宜直接等同为 Sonara 这里的“播放层 Track”

因此对 Sonara 的更具体启发是：

- “容器默认拥有 1 条隐式 main track，需要时再展开多个显式 track”这条思路是可行的
- 这一路线更接近 CRI 的 `Sequence -> Track`
- 如果未来需要更细的 authoring 分层，再考虑往 Wwise 风格的：
  - `Container -> Segment -> Track`
  演进
- 现阶段不建议直接把 `Track` 设计成 FMOD 那类“时间线轨”，否则会把播放层和编排层混在一起

参考链接：

- CRIWARE / ADX2
  - Track transition by selector:
    - https://game.criware.jp/manual/native/adx2_en/latest/criatom_tools_atomcraft_track_transition_by_selector.html
    - https://game.criware.jp/manual/native/adx2_en/latest/criatom_tools_atomcraft_track_transition_by_selector03.html
  - Sync change music:
    - https://game.criware.jp/manual/adx2_tool_en/latest/craftv2_tips_performance_sync_change_music.html
- Wwise
  - Dynamic music design classification:
    - https://www.audiokinetic.com/blog/about-dynamic-music-design-part-1-design-classification/
  - Music segment practice discussion:
    - https://www.audiokinetic.com/qa/713/automatically-loop-a-music-segment
  - Music segment / track learning video:
    - https://www.audiokinetic.com/learn/videos/_bvus5FIjxk/
- FMOD
  - Transition timeline / track behavior discussion:
    - https://qa.fmod.com/t/track-mutes-after-transition-timeline-in-horizontal-music-event-possible-bug/23589

对 Sonara 的直接启发：

- 不要把未来的音乐系统锁死在“单条时间线 + cursor 跳转”上
- 更适合走：
  - `MusicGraph`
  - `Track`
  - `TransitionRule`
  - `Automation`
  这套相对解耦的结构
- 其中 `Track` 更像播放控制层，`Bus` 更像混音路由层

这一方向目前仍停留在设计层，不建议在 `[1][2]` MVP 尚未完全收口前立即实现。

### 1.2 项目级模型示意

如果把 Sonara 放进一个具有多类音乐、多个切换模式、以及大量 one-shot 特效的大项目里，更接近目标形态的整体结构可以整理为：

```text
GameAudioProject
├── Assets
│   ├── MusicFiles
│   └── SfxFiles
│
├── Routing
│   ├── MasterBus
│   ├── MusicBus
│   ├── SfxBus
│   ├── UiBus
│   └── SnapshotDefs
│
├── SharedContent
│   ├── Clips
│   ├── Cues
│   ├── ResumeSlots
│   └── SyncDomains
│
├── Music
│   ├── Graph: WorldRegionA
│   │   ├── States
│   │   │   ├── explore                [功能1]
│   │   │   └── combat                 [功能1]
│   │   └── TransitionRules
│   │       ├── explore -> combat
│   │       └── combat -> explore
│   │
│   ├── Graph: BossBattleFlow
│   │   ├── States
│   │   │   ├── preheat                [功能2]
│   │   │   └── combat                 [功能2]
│   │   └── TransitionRules
│   │       └── preheat -> combat
│   │           ├── exit: NextMatchingCue(battle_ready)
│   │           ├── bridge
│   │           │   └── music_bridge -> ClipRef(boss_bridge)
│   │           ├── stinger            [未来可加]
│   │           │   └── music_stinger -> ClipRef(battle_start_stinger)
│   │           └── destination: EntryCue(combat_in)
│   │
│   └── Graph: DayNightRegion
│       ├── States
│       │   ├── day                    [功能3]
│       │   └── night                  [功能3]
│       ├── Tracks
│       │   ├── music_main
│       │   ├── music_layer_1
│       │   └── music_layer_2
│       ├── SyncDomain
│       │   └── day_night_region_domain
│       └── TransitionRules
│           ├── day -> night
│           └── night -> day
│
├── Sfx
│   ├── Event: player.footstep
│   ├── Event: battle.hit
│   └── Event: ui.confirm
│
└── Runtime
    ├── MusicSessions
    ├── EventInstances
    ├── ResumeMemory
    └── Automation        [未来]
```

这棵树对应的层次划分是：

- `Assets / SharedContent`
  - 管原始媒体、可复用 `Clip`、`Cue`、`ResumeSlot`、`SyncDomain`
- `MusicGraph`
  - 管音乐状态与切换
- `Track`
  - 管哪些内容在哪条播放层上发声
- `TransitionRule`
  - 管一次切换的退出点、bridge、stinger、目标进入方式
- `Sfx Event`
  - 管不属于持续音乐会话的普通 one-shot / 随机 / 条件音效
- `Runtime`
  - 管真实会话、实例、记忆、以及未来的参数自动化

按功能映射：

- `[1]` 主要落在：
  - `MusicGraph.State.memory_slot`
  - `default_entry`
  - `Runtime.ResumeMemory`
- `[2]` 主要落在：
  - `TransitionRule.exit`
  - `bridge`
  - `destination`
  - `MusicSession.phase / pending_transition`
- `[3]` 主要落在：
  - `Track`
  - `SyncDomain`
  - `SameSyncPosition`
  - 将来的 `Stem / Variant` 绑定

对当前 Sonara 的直接启发：

- 现有 `Event` 系统仍适合承载大量 one-shot / 条件触发音效
- 交互音乐应逐步收敛到 `MusicGraph` 主线
- `Track` 更适合先作为音乐层扩展，而不是立刻推广到所有音频对象
- 长远上，`Runtime` 需要同时容纳：
  - `MusicSession`
  - `EventInstance`
  - `Automation`
  这三类并行概念

### 1.3 现有对象盘点与缺口清单

按当前代码现状，Sonara 已经拥有音乐系统的基础骨架，不需要从零重新定义整套模型。更现实的工作方式是：

- 盘点哪些对象已经存在
- 补齐真正还缺的那一小层
- 明确哪些想法先暂缓

当前已经有的模型对象：

- `Clip`
  - `asset_id`
  - `source_range`
  - `loop_range`
  - `cues`
  - `sync_domain`
- `CuePoint`
- `ResumeSlot`
- `SyncDomain`
- `SyncPoint`
- `MusicGraph`
- `MusicStateNode`
- `PlaybackTarget`
  - 当前只有 `Clip { clip_id }`
- `EntryPolicy`
- `ExitPolicy`
- `MemoryPolicy`
- `TransitionRule`

当前已经有的运行时对象：

- `MusicSession`
- `PendingMusicTransition`
- `MusicStatus`
- `ResumeMemory`
- cue 查找与 bridge 状态推进

下一批真正缺失、但最值得补的对象：

- `MusicTrack`
  - 例如：
    - `music_main`
    - `music_bridge`
    - `music_stinger`
- `TrackRole`
- `TrackBinding`
  - 把内容绑定到某条 `Track`
- `TransitionRule.stinger`
- `stinger_timing`
- 更丰富的 `PlaybackTarget`
  - 例如：
    - `SyncVariantSet`
    - `StemSet`

当前明确暂缓的对象：

- `Lane`
- `ClipPlacement`
- 完整 timeline authoring
- 完整参数自动化系统
- 完整的 `[3]` 同步变体运行时

因此下一阶段更合理的策略不是“重写音乐模型”，而是：

1. 保留现有：
   - `MusicGraph`
   - `MusicStateNode`
   - `TransitionRule`
   - `Clip`
   - `Cue`
   - `ResumeSlot`
   - `SyncDomain`
2. 在其上新增：
   - `MusicTrack`
   - `TrackBinding`
3. 再逐步扩展：
   - `stinger`
   - `SyncVariantSet / StemSet`

一句话总结：

- 现阶段不是“设计整个世界”
- 而是“在已有骨架上补 `Track` 这一层”

### 1.4 Track 的最小数据结构草案

按当前阶段的范围控制，`Track` 不宜一上来就做成完整 DAW 时间线，也不宜先扩出过多控制字段。更合适的第一版是：

```text
TrackId
TrackRole
Track
TrackBinding
```

推荐最小结构：

```rust
struct Track {
    id: TrackId,
    name: SmolStr,
    role: TrackRole,
}
```

```rust
enum TrackRole {
    Main,
    Bridge,
    Stinger,
    Layer,
}
```

```rust
struct TrackBinding {
    track_id: TrackId,
    target: PlaybackTarget,
}
```

设计原则：

- `Track` 先只表达“播放层身份”
- `TrackBinding` 先只表达“哪条 Track 播什么”
- 暂时不加：
  - `gain`
  - `mute`
  - `priority`
  - `output_bus`
  - `automation`
  - `lane`
  - `clip placement`

推荐挂载位置：

- `MusicGraph`
  - 新增：
    - `tracks: Vec<Track>`
- `MusicStateNode`
  - 当前的单个 `target`
  - 演进为：
    - `bindings: Vec<TrackBinding>`

兼容策略：

- 默认每个 `MusicGraph` 都拥有一条隐式 `main` track
- 现有的：
  - `MusicStateNode.target`
  语义上等价为：
  - `bindings = [ main -> target ]`
- 因此第一阶段可以做到：
  - 不破坏已有 `[1][2]` 主线
  - 先在模型层建立多轨扩展点

这版最小结构的目标不是立即覆盖所有高级用法，而是先为后续三类能力预留位置：

- `Bridge`
  - 承载 `[2]` 的过渡段
- `Stinger`
  - 承载切换时同步叠加的一次性音效/短乐句
- `Layer`
  - 承载 `[3]` 中的分层配器 / stem / 变体

后续建议顺序：

1. 先引入：
   - `Track`
   - `TrackBinding`
2. 再扩：
   - `TransitionRule.stinger`
   - `stinger_timing`
3. 最后再扩：
   - `SyncVariantSet`
   - `StemSet`
   - 更完整的 `PlaybackTarget`

### 1.5 Bridge / Stinger 的节点化模型

在继续讨论 `Track` 之后，一个更自然、也更接近 Unity Animator 这类状态机编辑器的长期方向是：

- `bridge` 不再被理解成“边上的一段 clip”
- `bridge` 是一个**自动转移节点**
- `stinger` 不再是独立特判
- `stinger` 是**同一节点上另一条 track 的一段素材**
- 边只负责“什么时候能走过去”

进一步收敛后，更推荐把图统一成：

```text
MusicGraph
├── Nodes
│   ├── preheat
│   ├── bridge
│   └── combat
└── Edges
    ├── preheat -> bridge   [NextMatchingCue(battle_ready)]
    └── bridge  -> combat   [OnComplete]
```

这里不一定要执着于“稳定/过渡”这种命名本身，关键是节点要能表达两类行为差异：

- 是否可以被外部直接请求
- 是否会在完成后自动沿边前进

因此更合适的节点级信息是：

- `externally_targetable`
- `completion_source`

例如：

```text
preheat
├── externally_targetable: true
└── completion_source: none

bridge
├── externally_targetable: false
└── completion_source: music_bridge

combat
├── externally_targetable: true
└── completion_source: none
```

其中 `bridge` 节点本身可以携带多条 track：

```text
bridge
├── music_bridge  -> ClipRef(boss_bridge)
└── music_stinger -> ClipRef(battle_start_stinger)
```

这样做的直接好处是：

- 所有真正会发声的内容都落在**节点**上
- 边只表达：
  - `NextCue`
  - `Immediate`
  - `OnComplete`
  这类转移条件
- `bridge_clip` 与 `stinger_clip` 都不必再挂在边上做特判
- backend 也不需要再猜：
  - stinger 是在 `bridge` 开始时播
  - 还是在目标状态接管时播

也就是说：

- 节点负责：
  - 自己要播什么内容
  - 这些内容落在哪些 track
  - 是否需要 memory / entry 等局部语义
  - 节点是否允许被外部直接请求
  - 节点由哪条 track 驱动完成
- 边负责：
  - 从一个节点到下一个节点的转移条件

这一模型还能顺手统一当前的 `stinger` 问题：

- `stinger` 不是“额外时机规则”
- 它只是 `bridge` 节点上 `music_stinger` track 的一段素材
- 进入 `bridge` 节点时，同步启动该节点上的多条 track 即可

这里需要补一个关键概念：

- `completion_source`

原因是 `bridge` 节点中可能同时存在：

- `music_bridge`
- `music_stinger`

但节点何时算“完成”，通常不应由所有 track 一起决定，而应由主导内容决定。对大多数 `bridge` 节点而言，通常是：

- `music_bridge` 负责驱动节点完成
- `music_stinger` 只是伴随层，不决定自动转移时机

因此更完整的节点语义应接近：

```text
bridge
├── TrackBindings
│   ├── music_bridge  -> ClipRef(...)
│   └── music_stinger -> ClipRef(...)
├── externally_targetable: false
└── completion_source: music_bridge
```

另外，“会一直待着”未必要靠节点类型表达，也可以通过图本身表达：

- 简单循环内容可继续使用 `Clip.loop_range`
- 如果需要图层语义，也可以使用：
  - `node -> node [OnComplete]`
  这样的 self-edge

这意味着长期上不一定非要把节点硬分成：

- `Stable`
- `Transition`

更通用的做法是：

- 统一成 `MusicNode`
- 再用节点属性和边触发来表达行为差异

对当前实现的启发是：

- 当前把 `bridge_clip / stinger_clip` 放在 `TransitionRule` 上，是为了 `[2]` 的 MVP 尽快跑通
- 但长期上，更优雅的终态应是：
  - `bridge` 升格成节点
  - `stinger` 归入该节点的另一条 track
  - 边只保留转移条件

因此后续如果继续演进，推荐目标不是“继续把边上的字段堆得更复杂”，而是：

1. 引入中性的 `MusicNode`
2. 引入：
   - `externally_targetable`
   - `completion_source`
3. 把 `bridge` 迁移为自动转移节点
4. 把 `stinger` 迁移为该节点上的 `music_stinger` track 内容
5. 让 `Edge` 只负责：
   - `NextCue`
   - `Immediate`
   - `OnComplete`

一句话总结：

- 当前实现把 `bridge` 当作“边上的内容”是 MVP 捷径
- 长期更推荐：
  - **所有可播放内容都是节点**
  - **所有转移条件都放在边上**
  - **stinger 是节点内部的另一条 track，而不是额外特判**

### 阶段 2：Runtime / Backend Transport 基础（进行中）

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

当前结果：

- 已完成：
  - Firewheel feature 打开：`scheduled_events`、`musical_transport`
  - `sonara-firewheel` 已支持：
    - `instance_playhead()`
    - `seek_instance()`
    - `seek_instance_after()`
    - `audio_clock_seconds()`
    - 非立即 `stop(..., Fade::seconds(...))`
    - 非立即 `stop_music_session(..., Fade::seconds(...))`
  - `sonara-bevy` 已支持：
    - `instance_playhead_seconds()`
- 未完成：
  - `start_clip(asset_id, offset, loop_range)` 这一层明确 API
  - `Clip.source_range / loop_range` 真正下沉到 backend
  - `schedule_handoff(...)`
  - runtime 里的 `ClipCursor / TransportStatus` 明确对象化

### 阶段 3：实现功能 `[1]` 和 `[2]`

目标：

- 建立 `MusicSession` 和 `PendingTransition`
- 让 Sonara 自己接管“等待切点再切”的状态机

这一阶段解决：

- `[1]` 每个 `memory_slot` 的播放头记忆
- `[1]` 基于 `MemoryPolicy.ttl` 的过期重置
- `[2]` 源段等待下一个合法 `Exit Cue`
- `[2]` 可选 `bridge node`
- `[2]` 过渡结束后进入目标状态

建议运行时状态：

```text
Stable
WaitingExitCue
WaitingNodeCompletion
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
  - `request_music_node(...)`

当前结果：

- runtime 逻辑骨架已提前落地：
  - `play_music_graph(...)`
  - `request_music_node(...)`
  - `complete_music_exit(...)`
  - `complete_music_node_completion(...)`
  - `music_status(...)`
- 但这仍是逻辑状态机，不是端到端可听见的音乐切换
- 本阶段真正完成的标志，仍然是：
  - `MusicSession` 接到真实 backend
  - 功能 `[1]` 能真实恢复进度
  - 功能 `[2]` 能真实等待切点并完成 bridge handoff

补充：

- 上述 3 条 MVP 主线现已具备，并已有可试听 example：
  - `[1]`：`music_resume`
  - `[2]`：`music_cue_trigger`
- 本阶段剩余工作更偏执行质量：
  - 更精确的 cue handoff
  - 以参数自动化为基础重新落 transition fade/crossfade

### 阶段 4：实现功能 `[3]`

目标：

- 支持共享播放头下的多轨 / 轨组变体切换

这一阶段建议拆两步：

1. 节点内多轨 / 轨组切换
   - 同一节点内多条 `Track` 共享一根播放头
   - 不同配器、风格或层被组织成可切换的 `TrackGroup`
   - 切换优先通过：
     - mute / unmute
     - gain automation
     - layer enable / disable
     来完成，而不是优先切到另一条独立 clip
2. 结构同步变体
   - 当确实需要切到另一组独立内容时
   - 再使用同步域把当前播放头映射到目标内容的对应位置

核心对象：

- `SyncDomain`
- `TrackGroup`
- `GroupState`
- `GroupAutomation`
- `SyncVariantSet`
- `StemSet`

关键进入策略：

- `SameSyncPosition`

运行时需要新增：

- `shared_playhead`
  - 以节点 `primary_track` 的播放位置作为节点级共享播放头
- `TrackGroup` 激活状态
- 组级静音 / 权重 / 自动化执行
- `SyncCursor`
  - 只有在切到另一组独立内容时，才需要把共享播放头映射到目标变体

关于 `TrackGroup`，建议采用：

- 显式组只在需要联动切换多条 track 时创建
- 未分组的 `Track` 不需要额外建一个“默认组”对象
- 语义上把未分组 `Track` 视为一个**隐式单轨组**
  - 这样：
    - `Track` 仍然能被独立 mute / gain / automation
    - 系统内部又不需要区分“有组”和“没组”两套完全不同模型

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
