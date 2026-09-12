# TokenScope

TokenScope 是一款本地优先的 Token 使用统计桌面应用，面向个人记录每天不同模型的 Token 用量，并生成趋势、模型对比和占比图表。

## 功能

- **1.1.1 新特性**：OCR 识别再进化与冲突智能处理
  - 深度适配主流 API 聚合面板与控制台表格排版，自动过滤表头关键词干扰
  - 智能数据冲突处理：支持保存时自由选择 **➕ 累加用量**、**🔄 覆盖更新**、**⏭️ 跳过冲突**，满足一天内分时段多次统计
  - 识别结果抽屉支持「全选 / 反选 / 仅选新模型」快捷批量勾选
  - 截图预览支持点击/双击查看全景大图，便于密集小字与账单核对
  - 增强带有空格与各类千分符的金额与常用缩写（如 `125.4k tokens`、`3.2M tokens`、`$ 12.50`）
- **1.1.0 特性**：支持 OCR 自动识别填入用量
  - 支持直接截图粘贴（Ctrl+V）、拖拽上传或文件选择图片
  - 自动识别模型名称（如 `gpt-6-astra`、`gemini-3.8-flash` 等）与 Token 数量
  - 自动纠正常见 OCR 误识别（如编程字体中的斜杠零 `0̸` 自动还原、连接符空白优化等）
  - 自动识别关联费用并记录到备注（如 `$39.72`）
  - 日期默认自动填入当天，支持修改与单条/批量一键保存
  - 未在库中的新模型支持智能推断供应商并自动新建入库
  - 支持单条记录快速识图填入与多模型批量识别导入
- 日期 + 模型 + Token 总量记录
- 模型新增、编辑、停用
- 最近 7 天、30 天、90 天和全部范围筛选
- 每日趋势、模型对比、模型占比
- 当前筛选结果或全部数据导出为 Excel
- Windows 桌面端基于 Windows.Media.Ocr 硬件加速离线原生识别，0 网络请求，SQLite 本地持久化，无账号、无云端、无遥测
- 软件内一键检查更新：对接 GitHub Releases，一键下载并启动安装包
- 无损数据更新保障：更新前自动备份数据库快照至 AppData，历史用量记录 100% 完好保留
- 浏览器预览模式使用 localStorage 作为开发回退

## 技术栈

- Tauri 2 / Rust
- React / TypeScript / Vite
- SQLite / rusqlite
- ECharts
- ExcelJS
- Vitest

## 开发

```bash
npm install
npm run dev
```

浏览器预览地址通常为 `http://127.0.0.1:1420/`。

运行桌面开发模式：

```bash
npm run tauri dev
```

运行测试和生产构建：

```bash
npm test
npm run build
```

构建 Windows 应用：

```bash
npm run tauri build
```

首次生成 MSI 安装包时，Tauri 可能需要下载 WiX 工具链。应用本体会生成在 `src-tauri/target/release/token-scope.exe`。

## 数据位置

桌面端数据库文件名为 `token-statistics.sqlite`，存放在 Tauri 的应用数据目录。数据库通过版本化迁移创建 `models` 和 `usage_records` 表，并使用 `(usage_date, model_id)` 唯一约束避免重复统计。
