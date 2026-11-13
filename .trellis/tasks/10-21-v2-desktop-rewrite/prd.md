# Sketch2Asset v2 桌面端全面重构 — 总 PRD

## 1. 产品定位与愿景

- **产品定位**：面向游戏开发者与美术创作者的**开源 Windows 原生桌面端 AI 游戏资产创作工作站**。
- **核心架构**：`Tauri 2 + Svelte 5 (Runes) + Tailwind CSS v4 + 纯 Rust 核心 (reqwest / rusqlite / keyring / ort DirectML)`，彻底取代 v1 的 `start.bat + Python HTTP Server + 单文件 HTML`。
- **视觉标准**：遵循 `.trellis/spec/frontend/design-system.md`（对标 Awwwards 级“暗房/锻造坊”先锋视觉，Stage 表现区张扬惊艳，Bench 工作区冷静高效，全量 Lucide 图标、零 Emoji、中英双语、深浅双主题）。
- **分发与更新**：GitHub Releases + `tauri-plugin-updater` 应用内一键自动检查与安装更新。

---

## 2. 三阶段迭代路线图 (Task Tree)

| 版本阶段 | Trellis 子任务 | 核心交付内容 |
| :--- | :--- | :--- |
| **第一版 (V1)** | `10-21-v2-phase1-core-gen` | **桌面端基础框架与核心文生图**：自定义标题栏外壳、API 配置多方案切换（Key 存 Windows 凭据管理器 + 磁盘 `settings.json` 永不丢失）、IDE 级可搜索模型下拉选择器、结构化提示词/草图引导生图（兼容 `/v1/images` 与 `/v1/chat/completions`）、非阻塞生成队列、项目归档与本地图库。 |
| **第二版 (V2)** | `10-21-v2-phase2-studio` | **Pro Studio 工作台**：Rust 原生 `ort + DirectML` 深度学习抠图（BiRefNet/BEN2/ISNet 在线下载与 GPU 推理）、物理前景色反解去白边 + Choke 收边 + 4px 色彩外扩、AI 像素艺术网格纯化与色板量化（单格绝对单色 + PICO-8/GB 色板 + 原生尺寸导出）、画布点选/笔刷修补与裁剪、系统级多图拖拽批量导入与处理。 |
| **第三版 (V3)** | `10-21-v2-phase3-character-inpaint` | **游戏角色工坊 & 画布框选批注局部重绘**：<br>1. **角色生成与三视图工坊**：从提示词或参考图生成角色立绘/标准正侧背三视图设定集；<br>2. **基于三视图的角色换装派生**：锁死角色体型与五官特征，输入新服装描述或参考图生成换装后的新三视图/立绘；<br>3. **角色序列帧生成 (Sprite Sheet)**：生成待机/行走/攻击等网格序列帧，内置切帧对齐、自动抠图纯化与 GIF/帧率动态预览；<br>4. **工作台多框选批注局部修改 (Annotated Region Edit)**：在画布上自由拉出多个选框并挂接文字备注（如“1号框：把剑柄改成骷髅头”、“2号框：去掉背景杂物”），自动合成视觉标记引导图或 Mask，一次请求完成精准局部重绘。 |

---

## 3. 全局验收标准 (Cross-Child Acceptance Criteria)

1. **零外部运行时依赖**：打包后在未安装 Python 的 Windows 10/11 机器上双击即用。
2. **配置永久可靠**：API 方案与设置在应用升级、重启后 100% 保留；API Key 安全存放于 Windows 凭据管理器。
3. **跨层性能底线**：前后端之间绝不传输 Base64 大图，全部通过本地文件路径 + `asset://` 协议流转；像素纯化与大图处理不阻塞 UI 帧率（60fps）。
4. **工程质量门禁**：每个子阶段完成时必须通过 `pnpm check`、`pnpm test`、`cargo clippy -- -D warnings`、`cargo test`。
