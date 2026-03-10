# Sonara

Sonara 是一个面向游戏的、Rust-first 的开源交互音频中间件。

当前方向：

- Rust-first
- Firewheel backend
- Bevy integration first
- 开源 authoring + runtime + bank pipeline
- 首版编辑器使用 `egui`

## 当前状态

当前仓库已经打通一条最小可运行主线：

- `build_bank(...)` 构建 bank
- `sonara-runtime` 解析事件和参数
- `sonara-firewheel` 加载真实 wav 资源并实际播放
- `sonara-bevy` 已接入真实 `bevy_app` / `bevy_ecs`
- Bevy 路线已经能触发播放和 stop

目前已经具备的能力：

- 已建立 Cargo workspace
- 已建立核心 crate 骨架和第一版对象模型
- 已有最小 bank build / load 流程
- 已有 emitter 参数驱动的事件解析
- 已有 Firewheel one-shot 播放路径
- 已有最小实例 stop 和生命周期同步
- 已有最小 Bevy plugin / component / non-send audio resource 接入

## 仓库结构

- `sonara-model`
  - 核心对象模型
- `sonara-build`
  - bank 构建和最小校验
- `sonara-runtime`
  - 高层运行时和事件解析
- `sonara-firewheel`
  - Firewheel 后端适配层
- `sonara-bevy`
  - Bevy 集成层
- `sonara-editor`
  - 编辑器 crate 骨架
- `sonara-app`
  - 当前的 Firewheel/backend demo

## 运行示例

直接验证真实 Firewheel 播放：

```bash
cargo run -p sonara-app
```

这个 demo 会：

- 从 `sonara-app/assets/demo/project.json` 读取最小 authoring project
- 编译其中的 `core` bank
- 加载仓库内 wav 资源
- 创建 emitter
- 设置 `surface = stone`
- 播放 one-shot
- 再通过 `queue_stop(...)` 主动停止实例

验证真实 Bevy ECS + Firewheel 路径：

```bash
cargo run --example minimal_bevy -p sonara-bevy
```

这个 example 会：

- 启动真实 `bevy_app::App`
- 加载 `SonaraFirewheelPlugin`
- 在 startup system 中加载 bank
- 在 update system 中通过 `NonSendMut<SonaraAudio>` 设参、播放、stop

注意：

- `sonara-app` 当前是 backend demo，不是最终应用形态。
- `sonara-bevy` 里的音频资源当前是 Bevy non-send resource，因为 Firewheel backend 不是 `Send + Sync`。

## 文档

- 设计和分层说明见 [ARCHITECTURE.md](./ARCHITECTURE.md)
- 产品目标和范围见 [PRD.md](./PRD.md)
- 项目概览见 [docs/overview.md](./docs/overview.md)
- 术语表见 [docs/concepts/glossary.md](./docs/concepts/glossary.md)
- authoring 与 bank 概念见 [docs/concepts/authoring-and-bank.md](./docs/concepts/authoring-and-bank.md)
- runtime 与 backend 边界见 [docs/architecture/runtime-and-backend.md](./docs/architecture/runtime-and-backend.md)

## 当前阶段判断

Sonara 还没有到 editor / authoring 完整可用阶段，但已经不只是模型草图。
当前仓库更适合被理解为：

- 一条已经能跑通的最小中间件主线
- 一个正在快速收敛的 Firewheel backend
- 一个已经进入真实 Bevy 集成阶段的 runtime 层
