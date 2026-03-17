# Sonara Bus / Effect / Automation 设计记录

## 1. 目标

这份文档记录 Sonara 在 `Bus + Effect + Automation` 主线上的当前设计结论。

这条主线的目标不是重写整个音频后端，而是按现有 `Event + MusicGraph + Runtime + Firewheel + Bevy` 基线，补出一层可持续演进的实时混音控制基础设施。

当前优先目标：

- 让 `Bus` 从“起播时读取一次的音量表”升级为真实的 live control layer
- 让 `Snapshot` 真正驱动 live bus gain
- 为 `Effect` 建立以 `Bus` 为归属的链式结构
- 为 `Automation` 建立统一的参数时间变化抽象

当前非目标：

- 不回退 `MusicGraph + MusicNode + MusicEdge + Track + TrackGroup` 主模型
- 不优先修 `[3]` `music_track_groups` 的播放行为问题
- 不把 `fade` 重新设计成 `Track` 的静态属性
- 不先做 per-instance effect chain
- 不一次性做完整 timeline 级 automation authoring 系统

## 2. 当前基线

结合当前仓库真实实现，可以得到以下结论：

- `Bus` 在模型层已有最小定义：
  - `id`
  - `name`
  - `parent`
  - `default_volume`
- `Event` 已有 `default_bus`
- `Snapshot` 已有 `targets: Vec<SnapshotTarget>`
- `SonaraRuntime` 已有：
  - `bus_volumes`
  - `load_snapshot(...)`
  - `push_snapshot(...)`
  - `bus_volume(...)`
  - `active_bus_volume(...)`
- `FirewheelBackend` 当前普通事件起播时会读取一次 `active_bus_volume(instance_id)`

但当前实现仍然不是完整 live bus 系统：

- event worker 只是起播时读取一次 bus 音量
- bus 变化不会持续作用到已在播 event worker
- music worker 当前没有真正的 bus 路由
- `push_snapshot(...)` 更像“改 runtime 里的 bus 表”，而不是完整驱动 live worker

从真实输出路径看，当前在播 `event` 和 `music` 最终都直接进入 Firewheel 的全局输出节点，而不是先进入 Sonara 自己的 live bus 节点。

## 3. 核心边界

这条主线按以下边界推进：

### 3.1 Track / TrackGroup

`Track` / `TrackGroup` 负责：

- 哪些内容在播
- 哪些版本/层被启用

`Track` / `TrackGroup` 不负责：

- 长期混音控制
- 统一效果器链
- 长期参数自动化

### 3.2 Bus

`Bus` 负责：

- 声音最终往哪混
- 在哪一层做统一 gain 控制
- 在哪一层挂载统一 effect chain
- 在哪一层承接 snapshot / automation

### 3.3 Effect

`Effect` 第一阶段只归属于 `Bus`。

当前不优先做：

- 每个 event instance 自己的一条 effect chain
- 每个 music track instance 自己的一条 effect chain

### 3.4 Automation

`Automation` 是长期统一控制层。

它最终承载：

- bus gain automation
- snapshot fade
- effect parameter automation
- 未来的 track group gain automation

## 4. Bus 的新定义

### 4.1 当前 Bus 是什么

当前 `Bus` 更接近：

- 一个静态对象定义
- 一张 runtime 音量表上的键
- event 起播时读取一次的参数来源

它还不是完整意义上的 live mix control layer。

### 4.2 目标中的 Bus 是什么

新版本 `Bus` 应被理解为：

- 一个长期存在的混音控制节点
- 一个统一管理 gain / effect / automation 的挂点
- 一个所有 event / music 最终都应归属的实时控制层

可以把它理解成“调音台上的总线/编组”，而不是“起播前读一次的配置项”。

### 4.3 新版本 Bus 自己拥有的数据

Bus 的静态定义层至少应包含：

- `id`
- `name`
- `parent` 或明确输出目标
- `default_gain`
- `effect_chain`

Bus 的运行时状态层至少应包含：

- 当前 live gain / target gain
- 当前生效的 snapshot 结果
- 当前生效的 automation 状态
- 当前附着在该 bus 上的 live worker 关系

## 5. Snapshot 的定义

### 5.1 Snapshot 是什么

`Snapshot` 应被理解为：

- 一个混音场景预设
- 一组面向多个 bus 的目标参数集合
- 一个高层的“我要切到哪种混音状态”的表达

最直观的比喻是：

- `Bus` 是调音台上的推子
- `Snapshot` 是一组推子位置预设

例如：

- 常态：
  - `Music Bus = 0.7`
  - `SFX Bus = 0.8`
- 战斗 snapshot：
  - `Music Bus = 1.0`
  - `SFX Bus = 0.9`

当调用 `push_snapshot(...)` 时，本质上是在说：

- 把当前混音推到这个场景去

### 5.2 Snapshot 不是什么

`Snapshot` 不是：

- 音频片段
- 播放头快照
- “当前播到哪里”的存档
- 决定“播什么”的对象

`Snapshot` 只负责：

- 决定已经在播的声音现在应该如何整体混合

它不负责：

- 改变播放内容本身
- 决定 event / music graph 当前选中了哪条内容

### 5.3 Snapshot 与 Bus 的关系

`Snapshot` 不直接替代 `Bus`。

两者关系是：

- `Bus` 是长期存在的控制点
- `Snapshot` 是一组要作用到这些控制点上的场景目标

换句话说：

- `Bus` 是“旋钮本身”
- `Snapshot` 是“一组旋钮位置预设”

### 5.4 Snapshot 与 Automation 的关系

`Snapshot` 和 `Automation` 很接近，但层级不同。

可以用下面这组对照理解：

- `Snapshot`：我要切到哪个混音状态
- `Automation`：这些参数怎样随时间变化到位

因此更合理的系统关系是：

- `Snapshot` 提供一组目标参数
- `Automation` 负责把这些参数在时间上推到目标值

在 Sonara 的目标实现里，`Snapshot` 应当是 `Automation` 的高层来源之一。

也就是说：

- `push_snapshot(combat)`
- 生成若干条 `BusGain(...)` / `BusEffectParam(...)` automation
- backend 按音频时钟执行这些 automation

## 6. Effect 的定义

### 6.1 归属关系

第一阶段 effect 归属于 `Bus`，而不是归属于单个播放实例。

### 6.2 链式结构

Effect 应被设计成有序链，而不是单个孤立插槽。

信号流抽象为：

`Bus 输入汇总 -> EffectSlot1 -> EffectSlot2 -> EffectSlot3 -> ... -> Bus 输出 -> Parent / Master`

这里“输出”不是一个单独 effect 对象，而是每个 slot 的输出自然进入下一个 slot，最后离开 bus。

### 6.3 模型层与执行层分工

模型层不把 `Effect` 主体设计成 trait。

原因：

- 模型层需要稳定序列化
- bank 需要可持久化的对象定义
- 编辑器需要可枚举、可检查的参数结构

因此模型层更适合使用数据对象，例如：

- `EffectId`
- `EffectKind`
- `EffectDefinition`
- `BusEffectSlot`

而 backend 执行层可以内部使用 trait 或 adapter，用来：

- 创建 backend effect 实例
- 更新 effect 参数
- 把 effect 挂进 bus chain

### 6.4 第一批基础 Effect 方向

优先考虑：

- `Gain`
- `Pan`
- `LowPass`
- `HighPass`
- `BandPass` 或 `SVF`
- `Freeverb`

暂不优先承诺：

- `EQ`
- `Compressor`
- `Limiter`
- `Distortion`

## 7. Automation 的定义

### 7.1 基本定义

`Automation` 定义为：

一个作用于可寻址参数目标的、在有限时间域上定义的分段值函数。

可以把它理解为：

- 时间域：`t in [t0, t1]`
- 值函数：`v = f(t)`
- 目标：`target`

### 7.2 目标不是“对象”，而是“参数”

Automation 的目标应当是可寻址参数，而不是一个整个对象。

例如：

- `BusGain(bus_id)`
- `BusEffectParam { bus_id, slot_id, param }`

这样可以避免“一个 automation 同时修改一组不相干属性”的混乱设计。

### 7.3 第一阶段表达方式

第一阶段不要求直接用复杂控制点系统表达。

更适合先落成最小可执行的分段抽象：

- `target`
- `start_time`
- `duration`
- `from`
- `to`
- `curve`

也就是说，先优先支持“分段 automation”，而不是完整 DAW 级曲线编辑器。

### 7.4 第一阶段曲线类型

建议先支持：

- `Step`
- `Linear`

后续可扩：

- `EqualPower`
- `EaseInOut`

### 7.5 Snapshot 与 Automation 的关系

`Snapshot` 不是 `Automation` 的替代物。

更准确地说：

- `Snapshot` 是高层 authoring 对象
- `Automation` 是底层执行语义

也就是说，snapshot 最终不应靠一堆独立特判驱动，而是应被翻译成对以下目标的一组 automation：

- `BusGain(...)`
- `BusEffectParam(...)`

### 7.6 第一阶段冲突规则

第一阶段先采用简单规则：

- 同一个 target 上，后来的 automation 覆盖前面的 automation

当前不优先做：

- 多层 automation 混合
- 权重叠加
- 复杂 layer/blend stack

## 8. Runtime / Backend 分工

### 8.1 Runtime

runtime 负责：

- 持有 bus / snapshot / effect / automation 的高层语义状态
- 把 snapshot / set bus gain / set effect param 等高层语义翻译成 automation plan
- 提供 event / music 当前归属 bus 的解析结果

### 8.2 Backend

backend 负责：

- 维护 live worker 与 bus 的映射
- 在音频时钟上执行 gain / effect param 的变化
- 把 bus live 状态真正作用到已在播 event / music worker

backend 不负责理解高层业务语义，例如：

- “战斗 snapshot”
- “探索状态”
- “某个音乐节点的外部意图”

这些都应先在 runtime 层被翻译成可执行控制计划。

## 9. 实施顺序

### 第一阶段：live bus gain

目标：

- 让 bus 变成真实 live 控制层
- bus gain 变化能作用到已在播 event / music worker

要求：

- event 继续使用 `default_bus`
- music 采用最小 bus 接线方案
- 不急着先接 effect

### 第二阶段：snapshot -> live bus gain

目标：

- 让 snapshot 真正驱动 live bus gain

第一版允许：

- 先直接把目标 gain 作用到 bus
- fade 只做最小实现

### 第三阶段：bus effect foundation

目标：

- effect 先挂在 bus 上
- 不急着做 per-instance effect chain

重点：

- 建立 bus 和 effect chain 的关系
- 不要求一次做很多效果器品类

### 第四阶段：automation foundation

目标：

- 把 bus gain / snapshot fade / effect param 变化收敛到统一方向

这次不要求：

- 一次做完整 automation 系统

但要求避免继续新增大量彼此无关的 fade 特判。

## 10. 对象清单与最小字段草案

本节不是最终公开 API，只是当前阶段指导实现的最小数据骨架。

这里的字段命名表达的是设计意图：

- 若现有代码中已有相近字段，例如 `default_volume`
- 实现时可以先做兼容或渐进迁移
- 不要求文档字段名与第一次提交的代码命名完全一致

### 10.1 模型层对象

#### Bus

```text
Bus
- id: BusId
- name: SmolStr
- parent: Option<BusId>
- default_gain: f32
- effect_slots: Vec<BusEffectSlot>
```

说明：

- `default_gain` 表达 bus 的默认静态增益
- `effect_slots` 的顺序就是链顺序
- 当前代码中的 `default_volume` 可视为这里的 `default_gain`

#### Music Track 路由

第一阶段推荐的最小接线方案：

```text
Track
- id: TrackId
- name: SmolStr
- role: TrackRole
- group: Option<TrackGroupId>
- output_bus: Option<BusId>
```

说明：

- `Track` / `TrackGroup` 仍然只负责“播什么”
- `output_bus` 只负责“这一条内容最终混到哪个 bus”
- `None` 表示使用默认总线策略

#### EffectDefinition

```text
EffectDefinition
- id: EffectId
- name: SmolStr
- kind: EffectKind
- default_params: Vec<EffectParamValue>
```

说明：

- `EffectDefinition` 描述一个可复用 effect 对象
- `default_params` 第一阶段只要求支持标量参数

#### EffectKind

第一阶段建议的最小枚举：

```text
EffectKind
- Gain
- Pan
- LowPass
- HighPass
- BandPass
- Svf
- Freeverb
```

#### BusEffectSlot

```text
BusEffectSlot
- id: BusEffectSlotId
- effect_id: EffectId
- bypass: bool
- wet: f32
```

说明：

- slot 顺序由 `Bus.effect_slots` 的数组顺序决定
- `wet` 第一阶段可统一理解为 wet/mix 量
- 若某类 effect 暂不使用 `wet`，可以先固定为 `1.0`

#### Snapshot

第一阶段保持 snapshot 仍然是高层“混音场景”对象：

```text
Snapshot
- id: SnapshotId
- name: SmolStr
- fade_in_seconds: f32
- fade_out_seconds: f32
- targets: Vec<SnapshotTarget>
```

#### SnapshotTarget

推荐的可扩展方向：

```text
SnapshotTarget
- BusGain { bus_id: BusId, target_gain: f32 }
- BusEffectParam {
    bus_id: BusId,
    slot_id: BusEffectSlotId,
    param: EffectParamId,
    target_value: f32,
  }
```

说明：

- 第一阶段至少落地 `BusGain`
- `BusEffectParam` 可以先作为 foundation 预留，不要求第一次就接上真实执行

#### AutomationTarget

```text
AutomationTarget
- BusGain(BusId)
- BusEffectParam {
    bus_id: BusId,
    slot_id: BusEffectSlotId,
    param: EffectParamId,
  }
```

#### AutomationCurve

```text
AutomationCurve
- Step
- Linear
- EqualPower
- EaseInOut
```

说明：

- 第一阶段只要求真正支持 `Step` 和 `Linear`
- 其余类型允许先存在于模型草案中

#### AutomationSegment

```text
AutomationSegment
- target: AutomationTarget
- start_time_seconds: f64
- duration_seconds: f32
- from: f32
- to: f32
- curve: AutomationCurve
```

说明：

- 第一阶段先按“单段、单目标、单标量参数”实现
- 更复杂的控制点系统属于后续 authoring 表达增强

### 10.2 Runtime 层对象

#### LiveBusState

```text
LiveBusState
- bus_id: BusId
- target_gain: f32
- current_gain: f32
- snapshot_gain: Option<f32>
- dirty: bool
```

说明：

- `target_gain` 表示当前 bus 应该到达的值
- `current_gain` 表示 runtime/backend 已知的当前 live 值
- `snapshot_gain` 表示最近一层 snapshot 贡献的结果
- `dirty` 用于告知 backend 这一 bus 需要同步到 live worker

#### ActiveAutomation

```text
ActiveAutomation
- id: AutomationId
- target: AutomationTarget
- start_time_seconds: f64
- duration_seconds: f32
- from: f32
- to: f32
- curve: AutomationCurve
```

说明：

- 第一阶段同一 target 上后来的 automation 直接覆盖前面的 automation
- runtime 负责创建和替换这些对象

### 10.3 Backend 层对象

#### WorkerBusBinding

```text
WorkerBusBinding
- worker_id: WorkerID
- bus_id: BusId
```

说明：

- event worker 和 music worker 都需要能映射到一个 bus
- 这是 bus 变成真实 live control layer 的最小前提

#### BusWorkerSet

```text
BusWorkerSet
- bus_id: BusId
- workers: Vec<WorkerID>
```

说明：

- backend 需要能按 bus 找到当前所有 live worker
- 这样 bus gain / effect param 变化才能推到已经在播的声音上

#### LiveBusEffectState

```text
LiveBusEffectState
- bus_id: BusId
- slot_id: BusEffectSlotId
- effect_id: EffectId
- current_params: Vec<EffectParamValue>
```

说明：

- 第一阶段只要求这层 foundation 存在
- 是否第一次就接入完整 DSP chain，可以按实现进度决定

## 11. 当前记录下来的决定

截至本文档编写时，已明确的结论包括：

- `Bus` 必须升级为真实 live control layer
- `Snapshot` 是混音场景预设，而不是播放内容或播放头快照
- `Event` 和 `Music` 最终都应归属到 `Bus`
- `Track / TrackGroup` 不负责长期混音控制
- `Effect` 第一阶段只归属于 `Bus`
- `Effect` 在模型层使用数据定义，不以 trait 作为公开模型中心
- `Automation` 是统一的参数时间变化抽象
- `Snapshot` 是 `Automation` 的高层来源之一
- 当前实施顺序固定为：
  - `live bus gain`
  - `snapshot -> live bus gain`
  - `bus effect foundation`
  - `automation foundation`

## 12. 待下一次继续细化的问题

本文档暂不展开以下细节，后续实现前需要继续细化：

- bus parent 层级 gain 是否与第一阶段一起接入
- effect 参数类型系统如何定义
- runtime 中 live bus state 的精确数据结构
- backend 如何组织 bus effect chain 与 live worker 的连接方式
