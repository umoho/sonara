# Authoring 与 Bank

这篇文档解释 Sonara 里几个容易混淆的概念：

- `AuthoringProject`
- `BankDefinition`
- compiled `Bank`
- `BankObjects`
- `BankManifest`

## authoring 是什么

`authoring` 指的是“内容创作和组织”这一层。

它关注的是：

- 导入哪些音频资源
- 创建哪些事件
- 参数、bus、snapshot 怎么定义
- 哪些对象应该被编进哪个 bank

它不直接关心：

- 运行时如何触发
- 事件这一次具体解析出什么
- backend 如何真正出声

## `AuthoringProject`

`AuthoringProject` 是项目级 authoring 根对象。

当前它承载的是项目里被编辑和维护的内容集合：

- `assets`
- `parameters`
- `buses`
- `snapshots`
- `events`
- `banks`

你可以把它理解成“音频工程文件在内存中的结构”。

## `BankDefinition`

`BankDefinition` 属于 authoring 层。

它描述的是：

- 这个 bank 叫什么
- 哪些 `Event` 应该被编进去
- 哪些 `Bus` 应该被编进去
- 哪些 `Snapshot` 应该被编进去

它表达的是“选择关系”，不是最终运行时加载产物。

## compiled `Bank`

compiled `Bank` 是运行时加载单元。

它不是 authoring 工程本身，而是构建后的产物。

当前 `Bank` 里分成两部分：

- `objects`
- `manifest`

## `BankObjects`

`BankObjects` 是 compiled bank 中的高层对象清单。

当前包含：

- `events`
- `buses`
- `snapshots`

它表达的是：

- runtime 在加载这个 bank 后，应该知道哪些高层对象存在

## `BankManifest`

`BankManifest` 是 compiled bank 中的媒体清单。

当前包含：

- `assets`
- `resident_media`
- `streaming_media`

它表达的是：

- backend 在加载这个 bank 时，应该准备哪些底层媒体资源

## 为什么要分成这些层

如果不分层，几个概念会混在一起：

- authoring 工程里的定义
- build 阶段的选择关系
- runtime 的对象清单
- backend 的媒体清单

分层之后，职责更清楚：

- `AuthoringProject`
  - 项目内容集合
- `BankDefinition`
  - authoring 层的“这个 bank 要打包什么”
- `Bank`
  - compiled runtime bank
- `BankObjects`
  - runtime 关心的对象清单
- `BankManifest`
  - backend 关心的媒体清单

## 当前构建路径

当前最小构建路径是：

1. 在 `AuthoringProject` 里组织 `assets/events/banks`
2. 选择一个 `BankDefinition`
3. 调用 `compile_bank_definition(...)` 或 `compile_bank_definition_to_file(...)`
4. 得到 `CompiledBankPackage` 或写出 compiled bank JSON 文件
5. runtime 消费 `Bank.objects`
6. firewheel backend 消费 `Bank.manifest`

如果调用方希望直接从 project 级入口工作, 当前也可以使用：

- `compile_project_bank(...)`
- `compile_project_bank_file(...)`
- `compile_project_bank_to_file(...)`
- `compile_project_bank_file_to_file(...)`

当前推荐心智模型：

- editor / authoring 工具读取 `AuthoringProject`
- build 层把它导出成 compiled bank 文件
- runtime 只读取 compiled bank 文件

## 当前边界结论

当前仓库已经开始遵守这个边界：

- runtime 不再持有完整 compiled `Bank`
- runtime 只保留 `BankObjects`
- firewheel 直接消费 `BankManifest`
- `AudioAsset` 更偏 authoring/import 语义，不再是 backend 加载 compiled bank 的主路径
