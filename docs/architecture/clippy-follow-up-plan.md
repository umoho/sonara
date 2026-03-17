# Sonara Clippy 基线与跟进计划

## 1. 背景

这份文档记录一次针对当前工作区的 `cargo clippy` 基线检查结果，并明确后续处理策略。

检查命令：

```bash
cargo clippy --workspace --all-targets
```

检查日期：

- `2026-03-17`

本次检查的核心结论不是“所有 crate 都已进入可统一收口的 clippy 清理阶段”，而是：

- workspace 级 `clippy` 目前不能作为硬门禁
- 主要阻塞点来自 `sonara-editor`
- 当前研发重心仍应放在 `sonara-runtime`
- `editor` 需要等 `runtime / build` 相关命名与边界稳定后再集中跟进

## 2. 当前结论

当前 `cargo clippy --workspace --all-targets` 失败，不是因为模块拆分本身破坏了运行时行为，而是因为 `sonara-editor` 仍然跟着旧的错误命名工作。

也就是说，当前真实状态更接近：

- `runtime / build / firewheel / bevy` 主线可以继续推进
- `editor` 对 `BuildError` 的文案映射落后于当前模型命名
- 在 `editor` 专门跟进之前，workspace 级 `clippy` 不应阻塞 runtime 主线工作

## 3. 当前阻塞项

当前导致 workspace 级 `clippy` 直接失败的阻塞项是：

- `sonara-editor/src/lib.rs:1022`
  - 仍在匹配 `BuildError::DuplicateMusicStateId`
  - 当前实际枚举名已经是 `DuplicateMusicNodeId`
- `sonara-editor/src/lib.rs:1023`
  - 仍在匹配 `BuildError::MissingMusicStateDefinition`
  - 当前错误语义已经改成 `MusicNode` 语境，不再有这个旧变体

这说明：

- `editor` 的诊断文案层还保留着旧的 “music state” 命名
- 这属于 `editor` 跟进问题，不应在当前 runtime 收敛阶段强行混入处理

## 4. 当前阶段的处理策略

在 `runtime` 仍处于主线建设阶段时，建议采用“按活跃 crate 跑 clippy”的策略，而不是要求 workspace 一次性全绿。

建议门禁方式：

- 只改 `runtime` 时：

```bash
cargo clippy -p sonara-runtime --all-targets
```

- 改到 runtime 联动 backend / bevy 时：

```bash
cargo clippy -p sonara-runtime -p sonara-firewheel -p sonara-bevy --all-targets
```

- 改到 build 时：

```bash
cargo clippy -p sonara-build --all-targets
```

在 `editor` 没有完成版本跟进前，不要求：

```bash
cargo clippy --workspace --all-targets
```

必须全绿。

## 5. 当前值得处理的 clippy 项

### 5.1 runtime

`sonara-runtime` 当前值得优先处理的是低风险机械项：

- `needless_borrow`
- `collapsible_if`

这些 warning 主要出现在：

- `sonara-runtime/src/music.rs`

此外还有两处 `too_many_arguments`：

- `sonara-runtime/src/bank.rs`
- `sonara-runtime/src/commands.rs`

对这两处的建议是：

- 暂时记录，不作为当前清理目标
- 它们对应的是当前刻意保持稳定的 facade API
- 如果未来要改，应当在明确引入新的 bank payload / load args 类型时一起处理

## 6. 相邻 crate 的 warning 状态

### 6.1 sonara-build

当前以机械整理为主：

- `sonara-build/src/media.rs`
  - `needless_borrow`
- `sonara-build/src/validate.rs`
  - 多处 `collapsible_if`
- `sonara-build/src/tests.rs`
  - `cloned_ref_to_slice_refs`

这些都属于低风险清理项。

### 6.2 sonara-firewheel

当前主要是：

- `sonara-firewheel/src/music.rs`
  - `if_same_then_else`
  - 多处 `collapsible_if`
- `sonara-firewheel/src/workers.rs`
  - 多处 `collapsible_if`
- `sonara-firewheel/src/assets.rs`
  - `too_many_arguments`

其中 `assets.rs` 的参数过多问题本质上跟随 runtime facade，一样建议后移处理。

### 6.3 sonara-bevy

当前主要是：

- `sonara-bevy/src/audio.rs`
  - `too_many_arguments`
  - `large_enum_variant`

这里也不建议在当前阶段为了 clippy 立刻改 API 形状或改 enum 内存布局。

### 6.4 sonara-model

当前主要是：

- `derivable_impls`

这些属于很容易批量处理的整理项，但不属于当前 runtime 主线工作的最高优先级。

## 7. 后续执行顺序

建议按下面顺序处理：

1. 继续优先推进 `sonara-runtime` 主线与其直接相关模块
2. 在活跃 crate 上保持 targeted clippy 尽量干净
3. 暂不为了 workspace 级 clippy 去提前修改 `sonara-editor`
4. 等 `runtime / build` 命名和边界稳定后，再单独做一轮 `editor` 跟进
5. `editor` 跟进完成后，再恢复对 `cargo clippy --workspace --all-targets` 的全局要求

## 8. 文档用途

这份文档的作用是给当前阶段定一个清晰边界：

- 现在不是 “所有 crate 一起做风格统一清理” 的阶段
- 现在是 “runtime 主线优先，editor 延后同步” 的阶段
- workspace 级 `clippy` 红灯在当前语境下是已知状态，不应被误解为 runtime 主线回归
