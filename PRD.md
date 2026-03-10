# Sonara v0 产品需求文档

## 1. 产品概述

### 1.1 产品名称

Sonara

### 1.2 产品定位

Sonara 是一个面向游戏的, Rust-first 的开源交互音频中间件

当前明确方向:

- Rust-first
- 以 Firewheel 作为执行后端
- 优先集成 Bevy
- 提供开源的 authoring + runtime + bank pipeline
- 首版编辑器采用 `egui`

### 1.3 产品目标

Sonara v0 的目标不是完整替代 FMOD/Wwise, 而是验证以下命题:

- Rust/Bevy 生态可以拥有一个真正可用的开源交互音频中间件
- 音频内容可以被创作, 编译, 运行和调试
- 程序员和音频师都能以合理的心智模型使用这套系统

## 2. 目标用户

### 2.1 程序员

目标用户:

- 使用 Rust 开发游戏的程序员
- 使用 Bevy 的游戏开发者
- 希望用更高层事件系统而不是直接播放音频资源的开发者

核心诉求:

- 能简单触发音频事件
- 能把声音绑定到实体
- 能更新全局参数和 emitter 参数
- 能显式加载和卸载 bank
- 能知道为什么一个事件没有按预期工作

### 2.2 音频师

目标用户:

- 需要为游戏组织交互音频内容的音频设计者
- 希望使用事件, 参数, bus 和 bank 进行内容创作的使用者

核心诉求:

- 能导入资源
- 能创建事件
- 能组织随机, 分层, 切换, 循环等结构
- 能试听
- 能构建 bank
- 能在运行中的游戏里验证效果

## 3. 要解决的问题

Rust 生态当前缺少成熟的, 开源的, 面向游戏的交互音频中间件

现状问题:

- 有可复用的音频底层或音频图引擎, 但缺少完整中间件层
- 程序员往往只能直接使用较低层的播放库
- 音频师缺少成体系的 authoring + bank + runtime 工作流
- Bevy 生态缺少清晰的事件驱动音频中间件方案

Sonara v0 要优先补上的不是底层播放器, 而是:

- `Event`
- `Parameter`
- `Bus`
- `Snapshot`
- `Bank`
- Bevy 侧工作流

## 4. 核心产品原则

### 4.1 程序员侧原则

程序员主要操作这些对象:

- `Event`
- `Parameter`
- `Bus`
- `Snapshot`
- `Bank`

程序员不应直接操作:

- 底层 `AudioAsset`
- 运行时图内部细节
- 后端执行细节

### 4.2 音频师侧原则

音频师的主工作流应尽量短:

1. 导入资源
2. 创建事件
3. 组织内容结构
4. 绑定参数
5. 试听
6. 构建 bank
7. 在游戏里验证

### 4.3 架构原则

- runtime 抽象高于后端实现
- Firewheel 是执行后端, 不是产品本体
- authoring 数据和运行时 bank 从一开始就分离
- 先做事件内容树, 不先做完全自由图

## 5. v0 范围

### 5.1 纳入范围

- 事件驱动的音频 authoring
- 事件内容树
- 参数系统
- bus 树
- 基础 snapshot
- 基础 2D / 3D 音频语义
- bank 构建与加载
- `egui` 编辑器
- Firewheel backend 适配层
- Bevy integration first
- 基础调试和试听能力

### 5.2 不纳入范围

- 音乐时间线系统
- 节拍同步切换
- 对白/语音系统
- 插件 SDK
- 多后端支持
- 网络协作
- 完整 DSP patching 编辑器
- 复杂行为 VM

## 6. 核心对象模型

### 6.1 Event

`Event` 是面向游戏逻辑的主音频行为定义

当前事件类型:

- `OneShot`
- `Persistent`

说明:

- `OneShot` 适合脚步, 爆炸, UI 音
- `Persistent` 适合环境声, 音乐

### 6.2 Event 内容树

Sonara v0 使用内容树, 不使用完全自由图

容器节点:

- `Random`
- `Sequence`
- `Layer`
- `Switch`
- `Loop`

叶子节点:

- `Sampler`

其中:

- `Switch` 是一种 container
- `Switch` 在 v0 中只由 `Enum` 参数驱动

### 6.3 Parameter

参数类型:

- `Float`
- `Bool`
- `Enum`

参数作用域:

- `Global`
- `Emitter`
- `EventInstance`

当前重点:

- `Global` 适合全局状态, 音乐状态
- `Emitter` 适合角色速度, 地面材质等
- `EventInstance` 为后续更细粒度控制预留

### 6.4 Bus

`Bus` 是混音层级单位

v0 关注:

- 树形层级
- 音量
- mute
- solo
- 基础 metering

### 6.5 Snapshot

`Snapshot` 是一组针对 bus 的混音覆盖

v0 只需支持:

- bus 目标音量
- fade in
- fade out

### 6.6 AudioAsset

`AudioAsset` 是导入后的底层音频资源

它主要服务:

- 构建
- bank
- runtime 播放计划

### 6.7 Bank

`Bank` 是运行时加载单元, 不是作者文件夹

Sonara v0 中 bank 的职责:

- 划定内容加载边界
- 引用事件
- 引用 resident media
- 引用 streaming media

### 6.8 Emitter

`Emitter` 是绑定到游戏实体上的发声点

在 Bevy 中应主要体现为:

- `AudioEmitter` component

## 7. 关键使用场景

### 7.1 脚步声

目标:

- 玩家走路时触发 `player.footstep`
- 根据地面材质选择不同声音
- 声音挂在实体上

需要的对象:

- `Event`
- `Switch`
- `Random`
- `Emitter Parameter`
- `Emitter`

### 7.2 爆炸声

目标:

- 播放一次爆炸事件
- 支持多层同时触发
- 支持随机变化

需要的对象:

- `Layer`
- `Random`
- `OneShot`

### 7.3 环境声

目标:

- 长时间持续播放
- 支持 loop
- 支持停止和淡出

需要的对象:

- `Persistent`
- `Loop`
- `Fade`

### 7.4 基础音乐切换

目标:

- 根据全局状态切换探索/战斗音乐
- 或在同一曲中切不同配器层

v0 做法:

- 用 `Persistent Event`
- 用全局参数驱动
- 用 layer/stem 混合
- 支持 fade / crossfade

v0 不做:

- timeline
- bar/beat 对齐

## 8. 程序员侧工作流

### 8.1 Bevy 心智模型

推荐工作流:

- startup system 初始化音频和加载 bank
- update system 触发事件和同步参数
- 发声实体挂 `AudioEmitter`
- 监听实体挂 `AudioListener`
- 通过 `Res<SonaraAudio>` 访问音频运行时

### 8.2 目标 API 形状

核心 API 应围绕这些操作:

- `load_bank`
- `unload_bank`
- `play`
- `play_on`
- `stop`
- `set_global_param`
- `set_emitter_param`
- `push_snapshot`

### 8.3 现阶段实现状态

当前仓库已经具备:

- 最小 bank 加载骨架
- 最小 runtime 事件解析器
- `play`
- `play_on`
- `EmitterId`
- `set_emitter_param`
- `PlaybackPlan`

当前还未具备:

- 真正音频输出
- Bevy ECS 实体桥接
- Firewheel one-shot 播放

## 9. 音频师侧工作流

### 9.1 目标工作流

1. 导入资源
2. 创建事件
3. 添加 `Switch` / `Random` / `Layer` 等容器
4. 绑定参数
5. 路由到 bus
6. 试听
7. 构建 bank
8. 在运行中的游戏中验证

### 9.2 编辑器首页目标

建议布局:

- 左侧项目树
- 中间主编辑区
- 右侧 inspector
- 底部 preview / diagnostics

### 9.3 脚步声示例结构

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

## 10. 技术路线

### 10.1 总体路线

当前采用的实现路线是:

1. 先稳定 `model` 和 `build`
2. 再做纯内存 `runtime` 解释层
3. 再做 Bevy 接口层
4. 最后把 Firewheel 接成真实音频输出

这条路线的目的:

- 先验证中间件抽象
- 再接真实播放后端
- 避免项目过早滑向“普通音频库接入”

### 10.2 当前仓库状态

当前已经完成:

- workspace 骨架
- 中文架构草案
- 核心模型
- 构建层最小校验
- bank 基础组装
- runtime 最小事件解析器
- emitter 参数作用域
- `play_on` 路径
- 一个最小控制台 demo

## 11. 当前可见效果

当前运行:

```bash
cargo run -p sonara-app
```

可以看到一版最小效果:

- 创建一个脚步事件
- 创建一个 emitter
- 给 emitter 设置 `surface = stone`
- 调用 `play_on`
- 输出解析到的资源 ID

这证明:

- bank -> runtime -> emitter -> event resolution 这条链已经开始工作

## 12. 成功标准

Sonara v0 的阶段性成功标准:

- 能表达脚步声, 爆炸声, 环境声这三类典型场景
- 能完成 authoring -> build -> runtime 的最小闭环
- 程序员能在 Bevy 风格接口下使用它
- 音频师的核心概念模型已经稳定
- 已具备接 Firewheel 做真实 one-shot 播放的前置条件

## 13. 下一阶段建议

优先级建议:

1. 接 Firewheel 做真实 one-shot 播放
2. 让 `play_on` 真正播放 `PlaybackPlan.asset_ids`
3. 建立更像 authoring 输入的数据结构
4. 再逐步推进 editor

## 14. 当前仓库中的关键文件

- [ARCHITECTURE.md](./ARCHITECTURE.md)
- [PRD.md](./PRD.md)
- [README.md](./README.md)
- [sonara-model](./sonara-model)
- [sonara-build](./sonara-build)
- [sonara-runtime](./sonara-runtime)
- [sonara-bevy](./sonara-bevy)
- [sonara-app](./sonara-app)
