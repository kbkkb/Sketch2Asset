import { removeSolidBackground } from './color-cutout';
import { cleanPixelArtCanvas } from './pixel-clean';

export interface TurnaroundSliceItem {
  key: 'front' | 'side' | 'back';
  label: string;
  canvas: HTMLCanvasElement;
  dataUrl: string;
  width: number;
  height: number;
}

export interface FrameTweak {
  dx: number;
  dy: number;
  enabled: boolean;
  expandLeft?: number;
  expandRight?: number;
  expandTop?: number;
  expandBottom?: number;
}

export interface SpriteFrameItem {
  index: number;
  row: number;
  col: number;
  canvas: HTMLCanvasElement;
  dataUrl: string;
  width: number;
  height: number;
  hasContent: boolean;
}

export interface SpriteSheetProcessOptions {
  rows: number;
  cols: number;
  removeBg: boolean;
  bgTolerance?: number;
  alignPivot: boolean;
  baselineRatio?: number;
  pixelClean: boolean;
  smartIslandSlice?: boolean;
  preventClipping?: boolean;
   trimOuterMargin?: boolean;
  frameTweaks?: FrameTweak[];
}

/**
 * 自动寻找正/侧/背三视图在约 1/3 与 2/3 宽度处的最干净纵向空白分割线
 */
export function detectTurnaroundSplits(sourceCanvas: HTMLCanvasElement): [number, number] {
  const w = sourceCanvas.width;
  const h = sourceCanvas.height;
  if (w < 60 || h < 60) return [1 / 3, 2 / 3];

  const ctx = sourceCanvas.getContext('2d', { willReadFrequently: true })!;
  const data = ctx.getImageData(0, 0, w, h).data;

  // 采样四角背景基准色
  const corners: [number, number][] = [
    [2, 2],
    [w - 3, 2],
    [2, h - 3],
    [w - 3, h - 3]
  ];
  let bgR = 0;
  let bgG = 0;
  let bgB = 0;
  for (const [cx, cy] of corners) {
    const idx = (cy * w + cx) * 4;
    bgR += data[idx] ?? 255;
    bgG += data[idx + 1] ?? 255;
    bgB += data[idx + 2] ?? 255;
  }
  bgR /= corners.length;
  bgG /= corners.length;
  bgB /= corners.length;

  const colEnergy = new Float32Array(w);
  for (let x = 0; x < w; x++) {
    let sum = 0;
    for (let y = Math.floor(h * 0.05); y < Math.floor(h * 0.95); y += 2) {
      const idx = (y * w + x) * 4;
      const a = data[idx + 3] ?? 255;
      if (a < 32) continue;
      const diff =
        Math.abs((data[idx] ?? 255) - bgR) +
        Math.abs((data[idx + 1] ?? 255) - bgG) +
        Math.abs((data[idx + 2] ?? 255) - bgB);
      if (diff > 24) sum += diff;
    }
    colEnergy[x] = sum;
  }

  function findMinColumn(minRatio: number, maxRatio: number, idealRatio: number): number {
    const x0 = Math.max(4, Math.floor(w * minRatio));
    const x1 = Math.min(w - 5, Math.ceil(w * maxRatio));
    const idealX = w * idealRatio;
    let bestX = Math.round(idealX);
    let bestScore = Infinity;

    for (let x = x0; x <= x1; x++) {
      const e =
        ((colEnergy[x - 2] ?? 0) +
          (colEnergy[x - 1] ?? 0) +
          (colEnergy[x] ?? 0) * 2 +
          (colEnergy[x + 1] ?? 0) +
          (colEnergy[x + 2] ?? 0)) /
        6;
      const distPenalty = Math.abs(x - idealX) * 1.5;
      const score = e + distPenalty;
      if (score < bestScore) {
        bestScore = score;
        bestX = x;
      }
    }
    return bestX / w;
  }

  const s1 = findMinColumn(0.25, 0.42, 1 / 3);
  const s2 = findMinColumn(0.58, 0.75, 2 / 3);
  return [Number(s1.toFixed(4)), Number(s2.toFixed(4))];
}

/**
 * 将正/侧/背三视图按两条分割线切分为三张独立视图，可选自动剔除纯色背景并反解白边
 */
export function sliceTurnaroundViews(
  sourceCanvas: HTMLCanvasElement,
  split1Ratio: number,
  split2Ratio: number,
  autoRemoveBg = true,
  tolerance = 34
): TurnaroundSliceItem[] {
  const workCanvas = autoRemoveBg
    ? removeSolidBackground(sourceCanvas, tolerance, true)
    : sourceCanvas;

  const w = workCanvas.width;
  const h = workCanvas.height;
  const x1 = Math.max(16, Math.min(w - 32, Math.round(w * Math.min(split1Ratio, split2Ratio))));
  const x2 = Math.max(x1 + 16, Math.min(w - 16, Math.round(w * Math.max(split1Ratio, split2Ratio))));

  const ranges: { key: 'front' | 'side' | 'back'; label: string; sx: number; sw: number }[] = [
    { key: 'front', label: '正面视图 (Front)', sx: 0, sw: x1 },
    { key: 'side', label: '侧面视图 (Side)', sx: x1, sw: x2 - x1 },
    { key: 'back', label: '背面视图 (Back)', sx: x2, sw: w - x2 }
  ];

  return ranges.map((r) => {
    const cv = document.createElement('canvas');
    cv.width = r.sw;
    cv.height = h;
    const ctx = cv.getContext('2d')!;
    ctx.drawImage(workCanvas, r.sx, 0, r.sw, h, 0, 0, r.sw, h);
    return {
      key: r.key,
      label: r.label,
      canvas: cv,
      dataUrl: cv.toDataURL('image/png'),
      width: r.sw,
      height: h
    };
  });
}

/**
 * 自动擦除 AI 偶尔绘制在精灵表分格线或外框上的贯穿细线（避免框线干扰连通域切帧和重心计算）
 */
function stripGridBorderLines(
  imgData: ImageData,
  w: number,
  h: number,
  rows: number,
  cols: number
): void {
  const d = imgData.data;

  // 检查纵向网格线附近的贯穿直线（包含外边框 x=0, w-1 及内部分割线 c * w / cols）
  const checkBandX = Math.max(3, Math.round(w * 0.012));
  for (let c = 0; c <= cols; c++) {
    const nominalX = Math.round((c * w) / cols);
    const xStart = Math.max(0, nominalX - checkBandX);
    const xEnd = Math.min(w - 1, nominalX + checkBandX);
    for (let x = xStart; x <= xEnd; x++) {
      let opaqueCount = 0;
      for (let y = 0; y < h; y += 2) {
        if ((d[(y * w + x) * 4 + 3] ?? 0) > 25) opaqueCount++;
      }
      // 若该列超过 65% 的高度都有像素，说明是一条贯穿表格竖线
      if (opaqueCount > (h / 2) * 0.65) {
        for (let y = 0; y < h; y++) {
          d[(y * w + x) * 4 + 3] = 0;
        }
      }
    }
  }

  // 检查横向网格线附近的贯穿直线
  const checkBandY = Math.max(3, Math.round(h * 0.012));
  for (let r = 0; r <= rows; r++) {
    const nominalY = Math.round((r * h) / rows);
    const yStart = Math.max(0, nominalY - checkBandY);
    const yEnd = Math.min(h - 1, nominalY + checkBandY);
    for (let y = yStart; y <= yEnd; y++) {
      let opaqueCount = 0;
      for (let x = 0; x < w; x += 2) {
        if ((d[(y * w + x) * 4 + 3] ?? 0) > 25) opaqueCount++;
      }
      if (opaqueCount > (w / 2) * 0.65) {
        for (let x = 0; x < w; x++) {
          d[(y * w + x) * 4 + 3] = 0;
        }
      }
    }
  }
}

/**
 * 计算单帧内非透明像素的包围盒与下半身躯干/双脚稳定质心 X
 * 重点以角色下半身与核心躯干高密度列计算 centerX，防止角色横向挥出长刀/剑气时把身体重心拉偏
 */
function computeCharacterFootPivot(
  imgData: ImageData,
  w: number,
  h: number
): { minX: number; minY: number; maxX: number; maxY: number; centerX: number; hasContent: boolean } {
  const d = imgData.data;
  let minX = w;
  let minY = h;
  let maxX = -1;
  let maxY = -1;

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const a = d[(y * w + x) * 4 + 3] ?? 0;
      if (a > 28) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }

  if (maxX < minX || maxY < minY) {
    return { minX: 0, minY: 0, maxX: w - 1, maxY: h - 1, centerX: w / 2, hasContent: false };
  }

  const charH = Math.max(1, maxY - minY + 1);
  // 聚焦角色下半身（35% ~ 100% 高度区间，即腰胯至双脚），并且对每列像素密度做平方加权，抗长刀/横向弧光干扰
  const lowerTop = minY + Math.floor(charH * 0.35);
  let weightedSumX = 0;
  let totalWeight = 0;

  for (let x = minX; x <= maxX; x++) {
    let colCount = 0;
    for (let y = lowerTop; y <= maxY; y++) {
      const a = d[(y * w + x) * 4 + 3] ?? 0;
      if (a > 64) colCount++;
    }
    if (colCount > 0) {
      // 平方加权：躯干与腿部纵向厚实（colCount 大），而横向伸出的细长刀剑 colCount 小，从而稳定锚定在身体中轴线
      const weight = colCount * colCount;
      weightedSumX += x * weight;
      totalWeight += weight;
    }
  }

  const bboxCenterX = (minX + maxX) / 2;
  const coreCenterX = totalWeight > 0 ? weightedSumX / totalWeight : bboxCenterX;
  const centerX = coreCenterX * 0.8 + bboxCenterX * 0.2;

  return { minX, minY, maxX, maxY, centerX, hasContent: true };
}

/**
 * 在不抠图（保持原图背景与刀光特效完全不损失）的情况下，
 * 仅通过内存采样四角底色并计算色差，虚拟探测角色主体的腰胯质心与脚底位置，
 * 绝不修改画布上的任何一个像素！
 */
function detectOpaqueCellPivot(
  ctx: CanvasRenderingContext2D,
  w: number,
  h: number,
  baselineRatio: number
): { minX: number; minY: number; maxX: number; maxY: number; centerX: number; hasContent: boolean } {
  const imgData = ctx.getImageData(0, 0, w, h);
  const d = imgData.data;

  // 采样四角底色均值
  const corners: [number, number][] = [
    [1, 1],
    [Math.max(1, w - 2), 1],
    [1, Math.max(1, h - 2)],
    [Math.max(1, w - 2), Math.max(1, h - 2)]
  ];
  let bgR = 0;
  let bgG = 0;
  let bgB = 0;
  for (const [cx, cy] of corners) {
    const idx = (cy * w + cx) * 4;
    bgR += d[idx] ?? 255;
    bgG += d[idx + 1] ?? 255;
    bgB += d[idx + 2] ?? 255;
  }
  bgR /= corners.length;
  bgG /= corners.length;
  bgB /= corners.length;

  let minX = w;
  let minY = h;
  let maxX = -1;
  let maxY = -1;
  const colorDiffThreshold = 28;

  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      const idx = (y * w + x) * 4;
      const diff =
        Math.abs((d[idx] ?? 0) - bgR) +
        Math.abs((d[idx + 1] ?? 0) - bgG) +
        Math.abs((d[idx + 2] ?? 0) - bgB);
      if (diff > colorDiffThreshold) {
        if (x < minX) minX = x;
        if (x > maxX) maxX = x;
        if (y < minY) minY = y;
        if (y > maxY) maxY = y;
      }
    }
  }

  if (maxX < minX || maxY < minY) {
    return {
      minX: 0,
      minY: 0,
      maxX: w - 1,
      maxY: Math.round(h * baselineRatio),
      centerX: w / 2,
      hasContent: false
    };
  }

  const charH = Math.max(1, maxY - minY + 1);
  const lowerTop = minY + Math.floor(charH * 0.35);
  let weightedSumX = 0;
  let totalWeight = 0;

  for (let x = minX; x <= maxX; x++) {
    let colCount = 0;
    for (let y = lowerTop; y <= maxY; y++) {
      const idx = (y * w + x) * 4;
      const diff =
        Math.abs((d[idx] ?? 0) - bgR) +
        Math.abs((d[idx + 1] ?? 0) - bgG) +
        Math.abs((d[idx + 2] ?? 0) - bgB);
      if (diff > colorDiffThreshold) colCount++;
    }
    if (colCount > 0) {
      const weight = colCount * colCount;
      weightedSumX += x * weight;
      totalWeight += weight;
    }
  }

  const bboxCenterX = (minX + maxX) / 2;
  const coreCenterX = totalWeight > 0 ? weightedSumX / totalWeight : bboxCenterX;
  const centerX = coreCenterX * 0.8 + bboxCenterX * 0.2;

  return { minX, minY, maxX, maxY, centerX, hasContent: true };
}

interface ExtractedCellCanvas {
  canvas: HTMLCanvasElement;
  pivot: {
    minX: number;
    minY: number;
    maxX: number;
    maxY: number;
    centerX: number;
    hasContent: boolean;
  };
}

/**
 * 连通域智能聚类切帧：
 * 1. 自动找回跨过均匀分格线的刀尖、武器末端或衣角（只要与本帧角色相连，绝不一刀切平）；
 * 2. 自动剔除隔壁格子越界闯入当前帧角落的残片（如隔壁帧伸过来的脚尖或刀尖残影）。
 */
function extractCellsByConnectedIslands(
  workingSheet: HTMLCanvasElement,
  sourceCanvas: HTMLCanvasElement,
  rows: number,
  cols: number,
  useSmartIslands: boolean,
  removeBg: boolean,
  baselineRatio: number,
  frameTweaks?: FrameTweak[]
): ExtractedCellCanvas[] {
  const sheetW = workingSheet.width;
  const sheetH = workingSheet.height;
  const cellW = Math.max(16, Math.floor(sheetW / cols));
  const cellH = Math.max(16, Math.floor(sheetH / rows));

  // 1. 若用户明确未开启「自动去白底」：
  // 必须 100% 保护原图刀光、高光剑气、特效与色彩，严禁任何破坏性透明化！
  if (!removeBg) {
    const sctx = workingSheet.getContext('2d', { willReadFrequently: true })!;
    const sample = sctx.getImageData(0, 0, Math.min(sheetW, 120), Math.min(sheetH, 120)).data;
    let hasNativeAlpha = false;
    for (let i = 3; i < sample.length; i += 4) {
      if ((sample[i] ?? 255) < 220) {
        hasNativeAlpha = true;
        break;
      }
    }

    // 若原图没有原生透明度（例如为普通白底或黑底生图大图），直接精准切分原始矩形网格（支持单帧自定义跨格边界扩展）
    if (!hasNativeAlpha) {
      const out: ExtractedCellCanvas[] = [];
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const idx = r * cols + c;
          const tweak = frameTweaks?.[idx];
          const expLeft = Math.max(-Math.floor(cellW * 0.4), tweak?.expandLeft ?? 0);
          const expRight = Math.max(-Math.floor(cellW * 0.4), tweak?.expandRight ?? 0);
          const expTop = Math.max(-Math.floor(cellH * 0.4), tweak?.expandTop ?? 0);
          const expBottom = Math.max(-Math.floor(cellH * 0.4), tweak?.expandBottom ?? 0);

          const sx0 = Math.max(0, c * cellW - expLeft);
          const sy0 = Math.max(0, r * cellH - expTop);
          const sx1 = Math.min(sheetW, (c + 1) * cellW + expRight);
          const sy1 = Math.min(sheetH, (r + 1) * cellH + expBottom);
          const actualW = Math.max(8, sx1 - sx0);
          const actualH = Math.max(8, sy1 - sy0);

          const cv = document.createElement('canvas');
          cv.width = actualW;
          cv.height = actualH;
          const ctx = cv.getContext('2d', { willReadFrequently: true })!;
          // 完整采样指定扩展区域的原始图像（彻底找回向右跨入下一格的刀光与剑气）
          ctx.drawImage(
            sourceCanvas,
            sx0,
            sy0,
            actualW,
            actualH,
            0,
            0,
            actualW,
            actualH
          );
          const pivot = detectOpaqueCellPivot(ctx, actualW, actualH, baselineRatio);
          out.push({ canvas: cv, pivot });
        }
      }
      return out;
    }
  }

  const sctx = workingSheet.getContext('2d', { willReadFrequently: true })!;
  const sheetImg = sctx.getImageData(0, 0, sheetW, sheetH);

  // 仅在开启去背景模式下，才清理可能存在的贯穿网格线
  if (removeBg) {
    stripGridBorderLines(sheetImg, sheetW, sheetH, rows, cols);
  }

  const totalCells = rows * cols;

  if (!useSmartIslands) {
    const out: ExtractedCellCanvas[] = [];
    for (let r = 0; r < rows; r++) {
      for (let c = 0; c < cols; c++) {
        const idx = r * cols + c;
        const tweak = frameTweaks?.[idx];
        const expLeft = Math.max(-Math.floor(cellW * 0.4), tweak?.expandLeft ?? 0);
        const expRight = Math.max(-Math.floor(cellW * 0.4), tweak?.expandRight ?? 0);
        const expTop = Math.max(-Math.floor(cellH * 0.4), tweak?.expandTop ?? 0);
        const expBottom = Math.max(-Math.floor(cellH * 0.4), tweak?.expandBottom ?? 0);

        const sx0 = Math.max(0, c * cellW - expLeft);
        const sy0 = Math.max(0, r * cellH - expTop);
        const sx1 = Math.min(sheetW, (c + 1) * cellW + expRight);
        const sy1 = Math.min(sheetH, (r + 1) * cellH + expBottom);
        const actualW = Math.max(8, sx1 - sx0);
        const actualH = Math.max(8, sy1 - sy0);

        const cv = document.createElement('canvas');
        cv.width = actualW;
        cv.height = actualH;
        const ctx = cv.getContext('2d', { willReadFrequently: true })!;
        ctx.putImageData(sheetImg, -sx0, -sy0, sx0, sy0, actualW, actualH);
        const cData = ctx.getImageData(0, 0, actualW, actualH);
        out.push({ canvas: cv, pivot: computeCharacterFootPivot(cData, actualW, actualH) });
      }
    }
    return out;
  }

  const d = sheetImg.data;
  const totalPixels = sheetW * sheetH;
  const labels = new Int32Array(totalPixels);
  labels.fill(-1);

  interface IslandInfo {
    id: number;
    pixels: number[];
    minX: number;
    minY: number;
    maxX: number;
    maxY: number;
    sumX: number;
    sumY: number;
    ownerCell: number;
    isOversized: boolean;
  }

  const islands: IslandInfo[] = [];
  const queue = new Int32Array(totalPixels);

  for (let p = 0; p < totalPixels; p++) {
    if (labels[p] !== -1) continue;
    if ((d[p * 4 + 3] ?? 0) <= 24) continue;

    const id = islands.length;
    labels[p] = id;
    let qHead = 0;
    let qTail = 0;
    queue[qTail++] = p;

    const pixels: number[] = [];
    let minX = sheetW;
    let minY = sheetH;
    let maxX = 0;
    let maxY = 0;
    let sumX = 0;
    let sumY = 0;

    while (qHead < qTail) {
      const curr = queue[qHead++]!;
      pixels.push(curr);
      const cy = Math.floor(curr / sheetW);
      const cx = curr - cy * sheetW;

      if (cx < minX) minX = cx;
      if (cx > maxX) maxX = cx;
      if (cy < minY) minY = cy;
      if (cy > maxY) maxY = cy;
      sumX += cx;
      sumY += cy;

      // 8-邻域搜索
      const y0 = Math.max(0, cy - 1);
      const y1 = Math.min(sheetH - 1, cy + 1);
      const x0 = Math.max(0, cx - 1);
      const x1 = Math.min(sheetW - 1, cx + 1);
      for (let ny = y0; ny <= y1; ny++) {
        const rowOff = ny * sheetW;
        for (let nx = x0; nx <= x1; nx++) {
          const np = rowOff + nx;
          if (labels[np] === -1 && (d[np * 4 + 3] ?? 0) > 24) {
            labels[np] = id;
            queue[qTail++] = np;
          }
        }
      }
    }

    // 过滤极小孤立噪点
    if (pixels.length < 10) continue;

    const meanX = sumX / pixels.length;
    const meanY = sumY / pixels.length;
    const c = Math.min(cols - 1, Math.max(0, Math.floor(meanX / cellW)));
    const r = Math.min(rows - 1, Math.max(0, Math.floor(meanY / cellH)));
    const bw = maxX - minX + 1;
    const bh = maxY - minY + 1;
    // 若单个连通域跨度超过 1.45 个单元格，说明相邻两帧角色粘连在一起了，此时对粘连块回退按网格边界切分
    const isOversized = bw > cellW * 1.45 || bh > cellH * 1.45;

    islands.push({
      id,
      pixels,
      minX,
      minY,
      maxX,
      maxY,
      sumX,
      sumY,
      ownerCell: r * cols + c,
      isOversized
    });
  }

  // 找到每个单元格的主体角色中心（面积最大的连通域），以便将分离的刀光/特效碎片归属给距离最近的角色主体
  const cellMainCenters: { cx: number; cy: number }[] = Array.from({ length: totalCells }, (_, idx) => {
    const r = Math.floor(idx / cols);
    const c = idx % cols;
    return { cx: (c + 0.5) * cellW, cy: (r + 0.5) * cellH };
  });

  for (let cellIdx = 0; cellIdx < totalCells; cellIdx++) {
    let bestArea = 0;
    for (const isl of islands) {
      if (isl.ownerCell === cellIdx && !isl.isOversized && isl.pixels.length > bestArea) {
        bestArea = isl.pixels.length;
        cellMainCenters[cellIdx] = {
          cx: isl.sumX / isl.pixels.length,
          cy: isl.sumY / isl.pixels.length
        };
      }
    }
  }

  // 对于面积较小且靠近单元格边缘的游离碎片（如隔壁帧被切断的脚尖或游离特效），按与各格主体中心的最近距离重新归属
  for (const isl of islands) {
    if (isl.isOversized) continue;
    if (isl.pixels.length < cellW * cellH * 0.04) {
      const mx = isl.sumX / isl.pixels.length;
      const my = isl.sumY / isl.pixels.length;
      let bestCell = isl.ownerCell;
      let bestDist = Infinity;
      for (let cellIdx = 0; cellIdx < totalCells; cellIdx++) {
        const mc = cellMainCenters[cellIdx]!;
        const dist = Math.hypot(mx - mc.cx, my - mc.cy);
        if (dist < bestDist) {
          bestDist = dist;
          bestCell = cellIdx;
        }
      }
      isl.ownerCell = bestCell;
    }
  }

  // 为每个单元格构建专属提取画布（预留四周各 25% 弹性溢出接收区，确保跨过网格线的刀尖完整保留！）
  const padX = Math.round(cellW * 0.25);
  const padY = Math.round(cellH * 0.25);
  const extW = cellW + padX * 2;
  const extH = cellH + padY * 2;

  const cellImgDatas = Array.from({ length: totalCells }, () => new ImageData(extW, extH));

  for (const isl of islands) {
    if (!isl.isOversized) {
      const cellIdx = isl.ownerCell;
      const r = Math.floor(cellIdx / cols);
      const c = cellIdx % cols;
      const originX = c * cellW - padX;
      const originY = r * cellH - padY;
      const dst = cellImgDatas[cellIdx]!.data;

      for (let i = 0; i < isl.pixels.length; i++) {
        const p = isl.pixels[i]!;
        const py = Math.floor(p / sheetW);
        const px = p - py * sheetW;
        const dx = px - originX;
        const dy = py - originY;
        if (dx >= 0 && dx < extW && dy >= 0 && dy < extH) {
          const sOff = p * 4;
          const dOff = (dy * extW + dx) * 4;
          dst[dOff] = d[sOff]!;
          dst[dOff + 1] = d[sOff + 1]!;
          dst[dOff + 2] = d[sOff + 2]!;
          dst[dOff + 3] = d[sOff + 3]!;
        }
      }
    } else {
      // 若两个相邻角色粘连成超大连通域，按各自标准网格范围分配像素
      for (let i = 0; i < isl.pixels.length; i++) {
        const p = isl.pixels[i]!;
        const py = Math.floor(p / sheetW);
        const px = p - py * sheetW;
        const c = Math.min(cols - 1, Math.max(0, Math.floor(px / cellW)));
        const r = Math.min(rows - 1, Math.max(0, Math.floor(py / cellH)));
        const cellIdx = r * cols + c;
        const originX = c * cellW - padX;
        const originY = r * cellH - padY;
        const dx = px - originX;
        const dy = py - originY;
        if (dx >= 0 && dx < extW && dy >= 0 && dy < extH) {
          const dst = cellImgDatas[cellIdx]!.data;
          const sOff = p * 4;
          const dOff = (dy * extW + dx) * 4;
          dst[dOff] = d[sOff]!;
          dst[dOff + 1] = d[sOff + 1]!;
          dst[dOff + 2] = d[sOff + 2]!;
          dst[dOff + 3] = d[sOff + 3]!;
        }
      }
    }
  }

  return cellImgDatas.map((imgData) => {
    const cv = document.createElement('canvas');
    cv.width = extW;
    cv.height = extH;
    cv.getContext('2d')!.putImageData(imgData, 0, 0);
    const pivot = computeCharacterFootPivot(imgData, extW, extH);
    return { canvas: cv, pivot };
  });
}

/**
 * 切分序列帧大图并执行：
 * 1. 背景剔除与网格框线擦除
 * 2. 连通域智能聚类切帧（找回跨格刀尖 + 清除邻格串门残片）
 * 3. 脚底重心对齐 (Bottom-Center Pivot Align)
 * 4. 全序列自适应安全画布防裁断（确保长刀挥砍、高举过顶 100% 零出界、零截断）
 */
export function sliceAndAlignSpriteSheet(
  sourceCanvas: HTMLCanvasElement,
  opts: SpriteSheetProcessOptions
): SpriteFrameItem[] {
  const rows = Math.max(1, Math.min(8, Math.round(opts.rows || 2)));
  const cols = Math.max(1, Math.min(8, Math.round(opts.cols || 4)));
  const baselineRatio = opts.baselineRatio ?? 0.88;
  const useSmartIslands = opts.smartIslandSlice ?? true;
  const preventClipping = opts.preventClipping ?? true;

  // 1. 严格由用户设定控制是否去背景：只有明确开启 removeBg 时，才执行纯色背景快抠
  // 彻底避免未勾选去背景时隐式抠图导致刀光、高光剑气与特效被误抹除！
  let workingSheet = sourceCanvas;
  if (opts.removeBg) {
    workingSheet = removeSolidBackground(sourceCanvas, opts.bgTolerance ?? 34, true);
  }

  // 2. 若开启像素纯化，对整张精灵表统一执行像素纯化（保证所有帧共用同一套像素步长与调色板）
  if (opts.pixelClean) {
    const cleaned = cleanPixelArtCanvas(workingSheet, {
      grid: 'auto',
      palette: 'auto',
      coalesceTolerance: 18,
      hardAlpha: true,
      denoise: true,
      detailMode: 'standard'
    });
    workingSheet = cleaned.canvas;
  }

  const sheetW = workingSheet.width;
  const sheetH = workingSheet.height;
  const baseCellW = Math.max(16, Math.floor(sheetW / cols));
  const baseCellH = Math.max(16, Math.floor(sheetH / rows));

  // 3. 提取每一帧的独立画布与脚底重心锚点（未勾选去背景时直接无损切片原始图像，保护全部刀光特效）
  const extracted = extractCellsByConnectedIslands(
    workingSheet,
    sourceCanvas,
    rows,
    cols,
    useSmartIslands,
    opts.removeBg,
    baselineRatio,
    opts.frameTweaks
  );

  // 4. 计算全序列在对齐到脚底重心后的最大左右跨度与上下高度，自适应扩容安全帧画布，彻底杜绝刀尖/特效出界被截断！
  let outW = baseCellW;
  let outH = baseCellH;

  if (preventClipping) {
    let maxHalfSpanX = baseCellW / 2;
    let maxTopSpanY = Math.round(baseCellH * baselineRatio);
    let maxBotSpanY = Math.round(baseCellH * (1 - baselineRatio));
    // 预留固定安全边距（24px），画布尺寸由全序列内容本身唯一决定且保持完全恒定
    const safetyMargin = 24;

    extracted.forEach((item) => {
      const p = item.pivot;
      if (!p.hasContent) return;
      if (opts.alignPivot) {
        // 关键：不要将 tweak.dx / tweak.dy 计入画布基础尺寸！
        // 画布尺寸必须保持绝对恒定，单帧微调是纯刚体平移，绝不产生缩放抖动
        const leftReach = p.centerX - p.minX + safetyMargin;
        const rightReach = p.maxX - p.centerX + safetyMargin;
        const topReach = p.maxY - p.minY + safetyMargin;
        maxHalfSpanX = Math.max(maxHalfSpanX, leftReach, rightReach);
        maxTopSpanY = Math.max(maxTopSpanY, topReach);
      } else {
        const halfW = (p.maxX - p.minX) / 2 + safetyMargin;
        const fullH = p.maxY - p.minY + safetyMargin * 2;
        maxHalfSpanX = Math.max(maxHalfSpanX, halfW);
        maxTopSpanY = Math.max(maxTopSpanY, Math.round(fullH * baselineRatio));
      }
    });

    const neededW = Math.ceil(maxHalfSpanX * 2);
    const neededHFromTop = Math.ceil(maxTopSpanY / Math.max(0.5, baselineRatio));
    const neededHFromBot = Math.ceil(maxBotSpanY / Math.max(0.08, 1 - baselineRatio));
    const neededH = Math.max(neededHFromTop, neededHFromBot);

    outW = Math.max(baseCellW, neededW + (neededW % 2));
    outH = Math.max(baseCellH, neededH + (neededH % 2));
  }

  const targetBaselineY = Math.round(outH * baselineRatio);
  const targetCenterX = Math.round(outW / 2);

  const frames: SpriteFrameItem[] = [];

  extracted.forEach((item, idx) => {
    const r = Math.floor(idx / cols);
    const c = idx % cols;
    const { canvas: rawCell, pivot } = item;
    const tweak = opts.frameTweaks?.[idx] ?? { dx: 0, dy: 0, enabled: true };

    const outCv = document.createElement('canvas');
    outCv.width = outW;
    outCv.height = outH;
    const octx = outCv.getContext('2d')!;
    octx.imageSmoothingEnabled = !opts.pixelClean;

    if (pivot.hasContent) {
      let shiftX = tweak.dx;
      let shiftY = tweak.dy;
      if (opts.alignPivot) {
        shiftX += Math.round(targetCenterX - pivot.centerX);
        shiftY += Math.round(targetBaselineY - pivot.maxY);
      } else {
        const expLeft = tweak.expandLeft ?? 0;
        const expTop = tweak.expandTop ?? 0;
        shiftX += Math.round((outW - baseCellW) / 2) - expLeft;
        shiftY += Math.round((outH - baseCellH) / 2) - expTop;
      }
      octx.drawImage(rawCell, shiftX, shiftY);
    }

    frames.push({
      index: idx,
      row: r,
      col: c,
      canvas: outCv,
      dataUrl: outCv.toDataURL('image/png'),
      width: outW,
      height: outH,
      hasContent: pivot.hasContent
    });
  });

  return frames;
}

/**
 * 将对齐/筛选后的序列帧重新拼合为标准等宽高网格的 Sprite Sheet 大图
 */
export function composeAlignedSpriteSheet(
  frames: SpriteFrameItem[],
  cols: number
): HTMLCanvasElement {
  const active = frames.length > 0 ? frames : [];
  const first = active[0];
  if (!first) {
    const empty = document.createElement('canvas');
    empty.width = 256;
    empty.height = 256;
    return empty;
  }

  const fw = first.width;
  const fh = first.height;
  const safeCols = Math.max(1, Math.min(cols, active.length));
  const rows = Math.max(1, Math.ceil(active.length / safeCols));

  const sheet = document.createElement('canvas');
  sheet.width = safeCols * fw;
  sheet.height = rows * fh;
  const ctx = sheet.getContext('2d')!;
  ctx.imageSmoothingEnabled = false;

  active.forEach((f, i) => {
    const r = Math.floor(i / safeCols);
    const c = i % safeCols;
    ctx.drawImage(f.canvas, c * fw, r * fh);
  });

  return sheet;
}
