# Sketch2Asset — AI Game Asset Forge, Sprite Sheet Slicer & Pixel Studio

<p align="center">
  <b>An open-source native desktop workstation built for game developers, indie creators, and digital artists</b><br>
  Unifying structured asset generation, character turnaround & skinning, anti-clipping sprite sheet slicing with per-frame bounds expansion, local DirectML GPU matting, and AI pixel art quantization into one production pipeline.
</p>

<p align="center">
  <b><a href="README.md">English</a></b> | <b><a href="README_zh.md">简体中文</a></b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Architecture-Tauri_2_%2B_Rust-orange.svg" alt="Tauri 2 + Rust">
  <img src="https://img.shields.io/badge/Frontend-Svelte_5_Runes-ff3e00.svg" alt="Svelte 5">
  <img src="https://img.shields.io/badge/Matting-ONNX_DirectML_GPU-10b981.svg" alt="ONNX Runtime DirectML">
  <img src="https://img.shields.io/badge/Storage-AppData_%2B_DPAPI-6366f1.svg" alt="AppData Isolation">
  <img src="https://img.shields.io/badge/i18n-English_%2F_简体中文-06b6d4.svg" alt="i18n">
  <img src="https://img.shields.io/badge/License-MIT-22c55e.svg" alt="MIT License">
</p>

---

## Why Sketch2Asset v0.2.0?

General-purpose AI image generators routinely fail at the **"last mile" of game production**: pixel art suffers from anti-aliasing blur and inconsistent sub-pixel "mixels"; sweeping sword slashes in sprite sheets breach uniform grid lines and get severed; transparent PNGs imported into Unity, Unreal, or Godot show ugly white halos under bilinear filtering; and multi-batch assets drift out of style consistency across a project.

**Sketch2Asset v0.2.0** is rebuilt from the ground up with **Tauri 2 + Rust + Svelte 5** as a pure native desktop application with zero Python dependency. It bridges generation, character derivation, sprite sheet calibration, and pixel-level post-processing across three specialized workspaces to deliver truly **Game-Ready** assets.

---

## Feature Panorama

### 1. Asset Forge — Structured Game Asset Generation

1. **6 Game Asset Archetypes & Context-Aware Modifier Tags**
   Switching archetypes automatically injects production composition rules and surfaces dedicated modifier chips:
   - **Item Icon (`item_icon`)**: Centered isolated object, readable silhouette, generous border margin; modifiers for *45° Isometric, Front Symmetrical, Crisp Rim Light, Metallic & Gem Sheen, Battle-worn*.
   - **Pixel Sprite (`pixel_sprite`)**: Enforces a strict 1:1 square pixel grid, **Zero Mixels** (no sub-pixel details), and flat solid cell colors; modifiers for *Strict Grid (No Mixels), 32×32 Grid Density, 64×64 HD Pixel, 1px Dark Outline, Side-scroller View, Top-down RPG View*. **Automatically triggers the Pixel Art Cleaner pipeline upon completion.**
   - **Character Card (`character_card`)**: Full-body head-to-toe standing composition; modifiers for *Standard A-Pose, Dynamic Action Pose, 3-View Turnaround, Layered Costume Detail*.
   - **Tile Texture (`tile_texture`)**: Orthographic top-down surface with even lighting; modifiers for *Seamless Tileable, 90° Orthographic Top-down, Flat Diffuse Lighting, Stylized Hand-painted*.
   - **UI Element (`ui_element`)**: Symmetrical front-facing orthographic UI component with clean center; modifiers for *9-Slice Frame Ready, Ornate Filigree Corners, HUD Emblem Medallion, Sci-Fi Hard-Surface HUD*.
   - **Free Mode (`free`)**: Unconstrained composition with optional *Clean Silhouette, Studio Rim Lighting, Game-Ready Asset* tags.

2. **Negative Guardrails & Prompt Variable Matrix**
   - **5 Built-in Negative Guardrails**: One-click toggles for *No Text / Watermark, No Cropped Edges, No Cast Ground Shadow, No Background Clutter, No Depth Blur / Noise*, plus custom negative prompt input.
   - **`[A|B|C]` Prompt Matrix Syntax**: Write prompts like `A [fire|ice|lightning] enchanted [greatsword|staff]` to automatically expand into up to 16 Cartesian variants for parallel batch exploration.

3. **Project Hub & Style Reference Anchors**
   - Create, rename, and switch isolated game projects with automatic per-project archiving.
   - Lock a project-wide **Style Prompt suffix** and up to **4 Style Reference Anchor images** that are automatically injected into every generation to keep art direction cohesive across hundreds of assets.

4. **Multi-Image Reference & Region Annotation Guide**
   - Drag or paste multiple reference images (`[#1]`, `[#2]`...) and draw numbered **Region Annotation boxes** directly on any reference image with targeted edit instructions, automatically compositing a visual guide for multimodal models.
   - 8 built-in art styles (*16-Bit Pixel, Cartoon, Anime Cel, 3D Clay/Resin Render, Low-Poly, PBR Realistic, Watercolor*) and full resolution presets (`1024×1024`, `1536×1024`, `1920×1080` 16:9 Widescreen, `2048×2048` 2K, `3840×2160` 4K UHD, plus custom dimensions).

---

### 2. Character Workshop — Turnarounds, Sprite Sheets & Expressions

Built on an extensible modular registry (`CHARACTER_MODULES`) to derive a complete character asset suite from a single concept:

#### `MOD-01` Turnaround & Identity-Locked Skinning
- **Front / Side / Back Turnaround Slicer**: Generates horizontally aligned front, side, and back views on a shared baseline. Uses vertical projection valley detection to automatically slice the sheet into 3 individual views without cutting into arms or capes, with draggable split lines for fine-tuning.
- **Identity-Locked Outfit Skinning**: Locks the base character's facial features, proportions, and silhouette while generating new armor tiers, class outfits, or color palettes from text prompts.

#### `MOD-02` Sprite Sheet Studio
- **8 Action Presets & Virtual Safe-Zone Grid Guide**:
  - Includes a **Default Free Template** (virtual placeholder that disappears on typing, with zero built-in motion bias) plus **Idle Breathing, Side Walk, Dynamic Run, Melee Attack, Magic Spellcasting, Jump & Landing Arc, and Hit / Knockback** presets.
  - Automatically synthesizes and injects a **Safe-Zone Grid Layout Guide** image to help multimodal models respect cell boundaries and foot baselines.
- **Connected-Island Smart Slicing & Slash VFX Protection**:
  - **Connected-Island Clustering**: Automatically tracks weapon tips or capes connected to the character across grid lines instead of severing them flat, while stripping stray fragments intruding from neighboring cells.
  - **Non-Destructive VFX Preservation**: When automatic background removal is unchecked, 100% of bright sword slashes, energy arcs, and translucent effects are preserved intact without accidental erasure.
- **Foot Pivot Alignment & Anti-Clipping Safe Canvas**:
  - Detects each frame's lower-body foot center of gravity and aligns all frames to a unified ground baseline. **Single-frame position nudges are strictly decoupled from canvas sizing**, guaranteeing pure rigid-body translation with zero scaling jitter.
  - Computes the maximum horizontal and vertical reach across the entire sequence to expand a uniform safe canvas so overhead slashes never clip.
- **Dual-Channel Per-Frame Calibration: Position Shift + Bounds Expand**:
  - **Position Shift**: Per-frame `←` `→` `↑` `↓` pixel nudging with one-click reset `⟲`.
  - **Bounds Expand (`R` / `L` / `T` / `B`)**: When a wide slash extends into the next cell (e.g., frame #5's blade arc crossing into frame #6), expand or shrink the capture boundary on any side (`+10` / `-10` px) to recover the full slash into the current frame or crop out neighbor debris.
  - **Interactive Raw Sheet Grid**: Click any cell directly on the raw sprite sheet overview to select that frame for calibration, complete with live highlight borders and expansion badges (e.g., `+40R`).
- **Interactive Animation Previewer & Export**:
  - Supports **smooth mouse-wheel zoom (0.25x ~ 8x)**, left-click drag pan, view reset, 2~24 FPS playback control, onion-skin ghosting, and per-frame ON/OFF toggles.
  - Re-composes and exports calibrated frames into a **standardized equal-cell Sprite Sheet PNG** or saves directly into the project library with instant cross-tab synchronization.

#### `MOD-03` Expression Sheet Generator
- Supports **2×2 (4 expressions), 2×3 (6 expressions), and 3×3 (9 expressions)** grids. Generates cohesive dialogue portrait variations (joy, anger, sorrow, surprise, shy, smug, etc.) and automatically slices them into standalone portrait assets.

---

### 3. Canvas Studio — Native GPU Matting & Pixel Art Quantizer

1. **Local DirectML GPU AI Matting & Model Manager**
   - Powered by Rust `ort` + native **ONNX Runtime with DirectML**, supporting NVIDIA, AMD, and Intel GPUs out of the box (with automatic CPU fallback) and zero Python/CUDA setup.
   - Built-in one-click downloader and manager for 6 SOTA segmentation models:
     | Model ID | Size / Precision | Best For |
     | :--- | :--- | :--- |
     | **`birefnet`** | ~490 MB (FP16) | **Flagship SOTA**: Complex silhouettes, hollow props, and semi-transparent edges |
     | **`ben2`** | ~220 MB | Next-gen balanced model combining fast inference with crisp boundaries |
     | **`birefnet-hrsod`** | ~490 MB (FP16) | High-resolution detail specialist for large character illustrations |
     | **`birefnet-lite`** | ~220 MB | Lightweight model for low-spec hardware or rapid previews |
     | **`isnet-general-use`** | ~178 MB | Ultra-fast general purpose matting for clean icon batches |
     | **`isnet-anime`** | ~176 MB | Specialized for 2D anime characters, hair strands, and cel lineart |
   - Also includes an instant **Solid-Color Corner Flood Cutout** algorithm for millisecond background removal on studio-white or solid-color backdrops.

2. **3-Stage Game Engine Anti-Halo Protection**
   - **Foreground Color Unmixing**: Mathematically removes white background contamination from semi-transparent edge pixels.
   - **Alpha Edge Choke (0~3px)**: Shrinks the alpha boundary inward to eliminate residual light halos.
   - **4px Export Color Bleed**: Dilates foreground RGB colors 4px outward into transparent regions (`Alpha = 0`) upon PNG export, completely preventing dark or white seams under bilinear texture sampling in Unity, Unreal, and Godot.

3. **AI Pixel Art Quantizer & Grid Cleaner**
   - **Eliminate Fake Pixels (Mixels) & Noise**: Automatically detects or locks pixel grid steps (`Auto`, `32×32`, `64×64`, `128×128`, `256×256`, or fixed `N px`), sampling dominant cell colors to strip diffusion gradients and guarantee **100% flat solid colors per pixel block**.
   - **Detail Preservation & Retro Palettes**: Switch between Standard and Detail-Preserving modes (protecting 1-pixel eye highlights); quantize to 16/32/64 colors or map to classic **PICO-8 (16-color)** and **Game Boy (4-color)** palettes.
   - **1:1 Native Resolution Export**: Export a 1024×1024 AI pixel sprite directly at its true native pixel resolution (e.g., crisp `64×64` or `32×32` PNG) with zero interpolation blur.

4. **7 Interactive Canvas Tools & Region Annotation Redraw**
   - **Wand Flood Restore / Erase**: Click any accidentally removed weapon tip or leftover background island to flood-restore from the original image or flood-erase by color tolerance.
   - **Brush Restore / Erase**: Freehand alpha channel retouching with adjustable brush radius.
   - **Marquee Crop & Auto-Trim**: Drag-crop custom bounds or one-click trim transparent margins.
   - **Region Annotation & AI Inpaint Redraw**: Draw numbered color boxes (`#1`, `#2`...) directly on the canvas, attach specific revision notes to each box, and submit for targeted AI redraw.
   - **4 Inspection Backdrops & 40-Step History**: Toggle between Checkerboard, Dark, Light, and **Engine Magenta (`#FF00FF`)** backdrops, hold to compare against the original image, and undo/redo up to 40 steps (`Ctrl+Z` / `Ctrl+Y`).

---

## 4. API Hub & System-Level Data Security

- **Dual-Pane Shuttle Model Picker**:
  - Ships with **zero hardcoded default models** to avoid clutter. Pull available models from `/v1/models` with one click, filter image models, search by keyword, inspect truncated long model names via **full-name hover tooltips**, shuttle models into your enabled list, or add custom model IDs manually.
  - Supports both OpenAI Images API (`/v1/images/generations` & `/v1/images/edits`) and multimodal Chat Completions API (`/v1/chat/completions`).
- **C: Drive AppData Isolation (Zero Repo Pollution)**:
  - **Encrypted Credentials**: API keys are stored via the native Windows Credential Manager (DPAPI) and user profile storage—never written as plaintext files inside the workspace folder.
  - **All User Data Isolated in `%AppData%\com.sketch2asset.app`**:
    - `settings.json` / `secrets.json`: Preferences and encrypted credential fallback
    - `data/library.db`: SQLite 3 (WAL mode) project and asset metadata index
    - `outputs/`: All user project folders and generated/edited PNGs
    - `models/`: Downloaded ONNX matting model weights
- **Self-Contained Runtime & Dual-Stage DLL Preloading**:
  - Immunized against legacy Windows C++ runtimes (e.g. VS 2019 14.22 in `System32`) and strict WebView2 isolation modes (`0x800`). Employs `preload_startup_runtime_dlls()` at process entry millisecond 0 to bind bundled modern MSVC 14.50 CRT & DirectML libraries, combined with `SetDefaultDllDirectories(0x1000)` + `AddDllDirectory` + `LOAD_WITH_ALTERED_SEARCH_PATH`, eliminating `LoadLibraryExW failed (1114)` startup errors entirely.
- **In-App Update Checker**:
  - The Settings modal includes an "About & Update Checker" card that connects to the GitHub Releases API in real-time, displaying current and remote versions with one-click access to the latest release assets.

---

## Local Development & Build

### 1. Prerequisites
- **OS**: Windows 10 / 11 (x64)
- **Frontend**: [Node.js](https://nodejs.org/) >= 18, [pnpm](https://pnpm.io/) >= 9
- **Backend**: [Rust](https://www.rust-lang.org/) >= 1.75 (MSVC toolchain)

### 2. Development Mode
```bash
# 1. Install frontend dependencies
pnpm install

# 2. Launch Tauri desktop window with HMR
pnpm tauri dev
```

### 3. Type Check & Production Build
```bash
# Run strict Svelte / TypeScript diagnostics
pnpm check

# Build frontend production bundle
pnpm build

# Compile optimized native Rust release binary
cargo build --release --manifest-path src-tauri/Cargo.toml --features tauri/custom-protocol
```

---

## Repository Structure

```text
sketch2asset/
├── src/                                  # Svelte 5 frontend source
│   ├── features/
│   │   ├── generate/                     # Asset Forge (archetypes, modifiers, prompt matrix, annotations)
│   │   ├── character/                    # Character Workshop (turnaround, sprite slicer & bounds expand, expressions)
│   │   ├── studio/                       # Canvas Studio (GPU matting, pixel cleaner, wand/brush, region redraw)
│   │   └── settings/                     # Settings Modal (provider shuttle, theme & language dropdown)
│   ├── lib/
│   │   ├── components/                   # Shared UI (custom titlebar, model combobox with tooltips)
│   │   ├── imaging/                      # Core imaging algorithms (island slicer, foot pivot, pixel cleaner, unmixing)
│   │   ├── i18n/                         # Reactive ZH/EN bilingual dictionary
│   │   └── stores/                       # Global reactive state & cross-tab asset sync
│   └── App.svelte                        # Root workspace container
├── src-tauri/                            # Tauri 2 + Rust backend source
│   ├── src/
│   │   ├── commands/                     # IPC command handlers
│   │   ├── library/                      # SQLite 3 asset & project repository
│   │   ├── matting/                      # ONNX Runtime (DirectML) inference & anti-halo post-processing
│   │   ├── providers/                    # OpenAI Images & Chat Completions dual-protocol client
│   │   ├── secrets.rs                    # Windows Keyring (DPAPI) + AppData credential manager
│   │   └── state.rs                      # AppData initialization & automatic legacy data migration
│   ├── build.rs                          # Build-time DLL synchronization script
│   ├── Cargo.toml                        # Rust package manifest
│   └── tauri.conf.json                   # Tauri window & bundle.resources configuration
├── package.json
└── vite.config.ts
```

---

## Acknowledgments

Special thanks to the **[LINUX DO](https://linux.do/)** community and all developers, creators, and testers whose feedback, discussions, and support made this release possible!

---

## License

Released under the [MIT License](LICENSE). Free for both personal indie projects and commercial game development.
