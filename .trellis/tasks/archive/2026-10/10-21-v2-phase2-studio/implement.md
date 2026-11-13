# V2 Phase 2: Pro Studio Workspace — Implementation Plan

## Step 1: Rust Native AI Matting & Studio Commands
1. Update `src-tauri/Cargo.toml` to include `ort` (`=2.0.0-rc.15` with `load-dynamic`, `directml`, `half`, `api-24`).
2. Copy `onnxruntime.dll`, `DirectML.dll`, and `onnxruntime_providers_shared.dll` into `D:\Sketch2Asset\bin\` so the standalone binary has zero external runtime dependencies.
3. Create `src-tauri/src/matting/mod.rs`:
   - Define the 6-model registry (`birefnet`, `ben2`, `birefnet-hrsod`, `birefnet-lite`, `isnet-general-use`, `isnetis`) resolving from `workspace_dir/models` and `app_data_dir/models`.
   - Implement lazy ONNX Runtime initialization (`ort::init_from`) and cached `Session` with `DirectML` $\rightarrow$ `CPU` execution provider fallback.
   - Implement `compose_rgba` in pure Rust (border median background estimation, optional enhanced high-chroma flood recovery, iterative 4-neighbor choke erosion, foreground color unmixing, and 4px 3×3 weighted color bleed).
   - Provide fallback connected-component color background removal if no ONNX model or runtime DLL is present.
   - Add unit tests for `compose_rgba` (choke, unmixing, and 4px color bleed).
4. Add Tauri commands in `src-tauri/src/commands/mod.rs` and register in `src-tauri/src/lib.rs`:
   - `list_matting_models`
   - `run_ai_matting`
   - `download_matting_model`
   - `save_studio_asset`
   - `import_external_images`
   - `read_image_base64` (for loading local disk assets into Studio's `ImageData` canvas cleanly without CORS taint)

## Step 2: Frontend Pixel Art Cleaner & Color Cutout Modules
1. Create `src/lib/imaging/pixel-clean.ts`:
   - Port `detectPixelBlockSize`, `quantizePalette` (K-Means++ + perceptual redmean distance), `PALETTE_PICO8`, `PALETTE_GAMEBOY`, and `cleanPixelArtCanvas`.
2. Create `src/lib/imaging/color-cutout.ts`:
   - Port `applyWandFloodFix` (Magic Wand restore/erase flood fill), `removeSolidBackground` (pure-color connected-component cutout + 3px edge unmixing), and `computeAlphaBounds` (for Auto-Trim).

## Step 3: Pro Studio UI (`src/features/studio/StudioView.svelte`) & Cross-View Integration
1. Build `src/features/studio/StudioView.svelte` following `.trellis/spec/frontend/design-system.md` (Bench zone rules: crisp 1px hairlines, fast micro-interactions, zero emojis, Lucide icons):
   - Top Workbench Bar: Active asset info (`width × height`, zoom level), tool buttons (`平移`, `点选恢复`, `点选擦除`, `笔刷恢复`, `笔刷擦除`, `框选裁剪`, `裁透明边`, `撤销`, `重做`, `适配视口`, `对比原图`), and quick actions (`导入外部图`, `保存到项目库`, `下载透明 PNG`).
   - Center Interactive Canvas Viewport: Checkerboard transparency backdrop, pan/zoom transform container, live brush radius circle cursor, crop marquee overlay with dimensions & `应用裁剪` pill, and full-viewport drag-and-drop file import overlay.
   - Bottom Filmstrip / Project Gallery Drawer: Switch between project assets, filter by project, or import external images.
   - Right Inspector Dock:
     - **AI 智能抠图与去白边**: Model selector (showing installed status & size), GPU/CPU engine badge, `物理反解去白边` (always active), `增强色彩补回` toggle, `边缘内缩 (Choke 0..5px)` slider, `一键 AI 抠图` & `纯色背景快抠` buttons.
     - **AI 像素纯化引擎 (Pixel Art Cleaner)**: Grid mode (`自动检测`, `32×32`, `64×64`, `128×128`, `256×256`, `4px 块`, `8px 块`, `16px 块`), Palette (`自动保真`, `16色`, `32色`, `64色`, `PICO-8`, `Game Boy`), `硬边缘透明化` toggle, `一键纯化色块` & `导出原生低清像素图` buttons.
     - **精修参数**: Magic Wand color tolerance (`5..140`) & Brush size (`2..120px`) sliders.
     - **分辨率无损导出**: Target Width (`W`), Height (`H`), `保持纵横比居中填充` toggle, quick size chips (`64`, `128`, `256`, `512`, `1024`), and `按尺寸导出 PNG` button.
2. Unlock the `工作台` tab in `src/lib/components/layout/TitleBar.svelte`, wire `StudioView` in `src/App.svelte`, and add **"送入工作台"** (`Send to Studio`) + **"一键像素纯化"** in `src/features/generate/GenerateView.svelte`.

## Step 4: Verification & Release Packaging
1. Run `pnpm check` and `pnpm build`.
2. Run `cargo test` and `cargo build --release --features tauri/custom-protocol`.
3. Copy `Sketch2Asset.exe`, `onnxruntime.dll`, `DirectML.dll`, and `onnxruntime_providers_shared.dll` to `D:\Sketch2Asset\bin\`.

## Step 5: User Feedback Refinements (V2.1)
1. **Button & Toolbar Anti-Deformation + Minimum Window Size**:
   - Enforce `minWidth: 1180`, `minHeight: 740` in `tauri.conf.json` and `lib.rs`.
   - Add `whitespace-nowrap shrink-0` to all toolbar, inspector, and action buttons so shrinking the window never causes vertical text wrapping or button deformation.
2. **Matting Model Management (Install / Uninstall) in Settings**:
   - Add `delete_matting_model(model_id)` in Rust (`matting/mod.rs` & `commands/mod.rs`) that evicts any cached session for that model and removes the `.onnx` file from disk.
   - Add a dedicated **AI Matting Model Hub** section in `SettingsView.svelte` allowing one-click Download/Install and Uninstall/Delete for each of the 6 ONNX models, plus opening the `models/` directory.
   - Simplify the Studio Matting Inspector model selector to list only installed models cleanly, with a quick button to jump to Settings for model management.
3. **High-Precision Edge-Phase Autocorrelation Pixel Art Cleaner (`pixel-clean.ts`)**:
   - Replace naive variance-based block detection with **1D Gradient Projection Folding + Phase Offset Alignment (`detectPixelGrid`)** that detects both the true pixel periodicity $S$ and phase offset $(\phi_x, \phi_y)$ with harmonic suppression.
   - Implement **Center-Core Weighted Mode Sampling** per cell (weighting the inner 60% of each cell 4× over anti-aliased cell borders) to preserve crisp 1px outlines and eliminate diffusion blur.
   - Add an interactive **Pixel Block Size (`px`) slider/stepper** alongside Auto Detection in `StudioView.svelte` so users can see the exact detected pixel block size (e.g. `8px → 128×128`) and fine-tune it if desired.

4. **Permanent Prevention of `localhost:1420` Connection Refused on Standalone `.exe`**:
   - Hard-enable `custom-protocol` (`tauri/custom-protocol`) as a default feature in `src-tauri/Cargo.toml` so `cargo build --release` never falls back to `devUrl` (`http://localhost:1420`).
   - Add `cargo:rerun-if-changed=../dist` in `src-tauri/build.rs` so updated frontend builds are always embedded into the executable.
5. **Settings Left-Rail Pagination (`providers` / `matting` / `system`)**:
   - Refactor `SettingsView.svelte` to use a left-side category navigation rail (`API 服务商与模型`, `本地 AI 抠图模型`, `外观与数据备份`) backed by `appStore.settingsTab`.
   - Connect `StudioView.svelte` "管理模型" shortcut to `appStore.openSettings('matting')` so it jumps directly to the Local AI Matting Models page.
6. **4-Stage Local AI Pixel Art Purification & Elastic Grid Rectification (`src/lib/imaging/pixel-clean.ts`)**:
   - **Stage 1 (`detectPixelGrid`)**: Run-Length Edge Span Histogram across horizontal & vertical color/alpha transitions with a 3-tap jitter kernel `[s-1, s, s+1]`, 2nd-harmonic reinforcement (`2*s`), and 5-tap sub-pixel centroid refinement. Independently detects `stepX` and `stepY` (allowing up to 20% aspect ratio drift between X and Y axes when AI generators stretch rows vs. columns).
   - **Stage 2 (`solveElasticCuts`)**: 1D Elastic Dynamic Programming seam alignment that flexes grid cut lines within `[0.72*S, 1.32*S]` to snap onto actual local pixel boundaries while maintaining spring regularity across flat regions.
   - **Stage 3**: Flat-interior dominant cluster sampling (ignoring outer 12% cell border and downweighting high-gradient anti-aliased edge pixels) + **1px Dark Outline Guard** (preserving dark outline clusters with weight $\ge 22\%$ and luminance contrast $> 65$).
   - **Stage 4 (`coalesceColorNoise`)**: 2-Pass Perceptual Palette Coalescing (`coalesceTolerance`, default `22`) that collapses noisy AI shades within the same color family into pure flat colors while keeping distinct tones separate, followed by single-cell salt-and-pepper denoise and **rectified uniform grid redraw** on `fullCanvas` and `nativeCanvas`.
   - **Non-Destructive Studio Integration (`StudioView.svelte`)**: Maintain `pixelBaseCanvas` so tweaking pixel block size, palette, or `色差收拢净化 (Coalesce)` re-runs cleanly from the pre-pixel-clean state without overwriting `origCanvas` (keeping `对比原图` intact), plus a live `显示网格线` overlay toggle.
7. **3-Part Closed-Loop Solution for AI Mixels & Fidelity Discrepancies**:
   - **Pre-Generation Anti-Mixel Guardrails & Virtual Grid Sketch Snapping (`presets.ts`, `GenerateView.svelte`, `pixel-clean.ts`)**:
     - Inject explicit `ZERO mixels (strictly uniform 1:1 square pixel grid, never mix sub-pixel resolutions for facial features or details)` into `pixel_sprite` and `style_pixel` prompts + automatic negative guardrails (`mixels, inconsistent pixel sizes, sub-pixel details`).
     - Add `snapDataUrlToVirtualGrid` to automatically snap reference/sketch images onto a strict 32×32 or 64×64 virtual pixel grid before sending to the AI generator in pixel mode.
   - **High-Fidelity Purification (`pixel-clean.ts`)**:
     - Constrain elastic cut drift (`maxDrift <= 0.48 * actualStep`, `springWeight = 0.85`) to prevent geometric warping when rectifying onto a uniform grid.
     - Add `detailMode: 'standard' | 'fine2x'` (`1× 标准规整网格` vs `2× 倍频保真 (防吞五官)`) to capture half-block AI sub-pixel facial features and highlights when desired.
     - Protect high-contrast micro-clusters (eye catchlights, pupils, gem glints) from being swallowed during palette coalescing (`Math.abs(clLum - pLum) <= 28`), and tune dark outline threshold (`34%`) so 1px outlines never bloat.
   - **Post-Purification Studio $\rightarrow$ Forge Refinement Loop (`StudioView.svelte`)**:
     - Add `送回生图工坊垫图重绘 (二次细化)` button in Studio to send the purified/edited pixel canvas back to `GenerateView` as a grid-aligned reference image for iterative AI refinement.
