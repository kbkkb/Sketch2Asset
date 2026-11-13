# V1: 桌面端基础框架与核心文生图 — 技术设计 (design.md)

## 1. 模块边界与目录结构

遵循 `.trellis/spec/frontend/directory-structure.md` 与 `.trellis/spec/backend/directory-structure.md`：

```text
D:\Sketch2Asset/
├── package.json
├── vite.config.ts
├── svelte.config.js
├── tsconfig.json
├── messages/
│   ├── zh-CN.json
│   └── en.json
├── src/
│   ├── app.css                        # Tailwind v4 @theme + Darkroom/Forge 令牌 + 噪点纹理
│   ├── main.ts
│   ├── App.svelte                     # 自定义无边框标题栏 + 导航 + 视图容器 + 全局 Toast
│   ├── lib/
│   │   ├── api/
│   │   │   ├── index.ts               # Tauri invoke / event 统一类型化封装
│   │   │   └── types.ts               # 前后端共享契约类型
│   │   ├── components/
│   │   │   ├── layout/TitleBar.svelte # 顶栏：视图切换、快捷方案/模型切换、主题/语言、窗口控制
│   │   │   └── ui/ModelCombobox.svelte# IDE 级可搜索分组模型下拉框
│   │   ├── stores/
│   │   │   ├── settings.svelte.ts     # 配置方案、主题、语言、当前项目响应式状态
│   │   │   └── jobs.svelte.ts         # 后台生成队列与时间线画廊状态
│   │   └── i18n/
│   │       └── index.svelte.ts        # 类型化双语消息引擎 (zh-CN / en)
│   └── features/
│       ├── generate/
│       │   ├── GenerateView.svelte    # 中央时间线画廊 Stage + 底部悬浮指令舱 Command Dock + 右侧项目抽屉
│       │   └── presets.ts             # 游戏素材品类芯片、美术风格与纯白底规则模板
│       └── settings/
│           └── SettingsView.svelte    # API 方案管理、Key 安全保存、配置 JSON 导入/导出
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/default.json
    └── src/
        ├── main.rs
        ├── lib.rs
        ├── error.rs                   # AppError + AppErrorCode
        ├── state.rs                   # AppState (Settings + DB + HttpClient)
        ├── secrets.rs                 # Windows Credential Manager (keyring) + 本地加密回退兜底
        ├── commands/
        │   ├── mod.rs
        │   ├── settings.rs            # get_settings, save_settings, set_api_key, import_legacy_config, export_config
        │   ├── generation.rs          # fetch_models, start_generation, cancel_job
        │   └── library.rs             # list_projects, create_project, rename_project, list_assets, delete_asset, open_in_explorer
        ├── providers/
        │   ├── mod.rs                 # ImageProvider 统一分发
        │   ├── openai_images.rs       # /v1/images/generations & /v1/images/edits
        │   └── openai_chat.rs         # /v1/chat/completions 多模态生图解析
        └── library/
            ├── mod.rs
            └── db.rs                  # SQLite (rusqlite) 资产与项目元数据存储
```

---

## 2. 核心数据流与契约

### 2.1 API 配置与 Key 双层安全持久化
- **为什么之前会丢配置**：v1 依赖与端口绑定的 `localStorage`。
- **v2 持久化机制**：
  1. 配置文件固定写入系统应用数据目录：`%APPDATA%/com.sketch2asset.app/settings.json`（原子写入：先写 `.tmp` 再 `rename`）。
  2. API Key 优先存入 Windows 凭据管理器（`keyring` service=`sketch2asset`, user=`profile_id`），同时在 `%APPDATA%/com.sketch2asset.app/secrets.json` 保留一份本地备份，确保在任何 Windows 权限环境下 100% 永不丢失。
  3. **首次启动自动迁移**：启动时若检测到旧版工程目录下的 `config.json`，自动将里面的 API 方案、Key 和模型列表无损导入到新配置中！

### 2.2 图片流转（零 Base64 IPC）
- 生成任务完成后，Rust 将解码后的 PNG 直接写入 `<library_root>/<project>/<ulid>.png`，并在 SQLite `assets` 表插入记录。
- Rust 通过 Tauri Event / Command 返回 `AssetItem { id, file_path, project, prompt, model, width, height, created_at }`。
- 前端使用 `convertFileSrc(asset.file_path)` 通过 `asset://localhost/...` 协议高速渲染图片。

### 2.3 结构化游戏素材 Prompt 合成器
- 最终发送给上游模型的 Prompt 由 Rust 端统一合成：
  `[User Subject Prompt] + [Archetype Template] + [Art Style Suffix] + [Project Locked Style] + [Background Rule]`
- 当用户上传了草图/参考图时，自动注入保持构图与主体的引导语。
