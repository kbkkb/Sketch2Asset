<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import {
    Play,
    Pause,
    Plus,
    Trash2,
    Save,
    RotateCcw,
    RotateCw,
    Undo2,
    Redo2,
    FlipHorizontal2,
    Camera,
    Sparkles,
    Copy,
    Download,
    Bone,
    Eye,
    Move3d
  } from '@lucide/svelte';
  import { api } from '$lib/api';
  import type { PoseActionClip, PoseKeyframe } from '$lib/api/types';
  import { appStore } from '$lib/stores/settings.svelte';
  import { i18n } from '$lib/i18n/index.svelte';
  import {
    BONE_LIST,
    BUILTIN_POSE_CLIPS,
    CAMERA_PRESETS,
    PROPORTION_PRESETS,
    WEAPON_TYPES,
    Pose3DRigScene,
    cloneClip,
    interpolatePoseAtTime,
    renderClipToSpriteSheetDataUrl,
    type BoneId,
    type GizmoAxis,
    type GizmoMode
  } from './pose3dEngine';

  interface Props {
    onApplyToSpriteStudio?: (
      dataUrl: string,
      rows: number,
      cols: number,
      actionName: string,
      actionId: string
    ) => void;
    externalMcpClip?: PoseActionClip | null;
  }

  let { onApplyToSpriteStudio, externalMcpClip = null }: Props = $props();

  let actionClips = $state<PoseActionClip[]>(BUILTIN_POSE_CLIPS.map(cloneClip));
  let selectedActionId = $state<string>('attack');
  let workingClip = $state<PoseActionClip>(
    cloneClip(BUILTIN_POSE_CLIPS.find((c) => c.id === 'attack') ?? BUILTIN_POSE_CLIPS[0]!)
  );

  let selectedBone = $state<BoneId>('r_upper_arm');
  let currentTime = $state<number>(0);
  let isPlaying = $state<boolean>(false);
  let playbackSpeed = $state<number>(1.0);
  let viewportDragMode = $state<'orbit' | 'bone'>('bone');
  let gizmoMode = $state<GizmoMode>('rotate');

  // Undo / Redo Stacks (max 50 snapshots)
  const MAX_UNDO_STEPS = 50;
  let undoStack = $state<PoseActionClip[]>([]);
  let redoStack = $state<PoseActionClip[]>([]);

  let sheetRows = $state<number>(2);
  let sheetCols = $state<number>(3);
  let previewSheetDataUrl = $state<string>('');
  let isSavingClip = $state<boolean>(false);

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let viewportWrapEl = $state<HTMLDivElement | null>(null);
  let rigScene: Pose3DRigScene | null = null;
  let rafId: number | null = null;
  let lastTickMs = 0;

  let pointerDown = false;
  let pointerMode: 'orbit' | 'pan' | 'bone' | 'gizmo' = 'orbit';
  let activeGizmoAxis = $state<GizmoAxis | null>(null);
  let hoveredGizmoAxis = $state<GizmoAxis | null>(null);
  let pointerStart = {
    x: 0,
    y: 0,
    yaw: -90,
    pitch: 0,
    panX: 0,
    panY: 0,
    rotX: 0,
    rotY: 0,
    rotZ: 0,
    rootX: 0,
    rootY: 0,
    rootZ: 0,
    tangent: { tx: 1, ty: 0, centerPx: [320, 240] as [number, number], axisSign: 1 },
    moved: false
  };

  function normalizeYaw(yaw: number): number {
    let n = (((yaw + 180) % 360) + 360) % 360 - 180;
    if (n === -180 && yaw > 0) n = 180;
    return Math.round(n);
  }

  function pushUndoSnapshot() {
    undoStack = [...undoStack.slice(-(MAX_UNDO_STEPS - 1)), cloneClip(workingClip)];
    redoStack = [];
  }

  function handleUndo() {
    if (undoStack.length === 0) return;
    const prev = undoStack[undoStack.length - 1]!;
    undoStack = undoStack.slice(0, -1);
    redoStack = [...redoStack.slice(-(MAX_UNDO_STEPS - 1)), cloneClip(workingClip)];
    workingClip = cloneClip(prev);
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleRedo() {
    if (redoStack.length === 0) return;
    const next = redoStack[redoStack.length - 1]!;
    redoStack = redoStack.slice(0, -1);
    undoStack = [...undoStack.slice(-(MAX_UNDO_STEPS - 1)), cloneClip(workingClip)];
    workingClip = cloneClip(next);
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement | null;
    const isTextInput =
      target &&
      ((target.tagName === 'INPUT' &&
        (target as HTMLInputElement).type !== 'range' &&
        (target as HTMLInputElement).type !== 'checkbox') ||
        target.tagName === 'TEXTAREA' ||
        target.isContentEditable);

    if (isTextInput) return;

    const key = e.key.toLowerCase();
    if ((e.ctrlKey || e.metaKey) && key === 'z') {
      e.preventDefault();
      e.stopPropagation();
      if (e.shiftKey) {
        handleRedo();
      } else {
        handleUndo();
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && key === 'y') {
      e.preventDefault();
      e.stopPropagation();
      handleRedo();
      return;
    }

    if (!e.ctrlKey && !e.metaKey && !e.altKey) {
      if (key === 'e') {
        gizmoMode = 'rotate';
        viewportDragMode = 'bone';
        syncSceneAndRender();
      } else if (key === 'w') {
        gizmoMode = 'translate';
        viewportDragMode = 'bone';
        syncSceneAndRender();
      }
    }
  }

  let currentPose = $derived(interpolatePoseAtTime(workingClip, currentTime));
  let currentBoneEuler = $derived< [number, number, number] >(
    currentPose.bones[selectedBone] ?? [0, 0, 0]
  );
  let currentRootPos = $derived<[number, number, number]>(currentPose.rootPos ?? [0, 0, 0]);

  let activeKeyframeIdx = $derived(
    workingClip.keyframes.findIndex((k) => Math.abs(k.t - currentTime) < 0.015)
  );

  onMount(() => {
    if (canvasEl) {
      const w = viewportWrapEl?.clientWidth || 640;
      const h = viewportWrapEl?.clientHeight || 480;
      rigScene = new Pose3DRigScene(canvasEl, w, h, false);
      rigScene.setSelectedBone(selectedBone);
      syncSceneAndRender();
      refreshSheetPreview();
    }

    void loadBackendActions();

    const onResize = () => {
      if (!rigScene || !viewportWrapEl) return;
      const w = Math.max(320, viewportWrapEl.clientWidth);
      const h = Math.max(260, viewportWrapEl.clientHeight);
      rigScene.resize(w, h, workingClip.camera);
      rigScene.render();
    };
    window.addEventListener('resize', onResize);

    const tick = (now: number) => {
      if (isPlaying) {
        const dt = lastTickMs ? Math.min(0.1, (now - lastTickMs) / 1000) : 0.016;
        let nextT = currentTime + dt * 0.65 * playbackSpeed;
        if (nextT > 1) {
          if (workingClip.loopAnim) {
            nextT = nextT % 1;
          } else {
            nextT = 1;
            isPlaying = false;
          }
        }
        currentTime = Number(nextT.toFixed(3));
        syncSceneAndRender();
      }
      lastTickMs = now;
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);

    return () => {
      window.removeEventListener('resize', onResize);
    };
  });

  onDestroy(() => {
    if (rafId !== null) cancelAnimationFrame(rafId);
    rigScene?.dispose();
  });

  $effect(() => {
    if (externalMcpClip) {
      workingClip = cloneClip(externalMcpClip);
      selectedActionId = externalMcpClip.id;
      sheetRows = externalMcpClip.defaultRows || 2;
      sheetCols = externalMcpClip.defaultCols || 3;
      currentTime = 0;
      syncSceneAndRender();
      refreshSheetPreview();
    }
  });

  async function loadBackendActions() {
    try {
      const remote = await api.listPoseActions();
      if (Array.isArray(remote) && remote.length > 0) {
        const merged = remote.map((clip) => {
          const c = cloneClip(clip);
          let migrated = false;
          if (
            c.builtin &&
            c.camera?.preset === 'side_2d' &&
            Math.abs((c.camera.yaw ?? 0) - 90) < 1
          ) {
            c.camera.yaw = -90;
            migrated = true;
          }
          if (
            c.builtin &&
            c.id === 'jump' &&
            c.keyframes?.some((k) => (k.rootPos?.[1] ?? 0) > 0.2)
          ) {
            const builtinJump = BUILTIN_POSE_CLIPS.find((b) => b.id === 'jump');
            if (builtinJump) {
              c.name = builtinJump.name;
              c.nameEn = builtinJump.nameEn;
              c.keyframes = cloneClip(builtinJump).keyframes;
              migrated = true;
            }
          }
          if (migrated) {
            void api.savePoseAction(c).catch(() => {});
          }
          return c;
        });
        for (const b of BUILTIN_POSE_CLIPS) {
          if (!merged.some((m) => m.id === b.id)) {
            merged.push(cloneClip(b));
          }
        }
        actionClips = merged;
        const cur = merged.find((c) => c.id === selectedActionId) ?? merged[0]!;
        selectActionClip(cur.id, false);
      }
    } catch {
      // Fallback to BUILTIN_POSE_CLIPS when backend command is not yet ready
    }
  }

  function selectActionClip(id: string, updatePreview = true) {
    const found =
      actionClips.find((c) => c.id === id) ??
      BUILTIN_POSE_CLIPS.find((c) => c.id === id) ??
      actionClips[0];
    if (!found) return;
    selectedActionId = found.id;
    workingClip = cloneClip(found);
    undoStack = [];
    redoStack = [];
    rigScene?.resetPan();
    sheetRows = workingClip.defaultRows || 2;
    sheetCols = workingClip.defaultCols || 3;
    currentTime = 0;
    isPlaying = false;
    syncSceneAndRender();
    if (updatePreview) refreshSheetPreview();
  }

  function syncSceneAndRender() {
    if (!rigScene || !viewportWrapEl) return;
    const w = Math.max(320, viewportWrapEl.clientWidth);
    const h = Math.max(260, viewportWrapEl.clientHeight);
    rigScene.setSelectedBone(selectedBone);
    rigScene.setGizmoMode(gizmoMode);
    rigScene.applyProportion(workingClip.proportion);
    rigScene.applyCamera(workingClip.camera, w, h);
    rigScene.applyPose(interpolatePoseAtTime(workingClip, currentTime));
    rigScene.render();
  }

  function refreshSheetPreview() {
    try {
      previewSheetDataUrl = renderClipToSpriteSheetDataUrl(
        workingClip,
        sheetRows,
        sheetCols,
        960,
        640
      );
    } catch {
      // ignore
    }
  }

  function ensureKeyframeAtCurrentTime(): PoseKeyframe {
    isPlaying = false;
    const existingIdx = workingClip.keyframes.findIndex(
      (k) => Math.abs(k.t - currentTime) < 0.015
    );
    if (existingIdx >= 0) {
      return workingClip.keyframes[existingIdx]!;
    }
    const interp = interpolatePoseAtTime(workingClip, currentTime);
    const newKf: PoseKeyframe = {
      t: Number(currentTime.toFixed(3)),
      rootPos: [...interp.rootPos] as [number, number, number],
      bones: { ...interp.bones }
    };
    workingClip.keyframes = [...workingClip.keyframes, newKf].sort((a, b) => a.t - b.t);
    const inserted = workingClip.keyframes.find((k) => Math.abs(k.t - newKf.t) < 0.002)!;
    return inserted;
  }

  function updateSelectedBoneAxis(axis: 0 | 1 | 2, degValue: number) {
    const kf = ensureKeyframeAtCurrentTime();
    const cur = [...(kf.bones[selectedBone] ?? currentBoneEuler)] as [number, number, number];
    cur[axis] = Math.max(-180, Math.min(180, Math.round(degValue)));
    kf.bones = { ...kf.bones, [selectedBone]: cur };
    workingClip.keyframes = [...workingClip.keyframes];
    syncSceneAndRender();
  }

  function updateRootPosAxis(axis: 0 | 1 | 2, val: number) {
    const kf = ensureKeyframeAtCurrentTime();
    const cur = [...(kf.rootPos ?? currentRootPos)] as [number, number, number];
    cur[axis] = Number(val.toFixed(2));
    kf.rootPos = cur;
    workingClip.keyframes = [...workingClip.keyframes];
    syncSceneAndRender();
  }

  function handleAddKeyframe() {
    pushUndoSnapshot();
    ensureKeyframeAtCurrentTime();
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleDeleteKeyframe() {
    if (workingClip.keyframes.length <= 1) return;
    if (activeKeyframeIdx >= 0) {
      pushUndoSnapshot();
      workingClip.keyframes = workingClip.keyframes.filter((_, i) => i !== activeKeyframeIdx);
      syncSceneAndRender();
      refreshSheetPreview();
    }
  }

  function handleCopyFirstFrameToEnd() {
    if (workingClip.keyframes.length === 0) return;
    pushUndoSnapshot();
    const sorted = [...workingClip.keyframes].sort((a, b) => a.t - b.t);
    const first = sorted[0]!;
    const endCopy: PoseKeyframe = {
      t: 1.0,
      rootPos: [...first.rootPos] as [number, number, number],
      bones: JSON.parse(JSON.stringify(first.bones))
    };
    const filtered = sorted.filter((k) => Math.abs(k.t - 1.0) > 0.015);
    workingClip.keyframes = [...filtered, endCopy];
    currentTime = 1.0;
    syncSceneAndRender();
    refreshSheetPreview();
    appStore.toast(
      'ok',
      i18n.locale === 'en'
        ? 'Copied first keyframe to t=1.0 for seamless loop'
        : '已将首帧姿态复制到末帧 (t=1.0) 实现无缝循环闭合'
    );
  }

  function handleResetClip() {
    const orig =
      BUILTIN_POSE_CLIPS.find((c) => c.id === workingClip.id) ??
      actionClips.find((c) => c.id === workingClip.id);
    if (orig) {
      pushUndoSnapshot();
      workingClip = cloneClip(orig);
      rigScene?.resetPan();
      currentTime = 0;
      syncSceneAndRender();
      refreshSheetPreview();
    }
  }

  async function handleSaveActionClip() {
    if (isSavingClip) return;
    isSavingClip = true;
    try {
      workingClip.defaultRows = sheetRows;
      workingClip.defaultCols = sheetCols;
      const saved = await api.savePoseAction(workingClip);
      const existsIdx = actionClips.findIndex((c) => c.id === saved.id);
      if (existsIdx >= 0) {
        actionClips[existsIdx] = cloneClip(saved);
      } else {
        actionClips = [...actionClips, cloneClip(saved)];
      }
      workingClip = cloneClip(saved);
      refreshSheetPreview();
      appStore.toast(
        'ok',
        i18n.locale === 'en' ? `Saved 3D action "${saved.name}"` : `已保存 3D 动作「${saved.name}」`
      );
    } catch (e: any) {
      appStore.toast('err', e?.message ?? '保存动作失败');
    } finally {
      isSavingClip = false;
    }
  }

  async function handleSaveAsCustomAction() {
    const customName = prompt(
      i18n.locale === 'en' ? 'Enter new custom action name:' : '请输入自定义动作名称：',
      `${workingClip.name} (自定义)`
    );
    if (!customName || !customName.trim()) return;
    const customId = `custom_${Date.now().toString(36)}`;
    workingClip = {
      ...cloneClip(workingClip),
      id: customId,
      name: customName.trim(),
      nameEn: customName.trim(),
      builtin: false,
      defaultRows: sheetRows,
      defaultCols: sheetCols
    };
    selectedActionId = customId;
    await handleSaveActionClip();
  }

  async function handleDeleteCustomAction() {
    if (workingClip.builtin) return;
    try {
      await api.deletePoseAction(workingClip.id);
      actionClips = actionClips.filter((c) => c.id !== workingClip.id);
      selectActionClip(actionClips[0]?.id ?? 'idle');
      appStore.toast('ok', i18n.locale === 'en' ? 'Deleted custom action' : '已删除自定义动作');
    } catch (e: any) {
      appStore.toast('err', e?.message ?? '删除失败');
    }
  }

  function handleCameraPreset(presetId: string) {
    const p = CAMERA_PRESETS.find((c) => c.id === presetId);
    if (!p) return;
    pushUndoSnapshot();
    rigScene?.resetPan();
    workingClip.camera = {
      preset: p.id,
      yaw: p.yaw,
      pitch: p.pitch,
      ortho: p.ortho,
      zoom: p.zoom
    };
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleFlipCameraDirection() {
    pushUndoSnapshot();
    const curYaw = workingClip.camera.yaw || -90;
    const flippedYaw = normalizeYaw(-curYaw);
    workingClip.camera.yaw = flippedYaw;
    if (Math.abs(flippedYaw - -90) < 1 && Math.abs(workingClip.camera.pitch) < 1) {
      workingClip.camera.preset = 'side_2d';
    } else if (Math.abs(flippedYaw - 90) < 1 && Math.abs(workingClip.camera.pitch) < 1) {
      workingClip.camera.preset = 'side_2d_left';
    } else {
      workingClip.camera.preset = 'custom';
    }
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleProportionPreset(presetId: string) {
    const p = PROPORTION_PRESETS.find((x) => x.id === presetId);
    if (!p) return;
    pushUndoSnapshot();
    workingClip.proportion = {
      ...workingClip.proportion,
      preset: p.id,
      headScale: p.headScale,
      torsoScale: p.torsoScale,
      limbScale: p.limbScale
    };
    syncSceneAndRender();
    refreshSheetPreview();
  }

  function handleViewportPointerDown(e: PointerEvent) {
    if (!rigScene || !canvasEl) return;
    const rect = canvasEl.getBoundingClientRect();
    const ndcX = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    const ndcY = -((e.clientY - rect.top) / rect.height) * 2 + 1;

    // Middle-click or Shift+Right-click pans camera target
    if (e.button === 1 || (e.button === 2 && e.shiftKey)) {
      pointerDown = true;
      pointerMode = 'pan';
      activeGizmoAxis = null;
      pointerStart = {
        ...pointerStart,
        x: e.clientX,
        y: e.clientY,
        panX: rigScene.panX,
        panY: rigScene.panY,
        moved: false
      };
      canvasEl.setPointerCapture(e.pointerId);
      return;
    }

    // Right-click or Alt+drag always orbits camera
    if (e.button === 2 || e.altKey) {
      pointerDown = true;
      pointerMode = 'orbit';
      activeGizmoAxis = null;
      pointerStart = {
        ...pointerStart,
        x: e.clientX,
        y: e.clientY,
        yaw: workingClip.camera.yaw,
        pitch: workingClip.camera.pitch,
        moved: false
      };
      canvasEl.setPointerCapture(e.pointerId);
      return;
    }

    // Left-click: 1) Check 3D Gizmo axis hit first
    const hitGizmo = rigScene.pickGizmoAxis(ndcX, ndcY);
    if (hitGizmo) {
      pushUndoSnapshot();
      pointerDown = true;
      pointerMode = 'gizmo';
      activeGizmoAxis = hitGizmo;
      hoveredGizmoAxis = hitGizmo;
      rigScene.setHoveredGizmoAxis(hitGizmo);
      const tangent = rigScene.getAxisScreenTangent(
        hitGizmo,
        rigScene.lastGizmoHitPoint ?? undefined,
        rect.width,
        rect.height
      );
      const euler = currentPose.bones[selectedBone] ?? [0, 0, 0];
      const rPos = currentPose.rootPos ?? [0, 0, 0];
      pointerStart = {
        x: e.clientX,
        y: e.clientY,
        yaw: workingClip.camera.yaw,
        pitch: workingClip.camera.pitch,
        panX: rigScene.panX,
        panY: rigScene.panY,
        rotX: euler[0] ?? 0,
        rotY: euler[1] ?? 0,
        rotZ: euler[2] ?? 0,
        rootX: rPos[0] ?? 0,
        rootY: rPos[1] ?? 0,
        rootZ: rPos[2] ?? 0,
        tangent,
        moved: false
      };
      rigScene.render();
      canvasEl.setPointerCapture(e.pointerId);
      return;
    }

    // 2) Check bone mesh hit
    const picked = rigScene.pickBone(ndcX, ndcY);
    if (picked) {
      selectedBone = picked;
      rigScene.setSelectedBone(picked);
      rigScene.render();
    }

    pointerDown = true;
    activeGizmoAxis = null;

    // If user clicked on a bone in 'bone' mode (or holding Shift), drag the bone;
    // If user clicked on EMPTY SPACE (neither Gizmo nor bone hit), automatically orbit camera!
    if (picked && (viewportDragMode === 'bone' || e.shiftKey)) {
      pushUndoSnapshot();
      pointerMode = 'bone';
    } else {
      pointerMode = 'orbit';
    }

    const euler = currentPose.bones[selectedBone] ?? [0, 0, 0];
    const rPos = currentPose.rootPos ?? [0, 0, 0];
    pointerStart = {
      x: e.clientX,
      y: e.clientY,
      yaw: workingClip.camera.yaw,
      pitch: workingClip.camera.pitch,
      panX: rigScene.panX,
      panY: rigScene.panY,
      rotX: euler[0] ?? 0,
      rotY: euler[1] ?? 0,
      rotZ: euler[2] ?? 0,
      rootX: rPos[0] ?? 0,
      rootY: rPos[1] ?? 0,
      rootZ: rPos[2] ?? 0,
      tangent: { tx: 1, ty: 0, centerPx: [rect.width * 0.5, rect.height * 0.5], axisSign: 1 },
      moved: false
    };
    canvasEl.setPointerCapture(e.pointerId);
  }

  function handleViewportPointerMove(e: PointerEvent) {
    if (!rigScene || !canvasEl) return;

    if (!pointerDown) {
      const rect = canvasEl.getBoundingClientRect();
      const ndcX = ((e.clientX - rect.left) / rect.width) * 2 - 1;
      const ndcY = -((e.clientY - rect.top) / rect.height) * 2 + 1;
      const hitAxis = rigScene.pickGizmoAxis(ndcX, ndcY);
      if (hitAxis !== hoveredGizmoAxis) {
        hoveredGizmoAxis = hitAxis;
        rigScene.setHoveredGizmoAxis(hitAxis);
        rigScene.render();
      }
      return;
    }

    const dx = e.clientX - pointerStart.x;
    const dy = e.clientY - pointerStart.y;
    if (Math.abs(dx) > 2 || Math.abs(dy) > 2) {
      pointerStart.moved = true;
    }

    if (pointerMode === 'pan') {
      const zoom = Math.max(0.35, workingClip.camera.zoom || 1.0);
      const scale = 0.0045 / zoom;
      rigScene.panX = Number((pointerStart.panX - dx * scale).toFixed(3));
      rigScene.panY = Number((pointerStart.panY + dy * scale).toFixed(3));
      syncSceneAndRender();
      return;
    }

    if (pointerMode === 'orbit') {
      workingClip.camera.preset = 'custom';
      workingClip.camera.yaw = normalizeYaw(pointerStart.yaw - dx * 0.45);
      workingClip.camera.pitch = Math.max(-85, Math.min(85, Math.round(pointerStart.pitch + dy * 0.35)));
      syncSceneAndRender();
      return;
    }

    if (pointerMode === 'gizmo' && activeGizmoAxis) {
      const kf = ensureKeyframeAtCurrentTime();
      const { tx, ty } = pointerStart.tangent;
      const projected = dx * tx + dy * ty;

      if (activeGizmoAxis === 'tx' || activeGizmoAxis === 'ty' || activeGizmoAxis === 'tz') {
        const curRoot: [number, number, number] = [
          pointerStart.rootX,
          pointerStart.rootY,
          pointerStart.rootZ
        ];
        const idx = activeGizmoAxis === 'tx' ? 0 : activeGizmoAxis === 'ty' ? 1 : 2;
        const nextVal = Math.max(-0.85, Math.min(0.85, curRoot[idx] + projected * 0.0045));
        curRoot[idx] = Number(nextVal.toFixed(2));
        kf.rootPos = curRoot;
        workingClip.keyframes = [...workingClip.keyframes];
        syncSceneAndRender();
        return;
      }

      const deltaDeg = projected * 0.75;
      const clampDeg = (v: number) => Math.max(-180, Math.min(180, Math.round(v)));
      const nextEuler: [number, number, number] = [
        pointerStart.rotX,
        pointerStart.rotY,
        pointerStart.rotZ
      ];

      if (activeGizmoAxis === 'x') {
        nextEuler[0] = clampDeg(pointerStart.rotX + deltaDeg);
      } else if (activeGizmoAxis === 'y') {
        nextEuler[1] = clampDeg(pointerStart.rotY + deltaDeg);
      } else if (activeGizmoAxis === 'z') {
        nextEuler[2] = clampDeg(pointerStart.rotZ + deltaDeg);
      } else if (activeGizmoAxis === 'view') {
        // Outer white view-aligned ring rotates Pitch X in side views or Roll Z in front views
        const absYaw = Math.abs(workingClip.camera.yaw);
        if (absYaw >= 45 && absYaw <= 135) {
          nextEuler[0] = clampDeg(pointerStart.rotX + deltaDeg);
        } else {
          nextEuler[2] = clampDeg(pointerStart.rotZ + deltaDeg);
        }
      }

      kf.bones = {
        ...kf.bones,
        [selectedBone]: nextEuler
      };
      workingClip.keyframes = [...workingClip.keyframes];
      syncSceneAndRender();
      return;
    }

    // Free bone drag fallback when dragging directly on bone mesh
    const kf = ensureKeyframeAtCurrentTime();
    const nextX = Math.max(-180, Math.min(180, Math.round(pointerStart.rotX + dy * 0.65)));
    const nextY = e.ctrlKey
      ? Math.max(-180, Math.min(180, Math.round(pointerStart.rotY + dx * 0.65)))
      : pointerStart.rotY;
    const nextZ = !e.ctrlKey
      ? Math.max(-180, Math.min(180, Math.round(pointerStart.rotZ - dx * 0.65)))
      : pointerStart.rotZ;
    kf.bones = {
      ...kf.bones,
      [selectedBone]: [nextX, nextY, nextZ]
    };
    workingClip.keyframes = [...workingClip.keyframes];
    syncSceneAndRender();
  }

  function handleViewportPointerUp() {
    if (pointerDown && pointerStart.moved) {
      refreshSheetPreview();
    }
    pointerDown = false;
    activeGizmoAxis = null;
  }

  function handleViewportWheel(e: WheelEvent) {
    e.preventDefault();
    const factor = e.deltaY < 0 ? 1.1 : 1 / 1.1;
    workingClip.camera.zoom = Number(
      Math.max(0.45, Math.min(2.8, workingClip.camera.zoom * factor)).toFixed(2)
    );
    syncSceneAndRender();
  }

  function handleSendSheetToSpriteStudio() {
    const hiResUrl = renderClipToSpriteSheetDataUrl(
      workingClip,
      sheetRows,
      sheetCols,
      sheetCols > sheetRows ? 1536 : 1024,
      1024
    );
    previewSheetDataUrl = hiResUrl;
    onApplyToSpriteStudio?.(
      hiResUrl,
      sheetRows,
      sheetCols,
      i18n.locale === 'en' ? workingClip.nameEn : workingClip.name,
      workingClip.id
    );
  }

  async function handleExportReferencePng() {
    try {
      const hiResUrl = renderClipToSpriteSheetDataUrl(
        workingClip,
        sheetRows,
        sheetCols,
        sheetCols > sheetRows ? 1536 : 1024,
        1024
      );
      const ts = new Date().toISOString().replace(/[:.]/g, '-').slice(0, 19);
      const outPath = await api.exportStudioPng(
        hiResUrl,
        `pose3d_${workingClip.id}_${sheetRows}x${sheetCols}_${ts}.png`,
        true
      );
      appStore.toast('ok', `已导出 3D 动作参考图: ${outPath}`);
    } catch (e: any) {
      appStore.toast('err', e?.message ?? '导出失败');
    }
  }
</script>

<svelte:window onkeydown={handleKeyDown} />

<div class="flex min-h-0 flex-1 overflow-hidden">
  <!-- CENTER: 3D Unlit Viewport + Keyframe Timeline + Sprite Reference Preview -->
  <div class="flex min-w-0 flex-1 flex-col gap-3 overflow-y-auto p-4">
    <div class="grid min-h-0 flex-1 grid-cols-12 gap-3.5">
      <!-- Left 7 Cols: Interactive 3D Stickman Viewport + Timeline -->
      <div class="col-span-7 flex flex-col justify-between rounded-2xl border border-line bg-ink-1/90 p-3.5 shadow-xl">
        <!-- Viewport Top Bar -->
        <div class="mb-2 flex flex-wrap items-center justify-between gap-2 border-b border-line pb-2">
          <div class="flex items-center gap-2">
            <Move3d size={15} class="text-accent" />
            <span class="text-xs font-extrabold text-fg">
              {i18n.locale === 'en' ? 'Unlit 3D Stickman Rig' : '无光照 3D 立体火柴人视口'}
            </span>
            <span class="rounded bg-accent/15 px-1.5 py-0.5 font-mono text-[10px] font-bold text-accent">
              Yaw {workingClip.camera.yaw}° · Pitch {workingClip.camera.pitch}°
            </span>
          </div>

          <!-- Undo/Redo + Unity 3D Gizmo Mode (Rotate E / Translate W) + Orbit Switch -->
          <div class="flex flex-wrap items-center gap-1.5">
            <!-- Undo / Redo Buttons -->
            <div class="flex rounded-lg border border-line bg-ink-0 p-0.5 text-[11px]">
              <button
                type="button"
                onclick={handleUndo}
                disabled={undoStack.length === 0}
                title="Ctrl+Z"
                class="flex items-center gap-1 rounded-md px-2 py-0.5 font-bold text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg disabled:opacity-35"
              >
                <Undo2 size={11} />
                <span>{i18n.locale === 'en' ? 'Undo (Ctrl+Z)' : '撤销 (Ctrl+Z)'}</span>
              </button>
              <button
                type="button"
                onclick={handleRedo}
                disabled={redoStack.length === 0}
                title="Ctrl+Y / Ctrl+Shift+Z"
                class="flex items-center gap-1 rounded-md px-2 py-0.5 font-bold text-fg-muted transition-colors hover:bg-ink-2 hover:text-fg disabled:opacity-35"
              >
                <Redo2 size={11} />
                <span>{i18n.locale === 'en' ? 'Redo (Ctrl+Y)' : '重做 (Ctrl+Y)'}</span>
              </button>
            </div>

            <!-- Unity Gizmo Mode: Rotate (E) vs Translate Root (W) -->
            <div class="flex rounded-lg border border-line bg-ink-0 p-0.5 text-[11px]">
              <button
                type="button"
                onclick={() => {
                  gizmoMode = 'rotate';
                  viewportDragMode = 'bone';
                  syncSceneAndRender();
                }}
                class="flex items-center gap-1 rounded-md px-2 py-0.5 font-bold transition-colors {gizmoMode === 'rotate' && viewportDragMode === 'bone'
                  ? 'bg-accent text-accent-ink'
                  : 'text-fg-muted hover:text-fg'}"
              >
                <RotateCw size={11} />
                <span>{i18n.locale === 'en' ? 'Rotate (E)' : '旋转 (E)'}</span>
              </button>
              <button
                type="button"
                onclick={() => {
                  gizmoMode = 'translate';
                  viewportDragMode = 'bone';
                  syncSceneAndRender();
                }}
                class="flex items-center gap-1 rounded-md px-2 py-0.5 font-bold transition-colors {gizmoMode === 'translate' && viewportDragMode === 'bone'
                  ? 'bg-accent text-accent-ink'
                  : 'text-fg-muted hover:text-fg'}"
              >
                <Move3d size={11} />
                <span>{i18n.locale === 'en' ? 'Translate (W)' : '位移 (W)'}</span>
              </button>
              <button
                type="button"
                onclick={() => (viewportDragMode = 'orbit')}
                class="flex items-center gap-1 rounded-md px-2 py-0.5 font-bold transition-colors {viewportDragMode === 'orbit'
                  ? 'bg-accent text-accent-ink'
                  : 'text-fg-muted hover:text-fg'}"
              >
                <Eye size={11} />
                <span>{i18n.locale === 'en' ? 'Orbit Cam' : '旋转视角'}</span>
              </button>
            </div>
          </div>
        </div>

        <!-- Camera Angle Presets Bar + One-Click Left/Right Flip -->
        <div class="mb-1.5 flex flex-wrap items-center justify-between gap-1.5 text-[11px]">
          <div class="flex flex-wrap items-center gap-1">
            {#each CAMERA_PRESETS as cam (cam.id)}
              <button
                type="button"
                onclick={() => handleCameraPreset(cam.id)}
                class="rounded-lg border px-2 py-1 font-bold transition-all {workingClip.camera.preset === cam.id
                  ? 'border-accent bg-accent/15 text-accent'
                  : 'border-line bg-ink-0 text-fg-muted hover:border-fg-muted hover:text-fg'}"
              >
                {i18n.locale === 'en' ? cam.labelEn : cam.label}
              </button>
            {/each}

            <button
              type="button"
              onclick={handleFlipCameraDirection}
              class="flex items-center gap-1 rounded-lg border border-accent/60 bg-accent/15 px-2.5 py-1 font-extrabold text-accent transition-all hover:bg-accent hover:text-accent-ink"
            >
              <FlipHorizontal2 size={12} />
              <span>{i18n.locale === 'en' ? '⇄ Flip Left/Right' : '⇄ 一键朝向翻转 (左/右)'}</span>
            </button>
          </div>

          <label class="flex cursor-pointer items-center gap-1 text-fg-muted">
            <input
              type="checkbox"
              bind:checked={workingClip.camera.ortho}
              onchange={() => {
                pushUndoSnapshot();
                syncSceneAndRender();
                refreshSheetPreview();
              }}
              class="accent-accent"
            />
            <span>{i18n.locale === 'en' ? 'Orthographic' : '正交无透视畸变'}</span>
          </label>
        </div>

        <!-- Compact Camera Yaw / Pitch / Zoom Sliders -->
        <div class="mb-2 grid grid-cols-3 gap-2 rounded-lg border border-line/70 bg-ink-0/65 px-2.5 py-1 text-[10px]">
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-fg-muted">Yaw</span>
            <input
              type="range"
              min="-180"
              max="180"
              step="1"
              value={workingClip.camera.yaw}
              onpointerdown={pushUndoSnapshot}
              oninput={(e) => {
                workingClip.camera.preset = 'custom';
                workingClip.camera.yaw = Number((e.currentTarget as HTMLInputElement).value);
                syncSceneAndRender();
              }}
              onchange={refreshSheetPreview}
              class="flex-1 accent-accent"
            />
            <span class="w-9 text-right font-mono font-bold text-accent">{workingClip.camera.yaw}°</span>
          </div>
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-fg-muted">Pitch</span>
            <input
              type="range"
              min="-85"
              max="85"
              step="1"
              value={workingClip.camera.pitch}
              onpointerdown={pushUndoSnapshot}
              oninput={(e) => {
                workingClip.camera.preset = 'custom';
                workingClip.camera.pitch = Number((e.currentTarget as HTMLInputElement).value);
                syncSceneAndRender();
              }}
              onchange={refreshSheetPreview}
              class="flex-1 accent-accent"
            />
            <span class="w-9 text-right font-mono font-bold text-accent">{workingClip.camera.pitch}°</span>
          </div>
          <div class="flex items-center gap-1.5">
            <span class="font-mono text-fg-muted">Zoom</span>
            <input
              type="range"
              min="0.4"
              max="2.8"
              step="0.05"
              value={workingClip.camera.zoom}
              onpointerdown={pushUndoSnapshot}
              oninput={(e) => {
                workingClip.camera.zoom = Number((e.currentTarget as HTMLInputElement).value);
                syncSceneAndRender();
              }}
              onchange={refreshSheetPreview}
              class="flex-1 accent-accent"
            />
            <span class="w-9 text-right font-mono font-bold text-accent">{workingClip.camera.zoom.toFixed(2)}×</span>
          </div>
        </div>

        <!-- 3D WebGL Canvas Container -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          bind:this={viewportWrapEl}
          oncontextmenu={(e) => e.preventDefault()}
          class="relative min-h-[290px] flex-1 overflow-hidden rounded-xl border border-line bg-[#0E1117]"
        >
          <canvas
            bind:this={canvasEl}
            onpointerdown={handleViewportPointerDown}
            onpointermove={handleViewportPointerMove}
            onpointerup={handleViewportPointerUp}
            onpointercancel={handleViewportPointerUp}
            onwheel={handleViewportWheel}
            class="h-full w-full cursor-crosshair block"
          ></canvas>

          <!-- Color Legend Overlay -->
          <div class="pointer-events-none absolute top-2.5 left-2.5 flex flex-wrap items-center gap-2 rounded-lg border border-line/70 bg-ink-0/85 px-2.5 py-1 text-[10px] font-bold">
            <span class="flex items-center gap-1 text-[#00E5FF]">
              <span class="inline-block h-2 w-2 rounded-full bg-[#00E5FF]"></span>
              {i18n.locale === 'en' ? 'Left Limbs (Cyan)' : '左侧手脚 (青蓝)'}
            </span>
            <span class="flex items-center gap-1 text-[#FF6B6B]">
              <span class="inline-block h-2 w-2 rounded-full bg-[#FF6B6B]"></span>
              {i18n.locale === 'en' ? 'Right Limbs (Coral)' : '右侧手脚 (珊红)'}
            </span>
            <span class="flex items-center gap-1 text-[#C8FF3D]">
              <span class="inline-block h-2 w-2 rounded-full bg-[#C8FF3D]"></span>
              {i18n.locale === 'en' ? 'Weapon (Lime)' : '武器 (荧光绿)'}
            </span>
          </div>

          <div class="pointer-events-none absolute bottom-2 left-2.5 rounded bg-ink-0/80 px-2 py-0.5 font-mono text-[10px] text-fg-muted">
            {i18n.locale === 'en'
              ? 'Drag R/G/B ring for X/Y/Z axis · Empty-space Left/Right-drag orbits · Middle/Shift+Right pans · Ctrl+Z Undo'
              : '拖拽红/绿/蓝环精准单轴旋转 · 空白处左键/右键旋转视角 · 中键/Shift+右键平移 · Ctrl+Z 撤销'}
          </div>
        </div>

        <!-- Keyframe Timeline Bar -->
        <div class="mt-3 flex flex-col gap-2 rounded-xl border border-line bg-ink-0/80 p-2.5">
          <div class="flex flex-wrap items-center justify-between gap-2 text-xs">
            <div class="flex items-center gap-1.5">
              <button
                type="button"
                onclick={() => (isPlaying = !isPlaying)}
                class="flex h-7 w-8 items-center justify-center rounded-lg bg-accent text-accent-ink shadow-sm"
              >
                {#if isPlaying}
                  <Pause size={14} />
                {:else}
                  <Play size={14} />
                {/if}
              </button>
              <span class="font-mono text-xs font-bold text-accent">
                t = {currentTime.toFixed(2)}s
              </span>
              <label class="ml-2 flex cursor-pointer items-center gap-1 text-[11px] text-fg-muted">
                <input type="checkbox" bind:checked={workingClip.loopAnim} class="accent-accent" />
                <span>{i18n.locale === 'en' ? 'Loop' : '循环动作'}</span>
              </label>
            </div>

            <div class="flex flex-wrap items-center gap-1.5">
              <button
                type="button"
                onclick={handleAddKeyframe}
                class="flex items-center gap-1 rounded-lg border border-accent/50 bg-accent/15 px-2 py-1 text-[11px] font-bold text-accent hover:bg-accent hover:text-accent-ink"
              >
                <Plus size={12} />
                <span>{i18n.locale === 'en' ? 'Key Here' : '打关键帧'}</span>
              </button>
              <button
                type="button"
                onclick={handleCopyFirstFrameToEnd}
                title="将 t=0 首帧复制到 t=1 末帧，确保循环无缝衔接"
                class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2 py-1 text-[11px] font-semibold text-fg hover:border-accent"
              >
                <Copy size={11} />
                <span>{i18n.locale === 'en' ? 'Copy 0→1' : '首帧复制到尾帧'}</span>
              </button>
              <button
                type="button"
                onclick={handleDeleteKeyframe}
                disabled={activeKeyframeIdx < 0 || workingClip.keyframes.length <= 1}
                class="flex items-center gap-1 rounded-lg border border-line bg-ink-2 px-2 py-1 text-[11px] font-semibold text-fg-muted hover:border-err hover:text-err disabled:opacity-40"
              >
                <Trash2 size={11} />
                <span>{i18n.locale === 'en' ? 'Del Key' : '删关键帧'}</span>
              </button>
            </div>
          </div>

          <!-- Scrubber + Diamond Keyframe Markers -->
          <div class="relative flex flex-col gap-1 pt-1">
            <input
              type="range"
              min="0"
              max="1"
              step="0.01"
              bind:value={currentTime}
              oninput={() => {
                isPlaying = false;
                syncSceneAndRender();
              }}
              class="w-full accent-accent"
            />
            <div class="relative h-5 w-full rounded bg-ink-2/70">
              {#each workingClip.keyframes as kfItem, idx (idx)}
                {@const isHit = Math.abs(kfItem.t - currentTime) < 0.015}
                <button
                  type="button"
                  onclick={() => {
                    isPlaying = false;
                    currentTime = kfItem.t;
                    syncSceneAndRender();
                  }}
                  title="Keyframe #{idx + 1} (t={kfItem.t.toFixed(2)})"
                  style="left: calc({kfItem.t * 100}% - 7px);"
                  class="absolute top-0.5 flex h-4 w-4 items-center justify-center rounded-sm border text-[9px] font-mono font-bold transition-transform {isHit
                    ? 'scale-110 border-white bg-accent text-accent-ink shadow'
                    : 'border-accent/60 bg-ink-0 text-accent hover:scale-110'}"
                >
                  ◆
                </button>
              {/each}
            </div>
          </div>
        </div>
      </div>

      <!-- Right 5 Cols: Multi-Frame Grid Preview + Send to Sprite Sheet Studio -->
      <div class="col-span-5 flex flex-col justify-between rounded-2xl border border-line bg-ink-1/90 p-3.5 shadow-xl">
        <div class="flex flex-col gap-2.5">
          <div class="flex items-center justify-between border-b border-line pb-2 text-xs">
            <div class="flex items-center gap-1.5">
              <Camera size={14} class="text-accent" />
              <span class="font-extrabold text-fg">
                {i18n.locale === 'en'
                  ? `Rendered ${sheetRows}×${sheetCols} Pose Sheet`
                  : `序列帧姿态参考图预览 (${sheetRows}×${sheetCols} · ${sheetRows * sheetCols}帧)`}
              </span>
            </div>
            <button
              type="button"
              onclick={refreshSheetPreview}
              class="text-[11px] font-bold text-accent hover:underline"
            >
              {i18n.locale === 'en' ? 'Refresh Grid' : '刷新连拍'}
            </button>
          </div>

          <div class="grid grid-cols-2 gap-2 text-xs">
            <div class="flex items-center justify-between rounded-lg border border-line bg-ink-0 px-2.5 py-1">
              <span class="text-[11px] text-fg-muted">{i18n.t('char_rows_label')}</span>
              <input
                type="number"
                min="1"
                max="6"
                bind:value={sheetRows}
                onchange={refreshSheetPreview}
                class="w-12 bg-transparent text-right font-mono text-xs font-bold text-fg outline-none"
              />
            </div>
            <div class="flex items-center justify-between rounded-lg border border-line bg-ink-0 px-2.5 py-1">
              <span class="text-[11px] text-fg-muted">{i18n.t('char_cols_label')}</span>
              <input
                type="number"
                min="1"
                max="8"
                bind:value={sheetCols}
                onchange={refreshSheetPreview}
                class="w-12 bg-transparent text-right font-mono text-xs font-bold text-fg outline-none"
              />
            </div>
          </div>

          {#if previewSheetDataUrl}
            <div class="flex items-center justify-center overflow-hidden rounded-xl border border-line bg-white p-1.5 shadow-inner">
              <img
                src={previewSheetDataUrl}
                alt="3D Pose Reference Grid"
                class="max-h-[260px] w-auto object-contain"
              />
            </div>
          {/if}
        </div>

        <!-- Root Elevation & Bone Euler Controls -->
        <div class="my-2 flex flex-col gap-2 rounded-xl border border-line bg-ink-0/75 p-2.5 text-xs">
          <div class="flex items-center justify-between">
            <span class="font-bold text-accent">
              {i18n.locale === 'en' ? 'Selected Bone:' : '当前选中骨骼:'}
              {BONE_LIST.find((b) => b.id === selectedBone)?.label ?? selectedBone}
            </span>
            <button
              type="button"
              onclick={() => {
                pushUndoSnapshot();
                updateSelectedBoneAxis(0, 0);
                updateSelectedBoneAxis(1, 0);
                updateSelectedBoneAxis(2, 0);
                refreshSheetPreview();
              }}
              class="text-[10px] text-fg-muted hover:text-accent"
            >
              {i18n.locale === 'en' ? 'Zero Bone' : '归零该骨骼'}
            </button>
          </div>

          <div class="grid grid-cols-3 gap-2 text-[11px]">
            <div class="flex flex-col gap-0.5">
              <div class="flex justify-between font-mono text-[10px] text-fg-muted">
                <span class="text-[#EF4444]">Pitch X</span>
                <span class="text-fg">{Math.round(currentBoneEuler[0])}°</span>
              </div>
              <input
                type="range"
                min="-160"
                max="160"
                step="1"
                value={currentBoneEuler[0]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateSelectedBoneAxis(0, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="accent-accent"
              />
            </div>
            <div class="flex flex-col gap-0.5">
              <div class="flex justify-between font-mono text-[10px] text-fg-muted">
                <span class="text-[#22C55E]">Yaw Y</span>
                <span class="text-fg">{Math.round(currentBoneEuler[1])}°</span>
              </div>
              <input
                type="range"
                min="-160"
                max="160"
                step="1"
                value={currentBoneEuler[1]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateSelectedBoneAxis(1, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="accent-accent"
              />
            </div>
            <div class="flex flex-col gap-0.5">
              <div class="flex justify-between font-mono text-[10px] text-fg-muted">
                <span class="text-[#3B82F6]">Roll Z</span>
                <span class="text-fg">{Math.round(currentBoneEuler[2])}°</span>
              </div>
              <input
                type="range"
                min="-160"
                max="160"
                step="1"
                value={currentBoneEuler[2]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateSelectedBoneAxis(2, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="accent-accent"
              />
            </div>
          </div>

          <!-- Root Position (X / Jump Y / Forward Z) -->
          <div class="grid grid-cols-3 gap-2 border-t border-line/60 pt-1.5 text-[11px]">
            <div class="flex items-center gap-1">
              <span class="font-mono text-[10px] text-[#EF4444]">
                {i18n.locale === 'en' ? 'Root X:' : '位移 X:'}
              </span>
              <input
                type="range"
                min="-0.7"
                max="0.7"
                step="0.02"
                value={currentRootPos[0]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateRootPosAxis(0, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="flex-1 accent-accent"
              />
              <span class="w-8 text-right font-mono text-[10px] text-accent">
                {currentRootPos[0].toFixed(2)}
              </span>
            </div>
            <div class="flex items-center gap-1">
              <span class="font-mono text-[10px] text-[#22C55E]">
                {i18n.locale === 'en' ? 'Jump Y:' : '跳跃 Y:'}
              </span>
              <input
                type="range"
                min="-0.75"
                max="0.75"
                step="0.02"
                value={currentRootPos[1]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateRootPosAxis(1, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="flex-1 accent-accent"
              />
              <span class="w-8 text-right font-mono text-[10px] text-accent">
                {currentRootPos[1].toFixed(2)}
              </span>
            </div>
            <div class="flex items-center gap-1">
              <span class="font-mono text-[10px] text-[#3B82F6]">
                {i18n.locale === 'en' ? 'Root Z:' : '前冲 Z:'}
              </span>
              <input
                type="range"
                min="-0.75"
                max="0.75"
                step="0.02"
                value={currentRootPos[2]}
                onpointerdown={pushUndoSnapshot}
                oninput={(e) => updateRootPosAxis(2, Number((e.currentTarget as HTMLInputElement).value))}
                onchange={refreshSheetPreview}
                class="flex-1 accent-accent"
              />
              <span class="w-8 text-right font-mono text-[10px] text-accent">
                {currentRootPos[2].toFixed(2)}
              </span>
            </div>
          </div>
        </div>

        <!-- Primary Action CTA: Send to Sprite Sheet Studio -->
        <div class="flex flex-col gap-2">
          <button
            type="button"
            onclick={handleSendSheetToSpriteStudio}
            class="flex w-full items-center justify-center gap-2 rounded-xl bg-accent py-2.5 text-xs font-extrabold text-accent-ink shadow-md hover:brightness-105"
          >
            <Sparkles size={15} />
            <span>
              {i18n.locale === 'en'
                ? 'Capture Reference Sheet & Send to Sprite Studio'
                : '拍成序列参考图并送入序列帧工坊'}
            </span>
          </button>

          <div class="flex gap-2">
            <button
              type="button"
              onclick={handleExportReferencePng}
              class="flex flex-1 items-center justify-center gap-1 rounded-lg border border-line bg-ink-2 py-1.5 text-[11px] font-semibold text-fg hover:border-accent"
            >
              <Download size={12} />
              <span>{i18n.locale === 'en' ? 'Export Pose Sheet PNG' : '导出姿态参考图 PNG'}</span>
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- RIGHT INSPECTOR: Action Library, Body Proportions, Weapon & Bone Picker -->
  <aside class="flex w-80 shrink-0 flex-col gap-3 overflow-y-auto border-l border-line bg-ink-1/95 p-3.5">
    <!-- 1. Action Preset & Custom Clip Manager -->
    <div class="flex flex-col gap-2 rounded-xl border border-line bg-ink-0/70 p-2.5 text-xs">
      <div class="flex items-center justify-between">
        <span class="font-extrabold text-fg">
          {i18n.locale === 'en' ? '3D Action Library' : '3D 动作预设与自定义库'}
        </span>
        <button
          type="button"
          onclick={handleResetClip}
          class="flex items-center gap-1 text-[11px] text-fg-muted hover:text-accent"
        >
          <RotateCcw size={11} />
          <span>{i18n.locale === 'en' ? 'Reset' : '重置动作'}</span>
        </button>
      </div>

      <select
        value={selectedActionId}
        onchange={(e) => selectActionClip((e.currentTarget as HTMLSelectElement).value)}
        class="w-full rounded-lg border border-line bg-ink-0 px-2.5 py-1.5 text-xs font-bold text-fg outline-none focus:border-accent"
      >
        {#each actionClips as clip (clip.id)}
          <option value={clip.id}>
            {i18n.locale === 'en' ? clip.nameEn || clip.name : clip.name}
            {clip.builtin ? '' : ' ★'}
          </option>
        {/each}
      </select>

      <div class="flex gap-1.5">
        <button
          type="button"
          onclick={handleSaveActionClip}
          disabled={isSavingClip}
          class="flex flex-1 items-center justify-center gap-1 rounded-lg bg-accent/15 py-1.5 text-[11px] font-extrabold text-accent hover:bg-accent hover:text-accent-ink"
        >
          <Save size={12} />
          <span>{i18n.locale === 'en' ? 'Save Action' : '保存当前修改'}</span>
        </button>
        <button
          type="button"
          onclick={handleSaveAsCustomAction}
          class="flex items-center justify-center gap-1 rounded-lg border border-line bg-ink-2 px-2.5 py-1.5 text-[11px] font-semibold text-fg hover:border-accent"
        >
          <Plus size={12} />
          <span>{i18n.locale === 'en' ? 'Save As' : '另存新动作'}</span>
        </button>
        {#if !workingClip.builtin}
          <button
            type="button"
            onclick={handleDeleteCustomAction}
            class="flex items-center justify-center rounded-lg border border-line bg-ink-2 px-2 py-1.5 text-fg-muted hover:border-err hover:text-err"
          >
            <Trash2 size={12} />
          </button>
        {/if}
      </div>
    </div>

    <!-- 2. Body Proportion & Head-to-Body Ratio Presets -->
    <div class="flex flex-col gap-2 rounded-xl border border-line bg-ink-0/70 p-2.5 text-xs">
      <span class="font-extrabold text-fg">
        {i18n.locale === 'en' ? 'Body Proportions & Weapon' : '头身比例与武器道具'}
      </span>

      <div class="grid grid-cols-3 gap-1">
        {#each PROPORTION_PRESETS as prop (prop.id)}
          <button
            type="button"
            onclick={() => handleProportionPreset(prop.id)}
            class="rounded-lg border px-1.5 py-1 text-[10px] font-bold transition-all {workingClip.proportion.preset === prop.id
              ? 'border-accent bg-accent/15 text-accent'
              : 'border-line bg-ink-0 text-fg-muted hover:text-fg'}"
          >
            {i18n.locale === 'en' ? prop.labelEn : prop.label}
          </button>
        {/each}
      </div>

      <div class="flex flex-col gap-1.5 pt-1 text-[11px]">
        <div class="flex items-center justify-between gap-2">
          <span class="text-fg-muted">{i18n.locale === 'en' ? 'Head Scale' : '头部大小'}</span>
          <input
            type="range"
            min="0.7"
            max="2.2"
            step="0.05"
            bind:value={workingClip.proportion.headScale}
            onpointerdown={pushUndoSnapshot}
            oninput={() => {
              workingClip.proportion.preset = 'custom';
              syncSceneAndRender();
            }}
            onchange={refreshSheetPreview}
            class="flex-1 accent-accent"
          />
          <span class="w-9 text-right font-mono text-[10px] text-accent">
            {workingClip.proportion.headScale.toFixed(2)}×
          </span>
        </div>

        <div class="flex items-center justify-between gap-2">
          <span class="text-fg-muted">{i18n.locale === 'en' ? 'Torso Scale' : '躯干比例'}</span>
          <input
            type="range"
            min="0.6"
            max="1.5"
            step="0.05"
            bind:value={workingClip.proportion.torsoScale}
            onpointerdown={pushUndoSnapshot}
            oninput={() => {
              workingClip.proportion.preset = 'custom';
              syncSceneAndRender();
            }}
            onchange={refreshSheetPreview}
            class="flex-1 accent-accent"
          />
          <span class="w-9 text-right font-mono text-[10px] text-accent">
            {workingClip.proportion.torsoScale.toFixed(2)}×
          </span>
        </div>

        <div class="flex items-center justify-between gap-2">
          <span class="text-fg-muted">{i18n.locale === 'en' ? 'Limb Length' : '四肢长度'}</span>
          <input
            type="range"
            min="0.55"
            max="1.45"
            step="0.05"
            bind:value={workingClip.proportion.limbScale}
            onpointerdown={pushUndoSnapshot}
            oninput={() => {
              workingClip.proportion.preset = 'custom';
              syncSceneAndRender();
            }}
            onchange={refreshSheetPreview}
            class="flex-1 accent-accent"
          />
          <span class="w-9 text-right font-mono text-[10px] text-accent">
            {workingClip.proportion.limbScale.toFixed(2)}×
          </span>
        </div>
      </div>

      <div class="grid grid-cols-2 gap-2 border-t border-line/60 pt-2 text-[11px]">
        <div class="flex flex-col gap-1">
          <span class="text-fg-muted">{i18n.locale === 'en' ? 'Weapon Type' : '右手手持武器'}</span>
          <select
            bind:value={workingClip.proportion.weaponType}
            onchange={() => {
              pushUndoSnapshot();
              syncSceneAndRender();
              refreshSheetPreview();
            }}
            class="rounded-lg border border-line bg-ink-0 px-2 py-1 text-xs font-semibold text-fg outline-none focus:border-accent"
          >
            {#each WEAPON_TYPES as wt (wt.id)}
              <option value={wt.id}>{i18n.locale === 'en' ? wt.labelEn : wt.label}</option>
            {/each}
          </select>
        </div>

        <div class="flex flex-col gap-1">
          <span class="text-fg-muted">
            {i18n.locale === 'en' ? 'Weapon Scale' : '武器尺寸'} ({workingClip.proportion.weaponScale.toFixed(1)}×)
          </span>
          <input
            type="range"
            min="0.5"
            max="2.0"
            step="0.1"
            bind:value={workingClip.proportion.weaponScale}
            onpointerdown={pushUndoSnapshot}
            oninput={syncSceneAndRender}
            onchange={refreshSheetPreview}
            class="mt-1.5 accent-accent"
          />
        </div>
      </div>
    </div>

    <!-- 3. Hierarchical 16-Bone Quick Selector -->
    <div class="flex flex-col gap-1.5 rounded-xl border border-line bg-ink-0/70 p-2.5 text-xs">
      <span class="font-extrabold text-fg">
        {i18n.locale === 'en' ? '16-Bone Hierarchy Picker' : '16 骨骼层级点选列表'}
      </span>
      <div class="grid grid-cols-2 gap-1 max-h-[250px] overflow-y-auto pr-0.5">
        {#each BONE_LIST as bone (bone.id)}
          {@const active = selectedBone === bone.id}
          <button
            type="button"
            onclick={() => {
              selectedBone = bone.id;
              syncSceneAndRender();
            }}
            class="flex items-center gap-1.5 truncate rounded-lg border px-2 py-1 text-left text-[10px] font-semibold transition-all {active
              ? 'border-accent bg-accent/15 text-accent shadow-sm'
              : 'border-line bg-ink-0 text-fg-muted hover:border-fg-muted hover:text-fg'}"
          >
            <span
              class="inline-block h-2 w-2 shrink-0 rounded-full"
              style="background-color: {bone.color};"
            ></span>
            <span class="truncate">{i18n.locale === 'en' ? bone.labelEn : bone.label}</span>
          </button>
        {/each}
      </div>
    </div>
  </aside>
</div>
