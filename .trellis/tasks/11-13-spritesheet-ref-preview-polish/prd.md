# MCP 与角色工坊序列帧质量、预览器修复及参考图升级

> 状态：开发中（WIP 云端同步断点，待与用户敲定参考图与 UI 细节后统一编码实施）

## Goal

沉淀截至 `2026-11-13` 已完成的 MCP/本地抠图/ComfyUI 基础能力，并系统修复桌面端实测暴露出的序列帧预览/导出贴地、关闭对齐向右下角裁剪、刀光高光区域局部穿孔、工作台/角色工坊交互痛点，以及建立跨视角、跨头身比友好的参考图工作流。

---

## 本阶段已完成（待云端同步的当前本地代码）

- **MCP 服务与热更新闭环** (`src-tauri/src/mcp/`)：
  - 新增 `sketch2asset-mcp.exe --mcp` 独立进程、一键 Agent 接入配置中心（`src/features/settings/SettingsView.svelte`）及 `s2a_create_project` / `s2a_switch_profile` / `s2a_list_assets` 等工具；
  - 每次 MCP 请求实时从磁盘重读 `settings.json` / 凭据配置；`s2a_generate_character` 补齐 `project` 参数与多图/引导参数接入。
- **ComfyUI 本地工作流接入与调试脚本** (`src-tauri/src/providers/comfyui.rs`、`scripts/mock_comfyui_server.py`)：
  - 支持本地 ComfyUI 工作流节点映射与离线 mock 验证脚本。
- **Rust 端序列帧编排与 GPU 抠图一期优化** (`src-tauri/src/postprocess.rs`、`src-tauri/src/matting/mod.rs`、`src-tauri/src/providers/mod.rs`)：
  - 动作关键词改为整词边界匹配（修复 `white` 误匹配 `hit` 等子串误触）；
  - Rust 序列帧对齐支持 `preserve_vertical_arc`（MCP 磁盘直出保留跳跃腾空高度）；
  - 新增 `run_matting_spritesheet_grid`、头发封闭腔体洪泛清理、单格像素量化、HTTP 上游自动重试与 `/images/edits` 非降级修复。

---

## 待修复与待开发清单 (Requirements & Checklist)

### 1. 桌面端序列帧切帧、预览与导出 Bug（高优）
- [ ] **修复关闭「自动脚底重心对齐」时整帧向右下偏移并裁断的 Bug**
  - **已定位根因**：`src/lib/imaging/spritesheet.ts` `extractCellsByConnectedIslands` (`#L637-L650`) 在输入带透明通道（或开启去背景）且启用 `useSmartIslands` 时，会给 `rawCell` 四周各加 `25%` 格宽/格高边距（`padX` / `padY`）；但 `sliceAndAlignSpriteSheet` 的 `alignPivot = false` 分支 (`#L820-L826`) 仍按无边距的 `baseCellW / baseCellH` 计算居中位移，未减去 `padX / padY`，导致图像向右下方多推 `1/4` 格宽高并被画布右/下边界直接截断。
- [ ] **修复桌面端预览器/导出器对跳跃动作强制贴地的 Bug**
  - **已定位根因**：MCP/Rust 端虽保留了腾空高度，但桌面端 `CharacterView.svelte` (`#L318`) 默认 `spriteAlignPivot = true`，载入预览时前端 `sliceAndAlignSpriteSheet` (`src/lib/imaging/spritesheet.ts#L817-L819`) 会无条件对每一帧执行 `shiftY += Math.round(targetBaselineY - pivot.maxY)`，把空中帧拉回 `GROUND (92%)`；桌面端导出（`handleExportAlignedSpriteSheet` / `handleExportCurrentFrame`）同样受影响。
  - **修法**：前端同步引入 `preserveVerticalArc`（或按动作类型/原始高度包络检测），仅水平居中并用地面帧锚定地面基准，保留空中帧相对离地高度。

### 2. 抠图与特效边缘质量优化
- [ ] **修复月牙刀光/发光特效内部亮白高光区偶发穿透镂空的问题**
  - **已定位根因**：`src-tauri/src/matting/mod.rs` `compose_rgba` (`#L1188-L1237`) 的背景腔体洪泛 `pocket_queue` 仅阻挡高于彩度阈值的 `vfx_mask` 像素；刀光内侧高白区域低彩度且接近白底，一旦边缘有半透明缝隙（`alpha_src < 0.85`），BFS 会渗透入内部导致小黑洞/棋盘格穿孔。
  - **修法**：为被 `vfx_mask` 环绕或紧邻亮色光效核心的内侧像素增加拓扑封闭屏障保护。

### 3. 角色工坊 & 动画预览器 UI / 交互优化
- [ ] **精简动画工坊与面板的冗长说明文案**：压缩 `src/lib/i18n/index.svelte.ts` (`#L416-L457`) 及 `CharacterView.svelte` (`#L2491-L2639`) 的长括号解释文本，核心说明移入悬停 Tooltip。
- [ ] **增强底部资产胶片栏滚动条可见度**：调整 `src/app.css` (`#L90-L104`) 与胶片栏容器（`CharacterView.svelte#L2781`、`StudioView.svelte#L2102`）横向滚动条粗细、轨道对比度及滚轮横向滚动体验。
- [ ] **统一底部胶片栏交互 + 右侧新增图库选图入口**：
  - 去掉 `CharacterView.svelte` (`#L2781-L2809`) 序列帧模式卡片上拥挤的双悬停小按钮（移除「设为参考图」），恢复像 `StudioView.svelte` (`#L2104-L2119`) 一样的单击直接载入查看/切帧；
  - 在右侧「角色外观参考图」面板（`CharacterView.svelte#L2507-L2535`）保留「+ 上传角色立绘」并新增「从工坊图库选择」。
- [ ] **预览器支持方向键微调与单拖角色图片对齐**：
  - 为预览阶段添加键盘方向键（`← → ↑ ↓`，可选 `Shift` 大步进）直接调节当前帧 `dx / dy`；
  - 在 `CharacterView.svelte` (`#L263-L283`) 增加「拖动当前帧图片（改 `dx/dy`）」与「拖动画布视角（改 `previewPanX/Y`）」的清晰操作切换/快捷修饰键。

### 4. 参考图（Reference Image）体系与多图上传重构（待用户最终拍板）
- [ ] **待决策方向**：由于游戏视角（横版 2D / HD-2D 伪 3D / 俯视角 / 4向）与头身比/非人形角色组合庞大，避免维护数百张易导致串味和体型绑架的僵硬火柴人素材库，拟采用双层结构：
  1. **内置通用层（视角与比例无关）**：将现有 `buildSafeGridGuideDataUrl` 升级为「每帧运动包络框 + 重心高度轨迹 + 动作方向矢量」引导图（桌面端与 MCP 共用）；
  2. **可选高精度母板层**：新增可选「动作姿态参考图 / 母板」槽位（UI 可选图库已有成品/外部图，MCP `s2a_generate_character` 同步开放 `pose_reference_path`），用于同项目跨角色换皮时复刻精准视角、比例与动作节奏。
- [ ] **修复 `/images/edits` 多图传输缺失隐患**：`src-tauri/src/providers/mod.rs` (`#L879-L952`) `edit_regions_via_images` 在无 mask 时第 `936` 行仅发送 `guide_bytes.unwrap_or(original_bytes)` 单张图（相比 `edit_regions_via_chat` 双 `image_url` 会丢图），需兼容多图上传（`image[]`）或自动合理合成为安全双图。

---

## Acceptance Criteria

- [ ] 取消「自动脚底重心对齐」时，含透明通道序列帧不再向右下角偏移裁断。
- [ ] 开启「自动脚底重心对齐」时，跳跃类序列帧在桌面端预览与导出中完整保留相对腾空高度。
- [ ] 发光刀光/特效内部高光区无误抠穿孔现象。
- [ ] 预览器支持方向键 `← → ↑ ↓` 微调当前帧偏移，并支持仅拖动角色图片本身校准而画布/基准线不动。
- [ ] 底部胶片栏滚动条清晰可辨、单击行为与工作台一致，右侧参考图面板支持从工坊图库直接选图。
- [ ] 完成参考图方案（通用轨迹安全格 + 可选动作母板 + 多图接口兼容）并在 Desktop UI 与 MCP 两端验证通过。
