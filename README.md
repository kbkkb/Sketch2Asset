# Sketch2Asset — AI Game Asset Forge, 3D Pose K-Frame Studio & Native Matting Workstation

<p align="center">
  <b>An open-source native desktop workstation built for game developers, indie creators, and digital artists</b><br>
  Unifying structured asset generation, Three.js 3D pose keyframing, anti-clipping sprite sheet calibration, local DirectML GPU matting, pixel grid quantization, and AI Agent MCP integration into one production pipeline.
</p>

<p align="center">
  <b><a href="README.md">English</a></b> | <b><a href="README_zh.md">简体中文</a></b> | <b><a href="CHANGELOG.md">Changelog</a></b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Release-v0.2.2-84cc16.svg" alt="Version v0.2.2">
  <img src="https://img.shields.io/badge/Architecture-Tauri_2_%2B_Rust-orange.svg" alt="Tauri 2 + Rust">
  <img src="https://img.shields.io/badge/Frontend-Svelte_5_%2B_Three.js-ff3e00.svg" alt="Svelte 5 + Three.js">
  <img src="https://img.shields.io/badge/Matting-ONNX_DirectML_GPU-10b981.svg" alt="ONNX Runtime DirectML">
  <img src="https://img.shields.io/badge/Agent-MCP_Server_(11_Tools)-6366f1.svg" alt="MCP Server">
  <img src="https://img.shields.io/badge/Workflow-ComfyUI_Ready-06b6d4.svg" alt="ComfyUI Ready">
  <img src="https://img.shields.io/badge/License-MIT-22c55e.svg" alt="MIT License">
</p>

---

## Why Sketch2Asset v0.2.1?

General-purpose AI image generators routinely fail at the **"last mile" of game production**:
- Sweeping sword slashes in sprite sheets breach uniform grid lines and get severed, or airborne jump frames float inside the texture and misalign Unity / Godot colliders;
- Generated pixel art suffers from anti-aliasing blur and inconsistent sub-pixel "mixels";
- Transparent PNGs show ugly white halos under bilinear filtering in game engines;
- Workflows lack native automation bridges to AI Coding Agents (Claude / Cursor / Antigravity) and local ComfyUI pipelines.

**Sketch2Asset** is built natively with **Tauri 2 + Rust + Svelte 5 + Three.js** with zero Python dependency—combining structured generation, 3D pose guidance, engine-ready sprite sheet calibration, DirectML GPU matting, and an MCP server into a single **Game-Ready** production pipeline.

---

## Feature Highlights

<table>
  <tr>
    <td width="50%" align="center">
      <b>1. MOD-04 · 3D Pose & Keyframe Studio</b><br>
      <sub>Unlit Three.js Capsule Rig · Unity-Style 3-Axis Gizmo · 8 Action Presets · Multi-Grid Export</sub><br><br>
      <img src="docs/media/demo-3d-pose.gif" alt="MOD-04 3D Pose and Keyframe Demo" width="100%">
    </td>
    <td width="50%" align="center">
      <b>2. MOD-02 · Anti-Clipping Sprite Slicer & Engine Pivot</b><br>
      <sub>Smart Island Slicing · Per-Frame Shift & Bounds Expand · In-Place Engine Alignment</sub><br><br>
      <img src="docs/media/demo-spritesheet.gif" alt="MOD-02 Sprite Sheet Calibration Demo" width="100%">
    </td>
  </tr>
  <tr>
    <td width="50%" align="center">
      <b>3. MOD-01 · Turnaround Slicer & Identity-Locked Skinning</b><br>
      <sub>Projection-Valley Auto Slice · Click-to-Zoom Lightbox · Class/Armor Outfit Variants</sub><br><br>
      <img src="docs/media/demo-turnaround-skinning.png" alt="MOD-01 Turnaround and Skinning Demo" width="100%">
    </td>
    <td width="50%" align="center">
      <b>4. MOD-03 · Expression & Portrait Variations</b><br>
      <sub>Batch Standalone Portraits from One Reference · Plus 2×2 / 2×3 / 3×3 Grid Slicing</sub><br><br>
      <img src="docs/media/demo-expression.png" alt="MOD-03 Expression Variations Demo" width="100%">
    </td>
  </tr>
  <tr>
    <td width="50%" align="center">
      <b>5. Local DirectML GPU Matting & Anti-Halo Protection</b><br>
      <sub>BiRefNet / BEN2 / IS-Net Native Acceleration · Unmixing + Choke + 4px Color Bleed</sub><br><br>
      <img src="docs/media/demo-gpu-matting.gif" alt="DirectML GPU Matting Demo" width="100%">
    </td>
    <td width="50%" align="center">
      <b>6. Canvas Studio · Region Annotation & AI Inpaint Redraw</b><br>
      <sub>Draw Numbered Boxes (#1, #2) Directly on Canvas · Targeted AI Redraw & Auto-Composite</sub><br><br>
      <img src="docs/media/demo-region-redraw.png" alt="Canvas Studio Region Annotation Redraw Demo" width="100%">
    </td>
  </tr>
</table>

---

## Workspace Panorama

### 1. Asset Forge — Structured Game Asset Generation

<p align="center">
  <!-- 📷 [Screenshot #3] Asset Forge full workspace with prompt dock, tags & project hub -->
  <img src="docs/media/forge-overview.png" alt="Asset Forge Workspace" width="88%">
</p>

- **7 Game Asset Archetypes**: Includes the new **`Sprite Sheet (sprite_sheet)`** archetype alongside **Item Icon, Pixel Sprite, Character Card, Tile Texture, UI Element, and Free Mode**, automatically injecting production composition rules and modifier tags.
- **Prompt Matrix & Negative Guardrails**: Expand `[fire|ice|lightning]` × `[greatsword|staff]` into up to 16 parallel variants; toggle 5 game-ready negative guardrails with one click.
- **Project Style Anchors & Region Annotations**: Lock a project-wide Style Prompt and up to 4 reference anchors, or draw numbered bounding boxes (`#1`, `#2`...) directly on reference images for targeted edits.

---

### 2. Character Workshop — 3D Pose, Turnarounds, Sprite Sheets & Expressions

<p align="center">
  <!-- 📷 [TODO Screenshot] Character Workshop full view (save as docs/media/character-workshop.png) -->
  <img src="docs/media/character-workshop.png" alt="Character Workshop Interface" width="88%">
</p>

- **`MOD-01` Turnaround & Identity-Locked Skinning**: Generates horizontally aligned front/side/back views with projection-valley slicing, **click-to-zoom Lightbox inspection**, and identity-locked outfit variants.
- **`MOD-02` Sprite Sheet Studio**:
  - **3D Pose Reference Driven**: Uses multi-cell pose sheets rendered from `MOD-04` instead of brittle text-only action templates.
  - **Engine In-Place Alignment (Default)**: Defaults to `preserveVerticalArc = false` so jump/action frames align cleanly with game engine physics colliders, while keeping an optional preview-arc toggle.
  - **Smart Island Slicing & Dual-Channel Calibration**: Preserves sweeping sword slashes & VFX intact; supports per-frame arrow-key / drag nudging, `R/L/T/B` bounds expansion, highlighted filmstrip scrollbars, and **right-click context menus**.
- **`MOD-03` Expression Generator**: Prioritizes generating multiple standalone full character portraits from a single reference for dialogue/AVG pipelines, while retaining `2×2` / `2×3` / `3×3` grid slicing.
- **`MOD-04` 3D Pose & Keyframe Studio** *(v0.2.1 New)*:
  - Unlit Three.js capsule rig (18+ hierarchical bones, side-scroller L/R, HD-2D 45°, 3/4, front, and top views) with 2-head chibi / 3-head / 6-head proportions and weapon props.
  - 8 handcrafted action templates (Idle, Walk, Run, Attack, Cast, Jump, Hit, KO), Unity-style 3-axis spherical rotation Gizmo, `W/E` translate/rotate switching, 50-step `Ctrl+Z/Y` undo/redo, and one-click capture into `MOD-02`.

---

### 3. Canvas Studio — Native GPU Matting & Pixel Quantizer

<p align="center">
  <img src="docs/media/demo-pixel-cleaner.png" alt="AI Pixel Art Cleaner Comparison" width="88%">
</p>

- **DirectML GPU AI Matting**: Powered by Rust `ort` + ONNX Runtime DirectML (NVIDIA / AMD / Intel out of the box, zero Python/CUDA) with 6 downloadable models:

<details>
<summary>Expand to view the 6 built-in matting models & 3-stage anti-halo pipeline</summary>

| Model ID | Size / Precision | Best For |
| :--- | :--- | :--- |
| **`birefnet`** | ~490 MB (FP16) | **Flagship SOTA**: Complex silhouettes, hollow props, and semi-transparent edges |
| **`ben2`** | ~220 MB | Next-gen balanced model combining fast inference with crisp boundaries |
| **`birefnet-hrsod`** | ~490 MB (FP16) | High-resolution detail specialist for large character illustrations |
| **`birefnet-lite`** | ~220 MB | Lightweight model for low-spec hardware or rapid previews |
| **`isnet-general-use`** | ~178 MB | Ultra-fast general purpose matting for clean icon batches |
| **`isnet-anime`** | ~176 MB | Specialized for 2D anime characters, hair strands, and cel lineart |

- **3-Stage Anti-Halo Protection**: Foreground Color Unmixing → Alpha Edge Choke (`0~3px`) → 4px Export Color Bleed.
- **7 Interactive Tools**: Wand Flood Restore/Erase, Brush Restore/Erase, Marquee Crop & Auto-Trim, Region Annotation AI Redraw, 4 backdrops (including `#FF00FF` Engine Magenta), and 40-step history.

</details>

---

### 4. AI Agent Hub (MCP), ComfyUI & Native Desktop Polish

<p align="center">
  <!-- 📷 [Screenshot #4] Settings -> AI Agent Hub or ComfyUI Workflow configuration -->
  <img src="docs/media/settings-mcp-comfyui.png" alt="AI Agent Hub and ComfyUI Settings" width="88%">
</p>

- **Standalone MCP Server (`sketch2asset-mcp.exe`)** *(v0.2.1 New)*:
  - Exposes **11 MCP tools** (asset generation, GPU matting, character derivation, reading/editing/rendering `MOD-04` 3D poses via live desktop bridge, region redraw, project management).
  - **Real-Time Dual Sync**: Settings changes persist immediately for MCP; the frontend polls `library_revision` every 2.5s so MCP-generated assets appear automatically without switching tabs.
  - **One-Click Setup**: Built-in **AI Agent Hub** in Settings generates and installs `mcpServers` configs for Claude Desktop, Cursor, Antigravity, and more.
- **Local ComfyUI Workflow Integration** *(v0.2.1 New)*: Connect local ComfyUI endpoints, import custom workflow JSONs, and map Prompt / Size / Batch / Steps / Reference Image nodes.
- **Native Windows System Tray & Update Checker** *(v0.2.1 New)*: Minimize-to-tray support with configurable close behavior (`ask` / `tray` / `exit`), TitleBar version pill, and accurate SemVer update notifications.
- **Decompose Workshop (Coming Soon)**: Top-bar placeholder reserved for upcoming transparent layer decomposition.
- **AppData & DPAPI Security**: Credentials encrypted via Windows DPAPI; all library data and models isolated inside `%AppData%\com.sketch2asset.app`.

---

## Local Development & Build

<details>
<summary>Expand for prerequisites, build commands, and repository structure</summary>

### 1. Prerequisites
- **OS**: Windows 10 / 11 (x64)
- **Frontend**: [Node.js](https://nodejs.org/) >= 18, [pnpm](https://pnpm.io/) >= 9
- **Backend**: [Rust](https://www.rust-lang.org/) >= 1.75 (MSVC toolchain)

### 2. Commands
```bash
# Install frontend dependencies
pnpm install

# Launch Tauri desktop window with HMR
pnpm tauri dev

# Strict diagnostics & unit tests
pnpm check
cargo test --manifest-path src-tauri/Cargo.toml --lib

# Production build (app + standalone MCP binary)
pnpm build
cargo build --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
```

### 3. Repository Structure
```text
sketch2asset/
├── src/                                  # Svelte 5 frontend source
│   ├── features/
│   │   ├── generate/                     # Asset Forge (7 archetypes, prompt matrix, annotations)
│   │   ├── character/                    # Character Workshop (MOD-01 ~ MOD-04)
│   │   ├── studio/                       # Canvas Studio (GPU matting, pixel cleaner, wand/brush)
│   │   └── settings/                     # Settings (Providers, ComfyUI, AI Agent Hub, Tray)
│   └── lib/
│       ├── components/                   # TitleBar (version/update/decompose), close dialog
│       ├── imaging/                      # Island slicer, foot pivot, pixel cleaner, unmixing
│       ├── i18n/                         # Reactive ZH/EN dictionary
│       └── stores/                       # Reactive state & library_revision sync
├── src-tauri/                            # Tauri 2 + Rust backend + MCP binary
│   └── src/
│       ├── bin/mcp.rs                    # sketch2asset-mcp standalone MCP server
│       ├── commands/                     # IPC commands & Agent Hub integration
│       ├── library/                      # SQLite 3 (WAL) repository & revision tracking
│       ├── matting/                      # ONNX Runtime (DirectML) & anti-halo pipeline
│       └── providers/                    # OpenAI Images / Chat / ComfyUI clients
├── docs/media/                           # README screenshots & demo GIFs
├── CHANGELOG.md                          # Version release notes
└── package.json
```

</details>

---

## Acknowledgments

Special thanks to the **[LINUX DO](https://linux.do/)** community and all developers, creators, and testers whose feedback, discussions, and support made this release possible!

## License

Released under the [MIT License](LICENSE). Free for both personal indie projects and commercial game development.
