# Sonara 概览

Sonara 是一个面向游戏的、Rust-first 的开源交互音频中间件。

当前目标不是完整替代 FMOD/Wwise，而是先证明一条最小但真实可运行的主线：

- 音频内容可以被定义
- 内容可以被构建成 bank
- 运行时可以解析事件和参数
- backend 可以把解析结果变成真实声音
- Bevy 集成路径可用

## 当前主线

当前仓库已经打通这条链路：

`authoring 数据模型 -> build -> compiled bank -> runtime -> firewheel backend -> bevy 集成`

其中：

- authoring 层负责表达“内容怎么被创作和组织”
- build 层负责把 authoring 数据构造成 compiled bank
- runtime 负责“事件如何被解析成播放计划”
- backend 负责“播放计划如何变成真实声音”
- bevy 集成层负责“如何在 ECS/system 里触发 Sonara”

## 当前 crate 分工

### `sonara-model`

核心对象模型层，只定义数据和语义，不负责执行。

主要对象：

- `AuthoringProject`
- `BankDefinition`
- `Bank`
- `BankObjects`
- `BankManifest`
- `Event`
- `Parameter`
- `Bus`
- `Snapshot`
- `AudioAsset`

### `sonara-build`

构建层，负责校验事件内容树，并把 authoring 数据构造成 compiled bank。

当前关键入口：

- `validate_event(...)`
- `build_bank(...)`
- `build_bank_from_definition(...)`
- `compile_bank_definition(...)`
- `compile_bank_definition_to_file(...)`
- `compile_project_bank_file_to_file(...)`

### `sonara-runtime`

高层运行时，负责：

- `load_bank`
- `play`
- `play_on`
- `stop`
- emitter / 参数管理
- 事件树解析
- `PlaybackPlan` 生成

runtime 关心的是“该播什么”，不直接负责真实音频输出。

### `sonara-firewheel`

Firewheel backend 适配层，负责：

- 启动真实输出流
- 从 compiled `BankManifest` 注册资源
- 解码 wav
- 消费 `PlaybackPlan.asset_ids`
- 创建真实 worker 播放
- stop 和 worker 生命周期同步

### `sonara-bevy`

Bevy 集成层，负责：

- 插件入口
- `SonaraAudio`
- `AudioEmitter`
- ECS/system 调用方式

它是集成层，不是中间件主线本体。

## 当前可运行入口

### backend 直连 demo

```bash
cargo run -p sonara-app
```

用于验证：

- build -> runtime -> firewheel
- 真实资源加载
- one-shot 播放
- stop

### 最小 Bevy 集成 demo

```bash
cargo run --example minimal_bevy -p sonara-bevy
```

用于验证：

- `App + SonaraFirewheelPlugin`
- `NonSendMut<SonaraAudio>`
- ECS system 中设参、播放、stop

### 交互式 3D 表面脚步 demo

```bash
cargo run --example surface_walk -p sonara-bevy
```

用于验证：

- 真实 Bevy 渲染
- 键盘控制
- emitter 参数驱动的表面脚步解析
- Firewheel 实际出声

## 当前还不完整的能力

以下能力还没有形成完整主线：

- snapshot 的运行时行为
- bus/mixer 真正生效
- persistent event 生命周期
- 非 immediate fade
- 完整 editor / authoring 工具链

## 建议阅读顺序

第一次读仓库，建议按这个顺序：

1. `README.md`
2. `ARCHITECTURE.md`
3. `PRD.md`
4. `docs/concepts/glossary.md`
5. `docs/concepts/authoring-and-bank.md`
6. `docs/architecture/runtime-and-backend.md`
