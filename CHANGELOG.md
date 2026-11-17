# Changelog

All notable changes to **Sketch2Asset** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
