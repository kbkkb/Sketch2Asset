# V1: 桌面端基础框架与核心文生图 — 实施计划 (implement.md)

## 执行步骤清单 (Ordered Checklist)

- [x] **Step 1: 旧版文件清理与旧配置保护备份**
  - 将根目录旧版运行文件（`index.html`, `server.py`, `matting_worker.py`, `setup_matting.py`, `setup_matting.bat`, `start.bat`, `requirements.txt`）移除（已在 Git tag `v1-final` 完整封存），保留 `config.json` 与 `outputs/` 供自动迁移。
- [x] **Step 2: 搭建前端工程骨架 (Vite + Svelte 5 + Tailwind CSS v4 + TypeScript)**
  - 创建 `package.json`, `vite.config.ts`, `svelte.config.js`, `tsconfig.json`, `index.html`。
  - 配置 `@tailwindcss/vite` 与 `@lucide/svelte`。
  - 实现 `src/app.css`（Awwwards 级 Darkroom/Forge 令牌体系、深浅双主题变量、自定义滚动条与微动效）。
- [x] **Step 3: 搭建 Tauri 2 Rust 后端核心 (`src-tauri/`)**
  - 创建 `Cargo.toml`, `build.rs`, `tauri.conf.json`（开启 `assetProtocol`、无边框窗口 `decorations: false`、最小尺寸 `1100×700`）、`capabilities/default.json` 及应用图标。
  - 实现 `error.rs`、`state.rs`、`secrets.rs`（Windows 凭据管理器 + `%APPDATA%` 双重持久化 + 自动导入旧版 `D:\Sketch2Asset\config.json`）。
  - 实现 `library/db.rs`（SQLite 项目与资产表初始化、自动扫描已有 `outputs/` 目录资产入库）。
  - 实现 `providers/`（`/v1/models` 在线拉取、`/v1/images/generations` & `/v1/images/edits` 图像接口、`/v1/chat/completions` 对话多模态生图接口）。
  - 实现 `commands/`（设置读写、模型拉取、异步生成任务、项目创建/重命名、资源管理器定位）。
- [x] **Step 4: 实现前端核心组件与视图**
  - 实现双语国际化字典与响应式 `i18n` 模块（`zh-CN` / `en`）。
  - 实现 `TitleBar.svelte`（先锋无边框顶栏、快捷方案切换、主题/语言切换、窗口控制）。
  - 实现 `ModelCombobox.svelte`（IDE 级全宽可搜索下拉选择器，`IMAGE` 徽标分组、键盘导航、随时展开）。
  - 实现 `GenerateView.svelte`（中央时间线画廊流 Stage + 底部悬浮指令舱 Command Dock + 草图拖入/粘贴 + 素材品类芯片 + 右侧项目归档抽屉）。
  - 实现 `SettingsView.svelte`（多套 API 方案管理、Key 安全保存、配置 JSON 导入/导出）。
- [x] **Step 5: 编译验证与质量门禁检查**
  - 运行 `pnpm install`、`pnpm check` 验证前端零类型错误。
  - 运行 `cargo check` / `cargo test` 验证 Rust 核心编译与单元测试通过。
- [x] **Step 6: V1 交互与视觉精修 (用户反馈迭代)**
  - 统一左上角应用主图标与 `.exe` 锻造棱镜徽标，并重绘顶部导航栏四大核心模块矢量图标。
  - 将模型选择器移至底部指令舱，合并为按「供应商方案 -> 旗下模型」分组的单一可搜索上弹面板（模型拉取统一收口在设置页）。
  - 升级底部生图指令舱：新增按素材品类联动的「视角与质感词条」、可开关的「防翻车负向护栏 (Negative Guardrails)」与自定义负向排除词，以及 `[A|B|C]` 批量变量语法自动拆分并发任务。
  - 并发张数支持 `×1 / ×2 / ×4` 快捷按钮 + `1 ~ 16` 自定义数字输入。
  - 右侧项目归档库新增「画风参考锚点图 (最多 4 张)」：支持从生成结果一键钉选或本地上传，并支持生图时自动注入主参考图锁死项目画风。
- [x] **Step 7: V1 体验深度打磨 (第二轮反馈迭代)**
  - **生图品类直选 & 顶部画廊筛选分离**：在底部生图指令舱增加显式的「生图品类 (`archetype`)」选择器；顶部栏改为「画廊品类筛选 (`全部品类 | 道具图标 | ...`)」，生成的资产自动记录所属品类标签。
  - **全局提示弹窗居中放大**：将 Toast 通知统一移至窗口顶部正中央（`top-14 left-1/2 -translate-x-1/2`）并加大字号与视觉对比度。
  - **API Key 密码框显隐与一键复制**：API 设置页保存密钥后不再清空输入框，默认以密码点号遮罩显示，内置小眼睛图标切换明文显隐，并提供一键复制按钮。
  - **AstrBot / PiDesktop 风格「双栏模型穿梭框」**：API 拉取到的全量模型进入左侧「远程拉取备选池」（支持搜索与仅看图像模型过滤），点击移入右侧「生图工坊已启用模型」后才会出现在生图工坊底部的模型下拉列表中，彻底排除无关对话模型干扰。
  - **项目行内铅笔图标重命名**：在右侧项目归档库中，当前选中项目右侧直接显示国际通用铅笔图标按钮，点击即可行内重命名（全面移除浏览器式 `window.prompt` 弹窗）。
  - **彻底抹除 WebView2 / Edge 浏览器痕迹**：通过 `additionalBrowserArgs` 禁用 `msVisualSearch`、`msEdgeImageMagnifier`、`msMiniMenu` 等 Edge 特性，全局拦截默认右键菜单与快捷键，并为生图卡片定制原生暗房右键快捷菜单。
