# V3 Implementation Plan

## Part 1: Studio Annotated Region Edit (工作台局部批注修改重绘)

### Step 1: Rust Backend Dual-Mode Region Edit Engine
1. Update `src-tauri/src/providers/mod.rs`:
   - Add `edit_regions_via_chat` supporting dual-image multimodal input (Image 1: clean original, Image 2: numbered visual guide) and single-image fallback.
   - Add `edit_regions_via_images` supporting `/v1/images/edits` with optional `mask` part (`mask.png`), falling back to visual guide `/v1/images/edits` and `/v1/chat/completions` if upstream does not support `mask`.
2. Update `src-tauri/src/commands/mod.rs` and `src-tauri/src/lib.rs`:
   - Define `RegionAnnotationItem`, `RegionEditPayload`, and `RegionEditResponse` (`image_data_url`, `via`, `elapsed_ms`).
   - Implement and register Tauri command `run_studio_region_edit`.

### Step 2: Frontend API, i18n & Helper Canvas Builders
1. Update `src/lib/api/types.ts` and `src/lib/api/index.ts` with `RegionAnnotationItem`, `RegionEditPayload`, `RegionEditResponse`, and `api.runStudioRegionEdit(payload)`.
2. Update `src/lib/i18n/index.svelte.ts` with bilingual keys for Annotated Region Edit.

### Step 3: Interactive Canvas Overlay & Inspector UI (`StudioView.svelte`)
1. Add `'annotate'` to `ToolKind` in `StudioView.svelte` with top toolbar button (`MessageSquarePlus` icon, shortcut `A`, badge count).
2. Implement multi-box drawing, selection, dragging (move), bottom-right resize handle, and auto-numbered `#1, #2, #3...` color-coded overlays on the viewport.
3. Implement a floating popover attached to the active region box with auto-focused text input, quick-fill prompt chips (`移除该物体`, `修补细节`, `更换材质配色`, `增加发光特效`), and delete button.
4. Add the **「局部批注重绘 (Annotated Region Edit)」** inspector card in the right dock:
   - Embedded `ProviderModelPicker` for selecting the AI model directly inside Studio.
   - Guidance mode selector (`智能双图视觉引导` vs `Mask 遮罩重绘`).
   - Real-time post-merge toggle (`整图采用` vs `仅羽化回贴选框区域 (锁定框外像素与透明底)`).
   - Interactive list of all `#1..#N` annotations, global constraint input, clear-all button, and primary `一键执行批注修改` action button.
5. Verify with `pnpm check`, `pnpm build`, `cargo test`, and `cargo build --release --features tauri/custom-protocol`.

---

## Part 2: Character Forge Workspace (角色工坊 V3 — 三视图 / 锁定特征换装 / 序列帧动画与重心对齐)

### Step 1: Rust Backend Multi-Reference Character Generation Command
1. Add `CharacterGeneratePayload` and `generate_character_asset` in `src-tauri/src/commands/mod.rs` and register in `src-tauri/src/lib.rs`:
   - Supports `sub_mode`: `"turnaround"` | `"outfit"` | `"spritesheet"`.
   - Supports up to 2 reference images (`anchor_data_url` as Image 1 identity anchor, `secondary_ref_data_url` as Image 2 outfit/pose reference) via `providers::edit_regions_via_chat` / `providers::edit_regions_via_images` or `generate_via_chat` / `generate_via_images`.
   - Automatically saves the generated sheet into the active project library (`character_card` or `pixel_sprite`) and returns `CharacterGenerateResponse` (`asset`, `image_data_url`, `elapsed_ms`, `via`).
2. Expose `api.generateCharacterAsset(payload)` in `src/lib/api/types.ts` and `src/lib/api/index.ts`.

### Step 2: Sprite Sheet Auto-Slicing & Bottom-Center Pivot Alignment Engine (`src/lib/imaging/spritesheet.ts`)
1. Create `src/lib/imaging/spritesheet.ts`:
   - `detectTurnaroundSplits(imgData, w, h)`: Automatically finds the two cleanest vertical separator columns around `1/3` and `2/3` of the canvas width so Front / Side / Back views can be sliced without clipping arms or weapons.
   - `sliceTurnaroundViews(sourceCanvas, split1Ratio, split2Ratio, autoCutoutBg)`: Slices the turnaround sheet into 3 standalone canvases (`Front`, `Side`, `Back`) with optional solid-background removal + 3px edge unmixing.
   - `sliceAndAlignSpriteSheet(sourceCanvas, opts)`:
     - Slices an $R \times C$ grid (`rows`, `cols`) into individual frames.
     - Optional one-click solid background removal (`removeBg`) and/or Pixel Art Purification (`pixelClean`).
     - **Bottom-Center Pivot Alignment (`alignPivot`)**: Computes each frame's opaque bounding box `(minX, minY, maxX, maxY)`, aligns every frame's bottom-most foot pixel `maxY` to a unified ground baseline (`baselineRatio = 0.88`) and horizontal center `(minX + maxX) / 2` to `frameW / 2`, plus supports per-frame manual `(dx, dy)` offset tweaks and frame enable/disable toggles.
     - `composeAlignedSpriteSheet(frames, cols, frameW, frameH)`: Re-packs the aligned frames into a clean, uniform-grid Sprite Sheet PNG ready for Unity / Godot / Unreal.

### Step 3: Character Forge UI (`src/features/character/CharacterView.svelte`) & Navigation Unlock
1. Unlock the `角色工坊` tab in `src/lib/components/layout/TitleBar.svelte`, add `'character'` to `appStore.activeView`, and mount `CharacterView` in `src/App.svelte`.
2. Build `src/features/character/CharacterView.svelte` with 3 sub-workbenches switchable via a top segmented mode bar:
   - **Mode 1: 角色设定与三视图 (Turnaround & Concept Sheet)**:
     - Layout preset (`正/侧/背标准三视图 1536×1024` vs `全身基准立绘 1024×1536`), art style selector, pose/proportion modifier chips (`标准 A-Pose`, `绑定 T-Pose`, `Q版二头身`, `日系七头身`, `分层材质设定`).
     - Interactive canvas with draggable `Front | Side | Back` vertical split lines (`智能寻边重置`), one-click `切分正/侧/背并存入图库`, `设为角色基准锚点`, and `送入工作台精修`.
   - **Mode 2: 锁定特征换装派生 (Identity-Locked Outfit Variant)**:
     - Dual reference slots: **Slot 1: 角色身份锚点 (Identity Anchor)** + **Slot 2: 服装款式参考图 (Optional Outfit Ref)**.
     - Lock mode selector (`严格锁死五官体型与三视图排版` / `锁定角色特征生成新换装立绘`), quick outfit theme chips, and side-by-side `基准锚点 vs 换装派生` comparison view on canvas.
   - **Mode 3: 序列帧生成与脚底对齐预览器 (Sprite Sheet & Pivot Animation Studio)**:
     - Action preset (`待机呼吸 Idle`, `行走循环 Walk`, `奔跑 Run`, `挥砍攻击 Attack`, `施法 Cast`, `跳跃 Jump`), grid selector (`2×2`, `1×4`, `2×3`, `2×4`, `3×3`).
     - Dual-stage viewport: Left/Top shows the sliced Sprite Sheet grid, Right/Center shows the **Live Animation Previewer** (`2 ~ 24 FPS`, Play/Pause, Ping-Pong loop, Onion Skinning, Ground Baseline guide).
     - Automatic processing controls: `一键去背景白边`, `自动脚底重心对齐 (消除晃动)`, `像素纯化统一色板`, per-frame `(ΔX, ΔY)` nudge controls, and `导出对齐后标准精灵表 PNG`.

