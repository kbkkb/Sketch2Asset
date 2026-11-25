# Changelog

All notable changes to **Sketch2Asset** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.0] - 2026-11-25

### ✨ 新增功能 (Added)

- **2.5D 拆图工坊 (Layer Decompose Workshop · See-Through 集成)**
  - 支持一键在隔离目录中自动配置 Python 3.12 + CUDA 12.8 运行时与 `NF4`（4-bit 量化）/ `BF16` 原生权重（基于 Rust 8 线程 HTTP Range 断点续传与镜像自动切换），将单张角色立绘自动透视补全遮挡区域，拆解为 **23+ 层全画布对齐独立散件 PNG 与标准分层 `.psd`**（开箱适配 Spine / Live2D / Unity `PSD Importer` / Godot）。
  - **Unity 风格 2D XY 移动手柄 (Move Gizmo) & 快捷键**：支持红轴（X 水平）、绿轴（Y 垂直）及蓝框（XY 自由拖拽）交互定位，缩放不变形手柄、支持 `Q` / `W` 工具切换与 `1 / 5 / 20px` 方向键精确微调，导出 PSD 与散件包时自动烘焙坐标偏移。
  - **图层二次精拆与精修打通**：支持按连通域/对称轴自动 **左右对称拆分（`-l` / `-r`）**、基于 Marigold 伪深度 KMeans 聚类的 **前后景深二拆（`-back` / `-front` 自动修复背景遮挡带）**、单层 Alpha 边缘收缩去白边（Defringe），以及一键将任意拆层送入「精修工坊」。
- **MCP 桌面端实时联动桥接 (Desktop Live TCP Bridge) & 异步拆图**
  - `Sketch2Asset.exe` 内置带 Session Token 鉴权的本地 TCP 桥接服务，工具集扩展至 **13 个 MCP 工具**（新增 `s2a_decompose_status` 与 `s2a_decompose_character`）。当 AI Agent 调用工具时，桌面端前台实时跳转对应工坊、显示正在生成/拆解的进度卡片，并支持用户直接手动暂停或中止任务。
  - **`s2a_decompose_character` 默认异步执行**：`async_mode: true` 默认毫秒级返回 `taskId`，Agent 通过轮询 `s2a_decompose_status` 读取实时阶段进度与产物路径，彻底摆脱 MCP 3 分钟同步超时限制。
- **桌面单实例锁 (Single-Instance Guard)**
  - 集成 `tauri-plugin-single-instance`，重复启动程序时自动唤醒并聚焦已有主窗口，防止多开实例抢占显存与端口冲突。

### 🎨 体验与画质优化 (Changed & Improved)

- **四重拆图后处理保真管线**
  - **防过度脑补与裙摆前后自动切分**：以原图非透明轮廓约束各层外溢像素（消除双腿镂空处误填色）；检测到外套/裙摆内衬遮挡腿部时自动切分底层 `bottomwear-back` 置于 `legwear` 之下。
  - **缺失部件自动抢救**：比对原图轮廓与高对比色块，自动找回被模型漏检或截断的靴尖（`footwear`）与头顶发饰/蝴蝶结（`headwear`）。
  - **Live2D 标准五官排序与头饰校准**：按五官解剖顺序重排面部层次，校准面部肤色并剔除耳鬓残留发丝；自动隐藏幻觉头饰，并在重叠像素更贴近原图时将干净前景头饰提至头发上方。
  - **无头/非人形立绘平滑降级**：遇到怪物、盔甲、背影等未检出独立人类头部的立绘时，保留全身 13 层语义拆分并跳过五官细化，不再报错中断。
- **显存自动回收与默认硬件控温护卡**
  - 当输入立绘已带有效透明通道时自动跳过冗余 BiRefNet 预抠图，并在启动 CUDA 拆图前主动卸载 DirectML 会话回收显存；按可用显存自适应启用 Group Offload / CPU Offload，UI 与 MCP 默认启用 `cool_ms: 60` 步间散热停顿保护显卡。
- **拆图项目文件夹完整联动**
  - 底部胶片栏支持切换全部 / 未分类 / 各自定义项目文件夹；读取历史拆图工程及执行二次拆分时持久化并自动恢复 `project` 归属与 `sourcePath`。

### 🛡️ 安全与稳定性修复 (Security & Fixed)

- **安全与目录边界加固**：`uninstall_decompose_extension` 改为受管子项白名单精准清理（仅移除 `.venv`、`models`、`see-through`、`s2a_*.py` 等扩展文件），杜绝用户自定义父目录被连带误删；本地 TCP 桥接新增鉴权前 16 MiB 单行上限与 5s 读取超时；HuggingFace 权重下载清单新增路径穿越清洗（拦截 `..`、盘符及绝对路径）。
- **并发与边界 Bug 修复**：修复拆图工坊「选择本地图片」上传 `.jpg` / `.webp` 因后缀固定 `.png` 导致解码失败的问题并自动清理 `%TEMP%` 临时文件；统一 UI 手动拆图与 MCP 异步拆图的原子并发锁（`try_start_decompose_tracker`）；修复 `s2a_remove_background` 中止后仍写盘孤儿文件问题；3D 姿态桥接改为事件直推（`pose_render_requested`）消除托盘后台定时器节流延迟；同步更新内置 Agent `SKILL.md` 文档模板。

### English Summary (`v0.3.0`)

- **Added**:
  - **2.5D Layer Decompose Workshop** (See-Through integration): Isolated Python 3.12 + CUDA 12.8 setup & 8-way parallel Range downloader (`NF4` 4-bit / `BF16`), extracting 23+ occluded/inpainted semantic layers + layered `.psd`, with a **Unity-style 2D XY Move Gizmo** (`Q`/`W` shortcuts, arrow nudging), **L/R & Marigold Depth secondary layer splitting**, single-layer defringe, and direct send-to-studio support.
  - **Desktop Live MCP Bridge & Async Decompose**: Token-authenticated localhost TCP bridge expanding MCP to **13 tools** (adding `s2a_decompose_status` and async-by-default `s2a_decompose_character`), synchronizing live UI tab switches, progress bars, and user cancel controls; added `tauri-plugin-single-instance`.
- **Changed & Improved**:
  - 4-pass decompose post-processing pipeline (silhouette clamp against background fill-in, `bottomwear-back` auto-split, missing `footwear` / `headwear` recovery, Live2D facial Z-order & skin tone calibration, and graceful headless/non-humanoid fallback).
  - Transparent-input skip for redundant matting, DirectML VRAM release before CUDA launch, adaptive Group/CPU Offload, default `cool_ms: 60` thermal protection across UI & MCP, and full project folder filtering & persistence.
- **Security & Fixed**:
  - Scoped extension uninstaller preserving unrelated user files, 16 MiB / 5s pre-auth bounds on the local TCP bridge, HF `rfilename` path-traversal sanitization, JPEG/WebP local upload re-encoding with temp cleanup, atomic UI/MCP decompose tracker locking, cancel checks prior to matting disk writes, event-driven 3D pose bridge wakeup, and updated `SKILL.md` template.

---

## [0.2.2] - 2026-11-17

### ✨ 新增功能 (Added)

- **应用内轻量增量更新 (In-App Incremental Patch Updater)**
  - `check_app_update` 自动识别 Release 中的轻量补丁包 `*_Patch_x64.zip`（仅含 `Sketch2Asset.exe`、`sketch2asset-mcp.exe` 与文档，跳过不变的 ~40 MB DirectML/ONNX 运行库；未提供补丁包时自动回退匹配 `*_Portable_x64.zip`）。
  - 新增 `apply_app_update` 命令：校验官方 GitHub Release 白名单地址、流式下载并实时推送 `app-update-progress` 百分比进度、Pure-Rust `zip` 安全解压校验，支持在当前程序退出后原地替换并自动重启新版本（如安装目录受系统保护，则提前提示使用安装器升级）。
  - 设置页「关于与更新」卡片新增 **「应用内增量更新」** 一键按钮与实时下载进度条，同时兼容安装版与绿色便携版。

### English Summary (`v0.2.2`)

- **Added**:
  - In-app lightweight incremental patch updater (`apply_app_update` + `Patch_x64.zip` auto-detection with `Portable_x64.zip` fallback), featuring real-time download progress bar, URL allowlist & zip extraction safety checks, write-permission preflight, and automatic restart on Windows for both installed and portable editions.

---

## [0.2.1] - 2026-11-17

### ✨ 新增功能 (Added)

- **AI Agent 互联中心 & 独立 MCP 服务 (`sketch2asset-mcp.exe`)**
  - 新增标准 MCP stdio 服务端，提供 11 个工具：`s2a_status`、`s2a_create_project`、`s2a_switch_profile`、`s2a_generate_asset`、`s2a_remove_background`、`s2a_generate_character`、`s2a_list_actions`、`s2a_edit_action`、`s2a_render_3d_pose`、`s2a_edit_region`、`s2a_list_assets`。
  - **实时同步**：桌面端保存 API 与设置时自动落盘，MCP 端实时读取最新配置；前端每 2.5 秒静默轮询 `library_revision`，MCP 新生成的资产可自动刷新入库，无需手动切页。
  - **一键接入引导**：设置页「AI Agent 互联中心」支持自动生成 Claude Desktop / Cursor / Antigravity / OpenCode 等客户端的 `mcpServers` 配置 JSON，并一键复制。
- **ComfyUI 本地工作流直连**
  - 设置页新增 ComfyUI 供应商配置与连通性测试，支持导入自定义工作流 JSON 并映射 Prompt / 宽高 / Batch Size / 采样步数 / 参考图等节点参数。
- **`MOD-04` 3D 动作与 K 帧工坊 (3D Pose & Keyframe Studio)**
  - 基于 Three.js 构建无光照胶囊人偶（支持横板侧面 / 正面 / 45° 斜视视角与 18 节以上完整骨骼层级），内置 8 套手工动作序列模板（Idle / Walk / Run / Attack / Cast / Jump / Hit / KO）。
  - 支持 Unity 风格三轴球面旋转 Gizmo、`W/E` 移动/旋转切换、50 步 `Ctrl+Z` / `Ctrl+Y` 撤销重做、关键帧插值播放，以及一键导出 `4×1` / `3×2` / `4×2` 绿幕/纯色参考图。
  - 集成 **MCP 桌面实时桥接**：AI Agent 可直接读取、修改和渲染用户在桌面端编辑中的 3D 动作数据。
- **Windows 系统托盘 & 关闭询问记忆**
  - 新增原生 Windows 系统托盘图标与菜单（显示主窗口 / 退出应用），以及关闭确认弹窗（最小化到系统托盘 / 直接退出 + 记住我的选择）。
  - 设置页新增「关闭窗口行为」选项（每次询问 / 最小化到托盘 / 直接退出）。
- **顶栏版本提示 & 拆图工坊入口**
  - 顶部导航栏展示当前版本号与检测到新版本时的即时升级提醒胶囊。
  - 新增「拆图工坊（Decompose · 待更新）」顶栏入口占位，用于后续图层拆解功能演进。

### 🎨 体验优化 (Changed & Improved)

- **生图工坊 (Asset Forge)**：新增 `sprite_sheet`（序列帧）生图品类及对应修饰标签与提示词规则。
- **角色工坊 (`MOD-01` 三视图)**：支持点击下方三视图卡片打开高清 Lightbox 放大预览与翻页查看。
- **角色工坊 (`MOD-02` 序列帧)**：
  - 以 **3D 动作参考图** 替代旧版纯文本动作预设模板；**默认启用「引擎原地对齐」**（`preserveVerticalArc = false`），保证横板游戏角色碰撞体与视觉帧对齐，同时保留「预览展示保留抛物线」可选开关。
  - 支持**单帧偏移微调**：点击右侧小窗可单帧拖拽或使用方向键精细微调对齐偏差。
- **角色工坊 (`MOD-03` 表情差分)**：调整为以「基于单张立绘批量生成多张完整角色立绘」作为主模式，更贴合 AVG / 对话差分资产生产。
- **工作台 / 角色工坊底部胶片栏**：增强自定义高亮滚动条与横向拖拽体验，并在角色工坊底部缩略图加入右键二级菜单（支持快捷发送精修、设为主立绘、删除等操作）。

### 🐛 问题修复 (Fixed)

- **SemVer 版本更新误报修复**：修复 `check_app_update` 对 `v0.2.1` 与远端 `v0.2.0` 的比较逻辑，避免高版本被误判为需要更新。
- **关闭对齐后向右下角裁剪偏移修复**：修复序列帧关闭自动居中/脚底对齐后单帧绘制锚点错误导致的画布裁剪问题。
- **3D 动作引导词误触发跳跃抬升修复**：修复 3D 引导模板公共说明中含 `"airborne"` / `"jump"` 词根导致地面动作（`idle`、`run`、`attack`、`ko`）被误加垂直抛物线抬升的 Bug，并强化英文动作词边界识别。
- **序列帧抠图与特效保护**：新增序列帧网格化单帧抠图与刀光/能量特效防穿孔保护，避免相邻帧互相干扰或强光效被误掏空。
- **`/images/edits` 双图协同与重试稳定性**：修复参考图编码、多模态图生图超时与部分中转端重试兼容问题。

---

### English Summary (`v0.2.1`)

- **Added**:
  - Standalone MCP server (`sketch2asset-mcp.exe`) with 11 tools (`s2a_status`, `s2a_create_project`, `s2a_switch_profile`, `s2a_generate_asset`, `s2a_remove_background`, `s2a_generate_character`, `s2a_list_actions`, `s2a_edit_action`, `s2a_render_3d_pose`, `s2a_edit_region`, `s2a_list_assets`), live settings disk sync, 2.5s frontend `library_revision` polling, and one-click MCP client config copy in Settings.
  - Local ComfyUI workflow integration with connection testing and custom JSON node parameter mapping.
  - `MOD-04` 3D Pose & Keyframe Studio powered by Three.js (unlit capsule rig, 8 action templates, Unity-style 3-axis rotation Gizmo, `W/E` mode switching, 50-step Undo/Redo, multi-grid pose sheet export, and live MCP desktop bridge).
  - Native Windows System Tray icon, close confirmation dialog ("Minimize to Tray" / "Exit" with "Remember my choice"), and configurable `close_behavior` in Settings.
  - TitleBar live version badge, update notification pill, and `Decompose (Coming Soon)` workshop tab.
- **Changed**:
  - Added `sprite_sheet` archetype to Asset Forge.
  - Added click-to-zoom Lightbox for `MOD-01` turnaround views.
  - Replaced text action presets with 3D Pose Reference integration in `MOD-02`, defaulted to engine-friendly in-place alignment (`preserveVerticalArc = false` with optional preview arc toggle), and added per-frame drag / arrow-key offset nudging.
  - Upgraded `MOD-03` Expression Generator to prioritize batch standalone portrait variations from a single character reference.
  - Enhanced bottom filmstrip scrollbars across Character Workshop & Canvas Studio, plus right-click context menus in Character Workshop.
- **Fixed**:
  - Fixed SemVer comparison in `check_app_update` so `v0.2.1` no longer falsely flags `v0.2.0` as an update.
  - Fixed bottom-right canvas clipping when frame alignment is toggled off.
  - Fixed 3D pose guide boilerplate strings falsely triggering `preserveVerticalArc` vertical lift on grounded animations (`idle`, `run`, `attack`, `ko`).
  - Improved per-cell sprite sheet matting with slash/VFX hole-protection barriers and `/images/edits` multi-image retry handling.

---

## [0.2.0] - 2026-10

- Complete native rewrite with **Tauri 2 + Rust + Svelte 5**.
- **Asset Forge**: 6 game asset archetypes, negative guardrails, `[A|B|C]` prompt variable matrix, project style anchors, and numbered region annotation references.
- **Character Workshop**: `MOD-01` Turnaround & Identity-Locked Skinning, `MOD-02` Smart Island Sprite Sheet Slicer with per-frame Bounds Expand & Foot Pivot Alignment, `MOD-03` Expression Sheet Generator.
- **Canvas Studio**: Local ONNX Runtime + DirectML GPU AI matting (6 models including BiRefNet / BEN2 / IS-Net), 3-stage game-engine anti-halo pipeline (Foreground Unmixing, Alpha Choke, 4px Color Bleed), AI Pixel Art Quantizer & Grid Cleaner, and 7 interactive retouch tools.
- **Security & Isolation**: AppData-isolated SQLite library and Windows DPAPI encrypted credential storage.
