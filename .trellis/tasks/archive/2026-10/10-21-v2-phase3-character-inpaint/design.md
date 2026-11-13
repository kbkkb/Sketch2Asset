# V3 Phase 3A: Studio Annotated Region Edit (工作台多框选批注局部重绘) — Design

## 1. Interaction & UI Architecture (`StudioView.svelte`)

1. **Tool Activation (`activeTool === 'annotate'`)**:
   - Add a dedicated **「批注修改 (`annotate`)」** tool button in the top Studio toolbar (icon: `MessageSquarePlus`, shortcut: `A`), showing a live badge of active annotation regions (`annotations.length`).
   - When `activeTool === 'annotate'` is active, dragging on the canvas creates a numbered `RegionAnnotation` (`#1`, `#2`, `#3`...) with distinct high-contrast colors (`#00E5FF`, `#FF3366`, `#FFB800`, `#A855F7`, `#10B981`).
   - Clicking an existing box selects it; dragging inside a selected box moves it; dragging its bottom-right corner handle resizes it.
   - A floating popover appears directly adjacent to the currently selected box (`activeAnnotationId`) with:
     - Auto-focused note input (`Enter` to confirm & draw next box, `Ctrl+Enter` to immediately run AI Region Edit)
     - Quick-fill prompt chips (`移除该物体`, `修补破损细节`, `更换材质/颜色`, `增加发光特效`)
     - Delete button (`Trash2` / `X`).

2. **Right Inspector Panel — Annotated Region Edit Card**:
   - Positioned prominently in the right inspector dock (auto-expanded and highlighted when `activeTool === 'annotate'` or `annotations.length > 0`).
   - **Model Picker**: Embeds `ProviderModelPicker` so users can choose or switch the AI model directly inside Studio.
   - **Guidance Mode Selector**:
     - `visual_guide` (智能双图视觉引导 — 默认推荐，支持 Gemini / GPT-4o / Nano-Banana / Seedream 等全部多模态与垫图接口)
     - `mask_edit` (标准 Mask 遮罩重绘 — 针对支持 `/v1/images/edits` `mask` 参数的接口)
   - **Post-Merge Strategy (`blendMode`)**:
     - `full` (采用完整生成图 — 适合造型延展超出选框边界的修改，支持一键切换对比)
     - `patch` (仅羽化回贴选框区域 — 选框外 100% 锁定原图像素与透明底不变，自动在选框边缘做柔边融合)
     - Moreover, once AI returns the result image, cache `lastRegionEditFullCanvas` so the user can toggle between **「整图结果」** and **「仅回贴选框区域」** in real time without re-calling the API!
   - **Annotation List & Global Constraint**:
     - Interactive list of `#1`, `#2`... boxes with inline text inputs and delete actions.
     - Optional global style/consistency constraint input (e.g. `保持像素画风与光影一致`).

---

## 2. Backend Dual-Mode Engine (`src-tauri/src/providers/mod.rs` & `commands/mod.rs`)

1. **Tauri Command `run_studio_region_edit`**:
   - Accepts `RegionEditPayload`:
     - `profile_id`: String
     - `model`: String
     - `mode`: String (`"visual_guide"` | `"mask_edit"`)
     - `original_data_url`: String (clean source image composited on white if transparent)
     - `guide_data_url`: String (source image with numbered `#1`, `#2` boxes rendered on top)
     - `mask_data_url`: String (RGBA PNG where inside boxes `alpha = 0` and outside `alpha = 255`)
     - `annotations`: `Vec<RegionAnnotationInput>` (`index`, `x`, `y`, `w`, `h`, `note`)
     - `global_note`: String
     - `size`: String
2. **Prompt Synthesis**:
   - Constructs a structured region-editing prompt listing each numbered box (`Region #1 (at x,y,w,h): <note>`) and explicitly instructing the model to keep all unmarked regions identical and output a clean image without bounding boxes or labels.
3. **Provider Execution & Automatic Fallback**:
   - In `chat` mode (or when `mode == "visual_guide"` on chat models): sends **both** Image 1 (clean original) and Image 2 (annotated guide with `#1`, `#2` boxes) in the multimodal `messages` payload.
   - In `images` mode:
     - If `mode == "mask_edit"`, sends `image` (clean original) + `mask` (alpha mask PNG) to `/v1/images/edits`.
     - If upstream rejects `mask` or `mode == "visual_guide"`, sends `guide_bytes` or `original_bytes` to `/v1/images/edits`, with fallback to `/v1/chat/completions`.
