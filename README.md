# TokenScope

TokenScope 是一款本地优先的 Token 使用统计桌面应用，面向个人记录每天不同模型的 Token 用量，并生成趋势、模型对比和占比图表。

## 功能

- 日期 + 模型 + Token 总量记录
- 模型新增、编辑、停用
- 最近 7 天、30 天、90 天和全部范围筛选
- 每日趋势、模型对比、模型占比
- 当前筛选结果或全部数据导出为 Excel
- Windows 桌面端 SQLite 持久化，无账号、无云端、无遥测
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
