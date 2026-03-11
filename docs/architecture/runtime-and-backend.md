# Runtime 与 Backend 边界

这篇文档解释 Sonara 当前主线里最重要的一条边界：

- `sonara-runtime` 负责什么
- `sonara-firewheel` 负责什么
- 两者如何配合

## 最短结论

runtime 负责：

- 接收高层请求
- 管理参数和 emitter
- 解析事件内容树
- 生成 `PlaybackPlan`
- 管理活动实例

backend 负责：

- 加载媒体资源
- 把 `PlaybackPlan` 变成真实播放
- 管理播放对象生命周期
- 和底层音频系统交互
- 推进实例播放状态

一句话：

`runtime` 决定“该播什么”  
`backend` 决定“怎么真的播出来”

当前 v0 阶段还有一个中间层需要一起理解:

- `CompiledBankPackage`

它是 build 层导出的当前运行时加载载荷, 不是 editor 工程文件, 也不是最终固定标准。
在当前实现里:

- runtime 主要消费:
  - `bank.objects`
  - `events`
  - `buses`
  - `snapshots`
- backend 主要消费:
  - `bank.manifest`

## runtime 当前负责的事

`sonara-runtime` 当前已经负责：

- `load_bank`
- `unload_bank`
- `play`
- `play_on`
- `stop`
- `create_emitter`
- `set_global_param`
- `set_emitter_param`
- `PlaybackPlan` 生成
- `RuntimeRequest` / `RuntimeRequestResult`
- `QueuedRuntime`

在 bank 结构上，runtime 当前只保留：

- `BankObjects`

也就是说，runtime 当前不再持有媒体 manifest。

## backend 当前负责的事

`sonara-firewheel` 当前已经负责：

- 启动 Firewheel/CPAL 输出流
- 从 `BankManifest` 注册资源
- 准备 resident 资源
- 后台预热 streaming 资源
- 管理 sample resource
- 消费 `PlaybackPlan.asset_ids`
- 创建 Firewheel worker
- 挂起依赖未就绪 streaming 资源的播放
- 在资源就绪后自动启动挂起播放
- stop 与 worker 生命周期同步

也就是说，backend 当前真正关心的是：

- `BankManifest`
- `BankAsset`
- `PlaybackPlan.asset_ids`

当前资源策略是：

- `resident_media`
  - 在加载 compiled bank 时准备好
- `streaming_media`
  - 不在 startup 阶段同步整包解码
  - 由 backend 在后台预热
  - 如果播放时还没准备好，对应实例先进入 `PendingMedia`
  - 等资源就绪后，backend 在后续 `update()` 中自动启动

## 典型执行路径

以一次 `play_on(emitter_id, event_id)` 为例：

1. 调用进入 runtime
2. runtime 读取 emitter/global 参数
3. runtime 解析事件内容树
4. runtime 生成 `PlaybackPlan`
5. runtime 创建活动实例
6. backend 读取 `PlaybackPlan.asset_ids`
7. backend 找到已注册媒体
8. backend 创建真实播放对象
9. 声音输出到设备

如果某次播放依赖 streaming 资源且媒体尚未就绪，则路径会变成：

1. runtime 仍然生成 `PlaybackPlan`
2. backend 发现对应 streaming 资源未就绪
3. 实例状态进入 `PendingMedia`
4. backend 后台预热资源
5. 后续 `update()` 中资源完成注册
6. backend 自动启动先前挂起的实例
7. 实例状态推进到 `Playing`

## 为什么这个边界重要

如果 runtime 同时持有媒体 manifest，并且 backend 继续依赖 authoring 语义：

- runtime 会知道太多不该知道的底层信息
- backend 会对 authoring 模型耦合过深
- 后面加新功能时，很容易把逻辑堆到集成层或 demo 层

当前边界收紧之后，后续功能更容易放对位置：

- 事件解析、参数、实例行为
  - 优先加在 `sonara-runtime`
- 资源准备、播放、fade、worker 生命周期
  - 优先加在 `sonara-firewheel`
- ECS 触发方式
  - 放在 `sonara-bevy`

当前对外可查询的实例状态包括：

- `PendingMedia`
- `Playing`
- `Stopped`

这层状态由 runtime 定义，由 backend 推进，Bevy 集成层只负责转发给游戏侧查询。

## 当前还有哪些能力没补完

这条边界虽然已经清楚很多，但还有几块没完全长出来：

- snapshot 的运行时语义
- bus/mixer 真正影响播放
- persistent event 生命周期
- 更完整的 fade 支持

这些能力下一步应该优先落在 runtime/backend 主线，而不是先继续扩展示例。 
