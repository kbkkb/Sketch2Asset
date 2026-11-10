export type PixelGridMode =
  | 'auto'
  | '32'
  | '48'
  | '64'
  | '96'
  | '128'
  | '160'
  | '256'
  | 'block2'
  | 'block3'
  | 'block4'
  | 'block5'
  | 'block6'
  | 'block8'
  | 'block10'
  | 'block12'
  | 'block16'
  | 'block24'
  | 'block32';

export type PixelPaletteMode = 'auto' | '16' | '32' | '64' | 'pico8' | 'gameboy';
export type PixelDetailMode = 'standard' | 'fine2x';

export interface PixelCleanOptions {
  grid?: PixelGridMode;
  manualBlockSize?: number; // 2 ~ 64
  palette?: PixelPaletteMode;
  hardAlpha?: boolean;
  denoise?: boolean;
  /** 同色系噪点归并强度 (0 = 关闭, 默认 18) */
  coalesceTolerance?: number;
  /** 细节保真模式：standard = 1x 主周期规整，fine2x = 2x 倍频子像素保真（保留半格五官/高光） */
  detailMode?: PixelDetailMode;
}

export interface PixelCleanResult {
  canvas: HTMLCanvasElement;
  nativeCanvas: HTMLCanvasElement;
  gw: number;
  gh: number;
  detectedBlockSize: number;
  stepX: number;
  stepY: number;
  colorCount: number;
  cutsX: number[];
  cutsY: number[];
}

export const PALETTE_PICO8: [number, number, number][] = [
  [0, 0, 0],
  [29, 43, 83],
  [126, 37, 83],
  [0, 135, 81],
  [171, 82, 54],
  [95, 87, 79],
  [194, 195, 199],
  [255, 241, 232],
  [255, 0, 77],
  [255, 163, 0],
  [255, 236, 39],
  [0, 228, 54],
  [41, 173, 255],
  [131, 118, 156],
  [255, 119, 168],
  [255, 204, 170]
];

export const PALETTE_GAMEBOY: [number, number, number][] = [
  [15, 56, 15],
  [48, 98, 48],
  [139, 172, 15],
  [155, 188, 15]
];

function colorDistSq(
  r1: number,
  g1: number,
  b1: number,
  r2: number,
  g2: number,
  b2: number
): number {
  const rmean = (r1 + r2) / 2;
  const dr = r1 - r2;
  const dg = g1 - g2;
  const db = b1 - b2;
  return (((512 + rmean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - rmean) * db * db) >> 8);
}

/**
 * 阶段 1：基于边缘跃迁行程直方图 (Run-Length Edge Span Histogram) 的像素周期检测
 * AI 生成的“伪像素画”往往存在网格步长不均匀（如部分列 19px、部分列 21px，甚至水平与垂直步长略有差异）。
 * 传统的全局模运算 (x % s) 会因累积相位漂移而失效并误判为 32px。
 * 本算法直接统计相邻色彩/透明度突变边缘之间的局部跨度直方图，利用基频容差核 [s-1, s, s+1] 与二倍频谐波
 * 精准解出水平与垂直真实像素步长 (stepX, stepY) 以及全局代表块大小 blockSize。
 */
export function detectPixelGrid(
  imgData: ImageData,
  w: number,
  h: number
): {
  blockSize: number;
  stepX: number;
  stepY: number;
  phaseX: number;
  phaseY: number;
  gradX: Float32Array;
  gradY: Float32Array;
} {
  const d = imgData.data;
  const x0 = Math.floor(w * 0.06);
  const x1 = Math.max(x0 + 1, Math.floor(w * 0.94));
  const y0 = Math.floor(h * 0.06);
  const y1 = Math.max(y0 + 1, Math.floor(h * 0.94));

  const gradX = new Float32Array(w);
  const gradY = new Float32Array(h);
  const spanX = new Float32Array(68);
  const spanY = new Float32Array(68);

  // 统计水平边缘梯度与相邻突变间距
  for (let y = y0; y < y1; y += 2) {
    const row = y * w;
    let lastEdgeX = -1;
    for (let x = 1; x < w; x++) {
      const i0 = (row + x - 1) * 4;
      const i1 = (row + x) * 4;
      const a0 = d[i0 + 3] ?? 0;
      const a1 = d[i1 + 3] ?? 0;

      let diff = 0;
      if (Math.abs(a1 - a0) > 96) {
        diff = Math.abs(a1 - a0) * 2;
      } else if (a0 >= 48 && a1 >= 48) {
        diff =
          Math.abs((d[i1] ?? 0) - (d[i0] ?? 0)) +
          Math.abs((d[i1 + 1] ?? 0) - (d[i0 + 1] ?? 0)) +
          Math.abs((d[i1 + 2] ?? 0) - (d[i0 + 2] ?? 0));
      }

      if (diff > 36) {
        gradX[x] = (gradX[x] ?? 0) + diff;
      }
      if (x >= x0 && x < x1 && diff > 42) {
        if (lastEdgeX >= 0) {
          const span = x - lastEdgeX;
          if (span >= 3 && span < 64) {
            spanX[span] = (spanX[span] ?? 0) + diff;
          }
        }
        lastEdgeX = x;
      }
    }
  }

  // 统计垂直边缘梯度与相邻突变间距
  for (let x = x0; x < x1; x += 2) {
    let lastEdgeY = -1;
    for (let y = 1; y < h; y++) {
      const i0 = ((y - 1) * w + x) * 4;
      const i1 = (y * w + x) * 4;
      const a0 = d[i0 + 3] ?? 0;
      const a1 = d[i1 + 3] ?? 0;

      let diff = 0;
      if (Math.abs(a1 - a0) > 96) {
        diff = Math.abs(a1 - a0) * 2;
      } else if (a0 >= 48 && a1 >= 48) {
        diff =
          Math.abs((d[i1] ?? 0) - (d[i0] ?? 0)) +
          Math.abs((d[i1 + 1] ?? 0) - (d[i0 + 1] ?? 0)) +
          Math.abs((d[i1 + 2] ?? 0) - (d[i0 + 2] ?? 0));
      }

      if (diff > 36) {
        gradY[y] = (gradY[y] ?? 0) + diff;
      }
      if (y >= y0 && y < y1 && diff > 42) {
        if (lastEdgeY >= 0) {
          const span = y - lastEdgeY;
          if (span >= 3 && span < 64) {
            spanY[span] = (spanY[span] ?? 0) + diff;
          }
        }
        lastEdgeY = y;
      }
    }
  }

  function findFundamentalStep(hist: Float32Array, maxDim: number): { step: number; confidence: number } {
    let bestStep = 8;
    let bestScore = -1;
    let totalEnergy = 0;
    for (let i = 3; i < 64; i++) totalEnergy += hist[i] ?? 0;
    if (totalEnergy <= 0) return { step: Math.max(4, Math.round(maxDim / 64)), confidence: 0 };

    const maxCandidate = Math.min(36, Math.floor(maxDim / 8));
    for (let s = 4; s <= maxCandidate; s++) {
      const wL = hist[s - 1] ?? 0;
      const wC = hist[s] ?? 0;
      const wR = hist[s + 1] ?? 0;
      // 聚合 [s-1, s, s+1] 容忍 AI 生图 ±1px 的网格抖动
      let score = wL * 0.78 + wC + wR * 0.78;

      // 二倍频谐波增强 (2*s)，防止将 2 个相连同色块误判为基本步长，同时增强真实基频
      if (s * 2 < 64) {
        const h2 =
          (hist[s * 2 - 1] ?? 0) * 0.6 +
          (hist[s * 2] ?? 0) +
          (hist[s * 2 + 1] ?? 0) * 0.6;
        score += 0.35 * h2;
      }

      if (score > bestScore) {
        bestScore = score;
        // 5-tap 质心精化，兼顾相邻双峰（例如 20px 与 22px 交替的网格）
        const wL2 = s >= 5 ? (hist[s - 2] ?? 0) * 0.45 : 0;
        const wR2 = s + 2 < 64 ? (hist[s + 2] ?? 0) * 0.45 : 0;
        const tot = wL2 + wL + wC + wR + wR2;
        bestStep =
          tot > 0
            ? ((s - 2) * wL2 + (s - 1) * wL + s * wC + (s + 1) * wR + (s + 2) * wR2) / tot
            : s;
      }
    }
    return { step: Math.max(3, bestStep), confidence: bestScore / (totalEnergy + 1e-4) };
  }

  const detX = findFundamentalStep(spanX, w);
  const detY = findFundamentalStep(spanY, h);

  let stepX = detX.step;
  let stepY = detY.step;

  // 若 X 与 Y 步长差距过大（> 20%），说明某一轴边缘较少或落入谐波，向高置信度轴对齐
  const relDiff = Math.abs(stepX - stepY) / Math.max(stepX, stepY);
  if (relDiff > 0.2) {
    if (detX.confidence >= detY.confidence) {
      stepY = stepX;
    } else {
      stepX = stepY;
    }
  }

  const blockSize = Math.max(2, Math.round((stepX + stepY) / 2));
  return {
    blockSize,
    stepX,
    stepY,
    phaseX: 0,
    phaseY: 0,
    gradX,
    gradY
  };
}

/**
 * 阶段 2：1D 弹性动态规划网格对齐切分 (Elastic Dynamic Programming Seam Alignment)
 * 解决 AI 生成像素图“网格对不上、局部格子宽窄不一”的核心问题：
 * 在给定目标列数/行数 N 下，允许每一刀切分位置在 [0.72*S, 1.32*S] 范围内弹性伸缩，
 * 自动吸附到真实的像素块色彩突变边缘上；在纯色平坦区域则由弹簧正则项保持均匀网格。
 */
function solveElasticCuts(grad: Float32Array, totalLen: number, targetCount: number): number[] {
  const N = Math.max(4, targetCount);
  const actualStep = totalLen / N;

  // 当格子极小 (< 4px) 时直接退化为均匀网格
  if (actualStep < 3.5) {
    return Array.from({ length: N + 1 }, (_, k) => Math.round(k * actualStep));
  }

  const minStep = Math.max(2, Math.floor(actualStep * 0.78));
  const maxStep = Math.max(minStep + 1, Math.ceil(actualStep * 1.24));

  let sumG = 0;
  let cntG = 0;
  for (let i = 0; i < totalLen; i++) {
    const g = grad[i] ?? 0;
    if (g > 0) {
      sumG += g;
      cntG++;
    }
  }
  const meanG = cntG > 0 ? sumG / cntG : 1;

  const edgeScore = new Float32Array(totalLen + 1);
  for (let i = 1; i < totalLen; i++) {
    const g0 = (grad[i] ?? 0) / meanG;
    const gL = i > 1 ? (grad[i - 1] ?? 0) / meanG : 0;
    const gR = i + 1 < totalLen ? (grad[i + 1] ?? 0) / meanG : 0;
    edgeScore[i] = g0 + 0.25 * Math.max(gL, gR);
  }

  const stride = totalLen + 1;
  const dp = new Float32Array((N + 1) * stride);
  dp.fill(-1e9);
  const parent = new Int32Array((N + 1) * stride);
  dp[0] = 0;

  // 严控全局累积漂移 <= 0.48 个步长，既能吸附局部抖动的 AI 伪像素边缘，又绝不会在重绘到标准等宽网格时发生五官/轮廓移位变形
  const springWeight = 0.85;
  const maxDrift = Math.max(2, Math.round(actualStep * 0.48));

  for (let k = 1; k <= N; k++) {
    const idealPos = Math.round(k * actualStep);
    const pMin = k === N ? totalLen : Math.max(k * minStep, idealPos - maxDrift);
    const pMax = k === N ? totalLen : Math.min(totalLen - (N - k) * minStep, idealPos + maxDrift);
    const curRowOffset = k * stride;
    const prevRowOffset = (k - 1) * stride;

    for (let pos = pMin; pos <= pMax; pos++) {
      let bestPrev = -1;
      let bestVal = -1e9;
      const stepMin = Math.max((k - 1) * minStep, pos - maxStep);
      const stepMax = pos - minStep;

      for (let prev = stepMin; prev <= stepMax; prev++) {
        const prevVal = dp[prevRowOffset + prev] ?? -1e9;
        if (prevVal <= -1e8) continue;
        const step = pos - prev;
        const dev = (step - actualStep) / actualStep;
        const penalty = springWeight * dev * dev * 10;
        const val = prevVal - penalty;
        if (val > bestVal) {
          bestVal = val;
          bestPrev = prev;
        }
      }

      if (bestPrev >= 0) {
        dp[curRowOffset + pos] = bestVal + (pos < totalLen ? (edgeScore[pos] ?? 0) : 0);
        parent[curRowOffset + pos] = bestPrev;
      }
    }
  }

  // 若终点不可达则回退为均匀切分
  if ((dp[N * stride + totalLen] ?? -1e9) <= -1e8) {
    return Array.from({ length: N + 1 }, (_, k) => Math.round(k * actualStep));
  }

  const cuts = new Array<number>(N + 1);
  let cur = totalLen;
  for (let k = N; k >= 0; k--) {
    cuts[k] = cur;
    cur = parent[k * stride + cur] ?? 0;
  }
  return cuts;
}

export function quantizePalette(
  colors: [number, number, number][],
  k: number
): [number, number, number][] {
  if (!colors.length || colors.length <= k) return colors;
  const firstColor = colors[0]!;
  const centroids: [number, number, number][] = [firstColor];

  while (centroids.length < k) {
    let maxDist = -1;
    let bestCandidate: [number, number, number] = firstColor;
    for (const c of colors) {
      let minD = Infinity;
      for (const ct of centroids) {
        const d = colorDistSq(c[0], c[1], c[2], ct[0], ct[1], ct[2]);
        if (d < minD) minD = d;
      }
      if (minD > maxDist) {
        maxDist = minD;
        bestCandidate = c;
      }
    }
    centroids.push(bestCandidate);
  }

  for (let iter = 0; iter < 4; iter++) {
    const clusters = Array.from({ length: centroids.length }, () => ({
      r: 0,
      g: 0,
      b: 0,
      count: 0
    }));
    for (const c of colors) {
      let bestIdx = 0;
      let minDist = Infinity;
      for (let i = 0; i < centroids.length; i++) {
        const ct = centroids[i]!;
        const d = colorDistSq(c[0], c[1], c[2], ct[0], ct[1], ct[2]);
        if (d < minDist) {
          minDist = d;
          bestIdx = i;
        }
      }
      const cl = clusters[bestIdx]!;
      cl.r += c[0];
      cl.g += c[1];
      cl.b += c[2];
      cl.count++;
    }
    for (let i = 0; i < centroids.length; i++) {
      const cl = clusters[i]!;
      if (cl.count > 0) {
        centroids[i] = [
          Math.round(cl.r / cl.count),
          Math.round(cl.g / cl.count),
          Math.round(cl.b / cl.count)
        ];
      }
    }
  }

  return centroids;
}

export function cleanPixelArtCanvas(
  sourceCanvas: HTMLCanvasElement,
  opts: PixelCleanOptions = {}
): PixelCleanResult {
  const {
    grid = 'auto',
    manualBlockSize,
    palette = 'auto',
    hardAlpha = true,
    denoise = true,
    coalesceTolerance = 18,
    detailMode = 'standard'
  } = opts;

  const w = sourceCanvas.width;
  const h = sourceCanvas.height;
  const ctx = sourceCanvas.getContext('2d', { willReadFrequently: true })!;
  const srcData = ctx.getImageData(0, 0, w, h);
  const src = srcData.data;

  // 始终先提取水平与垂直边缘梯度，供弹性网格切分使用
  const det = detectPixelGrid(srcData, w, h);
  let blockSize = det.blockSize;
  let targetStepX = det.stepX;
  let targetStepY = det.stepY;

  if (manualBlockSize && manualBlockSize >= 2 && manualBlockSize <= 64) {
    blockSize = manualBlockSize;
    targetStepX = manualBlockSize;
    targetStepY = manualBlockSize;
  } else if (grid !== 'auto') {
    if (grid.startsWith('block')) {
      blockSize = parseInt(grid.slice(5), 10) || 8;
      targetStepX = blockSize;
      targetStepY = blockSize;
    } else {
      const targetDim = parseInt(grid, 10) || 64;
      targetStepX = Math.max(2, w / targetDim);
      targetStepY = Math.max(2, h / targetDim);
      blockSize = Math.max(2, Math.round((targetStepX + targetStepY) / 2));
    }
  }

  // 若开启 2× 子像素细节保真模式 (fine2x)，将采样网格细分一倍以完整保留 AI 绘制的半格五官/眼神光/纤细结构
  if (detailMode === 'fine2x' && targetStepX >= 4 && targetStepY >= 4) {
    targetStepX = Math.max(2, targetStepX / 2);
    targetStepY = Math.max(2, targetStepY / 2);
    blockSize = Math.max(2, Math.round(blockSize / 2));
  }

  const gw = Math.max(8, Math.round(w / targetStepX));
  const gh = Math.max(8, Math.round(h / targetStepY));

  // 使用 1D 弹性动态规划求解吸附到真实色彩边缘的网格切分线
  const cutsX = solveElasticCuts(det.gradX, w, gw);
  const cutsY = solveElasticCuts(det.gradY, h, gh);

  const gridCells: [number, number, number, number][][] = Array.from({ length: gh }, () =>
    Array.from({ length: gw }, (): [number, number, number, number] => [0, 0, 0, 0])
  );

  // 阶段 3：对每个弹性单元格执行【平坦核心聚类采样 + 1px 深色描边保护】
  for (let gy = 0; gy < gh; gy++) {
    const y0 = cutsY[gy] ?? Math.floor((gy * h) / gh);
    const y1 = Math.max(y0 + 1, cutsY[gy + 1] ?? Math.floor(((gy + 1) * h) / gh));
    const cellH = y1 - y0;
    const marginY = cellH >= 6 ? Math.max(1, Math.floor(cellH * 0.12)) : 0;
    const sy0 = y0 + marginY;
    const sy1 = Math.max(sy0 + 1, y1 - marginY);
    const rowCells = gridCells[gy]!;

    for (let gx = 0; gx < gw; gx++) {
      const x0 = cutsX[gx] ?? Math.floor((gx * w) / gw);
      const x1 = Math.max(x0 + 1, cutsX[gx + 1] ?? Math.floor(((gx + 1) * w) / gw));
      const cellW = x1 - x0;
      const marginX = cellW >= 6 ? Math.max(1, Math.floor(cellW * 0.12)) : 0;
      const sx0 = x0 + marginX;
      const sx1 = Math.max(sx0 + 1, x1 - marginX);

      let opaqueWeight = 0;
      let totalWeight = 0;
      const clusters: {
        r: number;
        g: number;
        b: number;
        w: number;
        sumR: number;
        sumG: number;
        sumB: number;
      }[] = [];

      for (let py = sy0; py < sy1; py++) {
        const ny = sy1 > sy0 + 1 ? Math.abs((py - sy0) / (sy1 - sy0 - 1) - 0.5) * 2 : 0;
        for (let px = sx0; px < sx1; px++) {
          const nx = sx1 > sx0 + 1 ? Math.abs((px - sx0) / (sx1 - sx0 - 1) - 0.5) * 2 : 0;
          const idx = (py * w + px) * 4;
          const a = src[idx + 3] ?? 0;

          // 计算局部像素梯度：严厉降权抗锯齿过渡带上的混合像素，优先采信格子内部纯平像素
          let localGrad = 0;
          if (px > 0 && px + 1 < w) {
            const iL = (py * w + px - 1) * 4;
            const iR = (py * w + px + 1) * 4;
            localGrad +=
              Math.abs((src[iR] ?? 0) - (src[iL] ?? 0)) +
              Math.abs((src[iR + 1] ?? 0) - (src[iL + 1] ?? 0)) +
              Math.abs((src[iR + 2] ?? 0) - (src[iL + 2] ?? 0));
          }
          if (py > 0 && py + 1 < h) {
            const iU = ((py - 1) * w + px) * 4;
            const iD = ((py + 1) * w + px) * 4;
            localGrad +=
              Math.abs((src[iD] ?? 0) - (src[iU] ?? 0)) +
              Math.abs((src[iD + 1] ?? 0) - (src[iU + 1] ?? 0)) +
              Math.abs((src[iD + 2] ?? 0) - (src[iU + 2] ?? 0));
          }

          const flatWeight = localGrad < 40 ? 3.0 : localGrad < 120 ? 1.0 : 0.15;
          const centerWeight = nx < 0.55 && ny < 0.55 ? 2.5 : 1.0;
          const weight = flatWeight * centerWeight;
          totalWeight += weight;

          if (a < (hardAlpha ? 115 : 32)) continue;
          opaqueWeight += weight;

          const r = src[idx] ?? 0;
          const g = src[idx + 1] ?? 0;
          const b = src[idx + 2] ?? 0;

          let matchedCluster: (typeof clusters)[number] | null = null;
          for (let ci = 0; ci < clusters.length; ci++) {
            const cl = clusters[ci]!;
            if (colorDistSq(r, g, b, cl.r, cl.g, cl.b) < 900) {
              matchedCluster = cl;
              break;
            }
          }

          if (matchedCluster) {
            matchedCluster.w += weight;
            matchedCluster.sumR += r * weight;
            matchedCluster.sumG += g * weight;
            matchedCluster.sumB += b * weight;
            matchedCluster.r = Math.round(matchedCluster.sumR / matchedCluster.w);
            matchedCluster.g = Math.round(matchedCluster.sumG / matchedCluster.w);
            matchedCluster.b = Math.round(matchedCluster.sumB / matchedCluster.w);
          } else {
            clusters.push({
              r,
              g,
              b,
              w: weight,
              sumR: r * weight,
              sumG: g * weight,
              sumB: b * weight
            });
          }
        }
      }

      if (clusters.length === 0 || opaqueWeight === 0) {
        rowCells[gx] = [0, 0, 0, 0];
        continue;
      }

      clusters.sort((a, b) => b.w - a.w);
      let best = clusters[0]!;
      const bestLum = 0.299 * best.r + 0.587 * best.g + 0.114 * best.b;

      // 1px 深色轮廓线与高光点防吞噬保护：
      // - 深色描边簇（占比 >= 34% 且明度显著低于主色）优先保留，避免 22% 过激阈值导致黑线变粗吞噬五官
      // - 眼神光/金属反光簇（占比 >= 32% 且高对比亮色）优先保留
      for (let i = 1; i < clusters.length; i++) {
        const cl = clusters[i]!;
        const lum = 0.299 * cl.r + 0.587 * cl.g + 0.114 * cl.b;
        if (cl.w >= opaqueWeight * 0.34 && lum < 82 && bestLum - lum > 72) {
          best = cl;
          break;
        }
        if (cl.w >= opaqueWeight * 0.32 && lum > 212 && lum - bestLum > 75) {
          best = cl;
          break;
        }
      }

      const finalLum = 0.299 * best.r + 0.587 * best.g + 0.114 * best.b;
      const alphaRatioThreshold = finalLum < 95 ? 0.26 : 0.38;
      if (hardAlpha && opaqueWeight < totalWeight * alphaRatioThreshold) {
        rowCells[gx] = [0, 0, 0, 0];
      } else {
        rowCells[gx] = [best.r, best.g, best.b, 255];
      }
    }
  }

  // 阶段 4：两阶段感知色差归并 (Perceptual Palette Coalescing)
  // 消除 AI 生图在同一色块内部产生的细微 RGB 噪点（例如把 138 种杂色收拢为真实的 7~12 种纯平像素色）
  if (coalesceTolerance > 0) {
    const tolSq = coalesceTolerance * coalesceTolerance * 2.5;
    const globalClusters: {
      r: number;
      g: number;
      b: number;
      count: number;
      sumR: number;
      sumG: number;
      sumB: number;
    }[] = [];

    for (let gy = 0; gy < gh; gy++) {
      const rowCells = gridCells[gy]!;
      for (let gx = 0; gx < gw; gx++) {
        const [r, g, b, a] = rowCells[gx]!;
        if (a === 0) continue;
        let found: (typeof globalClusters)[number] | null = null;
        for (let i = 0; i < globalClusters.length; i++) {
          const cl = globalClusters[i]!;
          if (colorDistSq(r, g, b, cl.r, cl.g, cl.b) <= tolSq) {
            found = cl;
            break;
          }
        }
        if (found) {
          found.count++;
          found.sumR += r;
          found.sumG += g;
          found.sumB += b;
          found.r = Math.round(found.sumR / found.count);
          found.g = Math.round(found.sumG / found.count);
          found.b = Math.round(found.sumB / found.count);
        } else {
          globalClusters.push({ r, g, b, count: 1, sumR: r, sumG: g, sumB: b });
        }
      }
    }

    globalClusters.sort((a, b) => b.count - a.count);

    // 第二趟：仅将明度相近的边缘过渡杂色簇吸附至主色簇，严禁吞并高反差的瞳孔/高光小色块
    const majorPalette: typeof globalClusters = [];
    for (const cl of globalClusters) {
      let target: (typeof globalClusters)[number] | null = null;
      const clLum = 0.299 * cl.r + 0.587 * cl.g + 0.114 * cl.b;
      const mergeLimit = cl.count <= 3 ? tolSq * 1.35 : tolSq * 0.92;
      let minDist = Infinity;
      for (const p of majorPalette) {
        const pLum = 0.299 * p.r + 0.587 * p.g + 0.114 * p.b;
        if (Math.abs(clLum - pLum) > 28) continue;
        const d = colorDistSq(cl.r, cl.g, cl.b, p.r, p.g, p.b);
        if (d <= mergeLimit && d < minDist) {
          minDist = d;
          target = p;
        }
      }
      if (target) {
        target.count += cl.count;
        target.sumR += cl.sumR;
        target.sumG += cl.sumG;
        target.sumB += cl.sumB;
        target.r = Math.round(target.sumR / target.count);
        target.g = Math.round(target.sumG / target.count);
        target.b = Math.round(target.sumB / target.count);
      } else {
        majorPalette.push({ ...cl });
      }
    }

    if (majorPalette.length > 0) {
      const firstMajor = majorPalette[0]!;
      for (let gy = 0; gy < gh; gy++) {
        const rowCells = gridCells[gy]!;
        for (let gx = 0; gx < gw; gx++) {
          const c = rowCells[gx]!;
          if (c[3] === 0) continue;
          let bestP = firstMajor;
          let minD = Infinity;
          for (const p of majorPalette) {
            const d = colorDistSq(c[0], c[1], c[2], p.r, p.g, p.b);
            if (d < minD) {
              minD = d;
              bestP = p;
            }
          }
          rowCells[gx] = [bestP.r, bestP.g, bestP.b, 255];
        }
      }
    }
  }

  // 若用户指定了固定调色板（16色 / 32色 / 64色 / PICO-8 / GameBoy），进一步执行调色板映射
  let activePalette: [number, number, number][] | null = null;
  if (palette === 'pico8') {
    activePalette = PALETTE_PICO8;
  } else if (palette === 'gameboy') {
    activePalette = PALETTE_GAMEBOY;
  } else if (palette === '16' || palette === '32' || palette === '64') {
    const k = parseInt(palette, 10);
    const currentColors: [number, number, number][] = [];
    for (let gy = 0; gy < gh; gy++) {
      const rowCells = gridCells[gy]!;
      for (let gx = 0; gx < gw; gx++) {
        const c = rowCells[gx]!;
        if (c[3] > 0) currentColors.push([c[0], c[1], c[2]]);
      }
    }
    activePalette = quantizePalette(currentColors, k);
  }

  if (activePalette && activePalette.length > 0) {
    const firstPal = activePalette[0]!;
    for (let gy = 0; gy < gh; gy++) {
      const rowCells = gridCells[gy]!;
      for (let gx = 0; gx < gw; gx++) {
        const c = rowCells[gx]!;
        if (c[3] === 0) continue;
        let minDist = Infinity;
        let bestColor: [number, number, number] = firstPal;
        for (const p of activePalette) {
          const d = colorDistSq(c[0], c[1], c[2], p[0], p[1], p[2]);
          if (d < minDist) {
            minDist = d;
            bestColor = p;
          }
        }
        rowCells[gx] = [bestColor[0], bestColor[1], bestColor[2], 255];
      }
    }
  }

  // 孤立单格杂点过滤 (Single-cell Salt-and-Pepper Denoise)
  if (denoise) {
    for (let gy = 1; gy < gh - 1; gy++) {
      const rowCells = gridCells[gy]!;
      const upRow = gridCells[gy - 1]!;
      const downRow = gridCells[gy + 1]!;
      for (let gx = 1; gx < gw - 1; gx++) {
        const c = rowCells[gx]!;
        if (c[3] === 0) continue;
        const up = upRow[gx]!;
        const down = downRow[gx]!;
        const left = rowCells[gx - 1]!;
        const right = rowCells[gx + 1]!;
        if (up[3] && down[3] && left[3] && right[3]) {
          if (
            up[0] === down[0] &&
            up[1] === down[1] &&
            up[2] === down[2] &&
            up[0] === left[0] &&
            up[1] === left[1] &&
            up[2] === left[2] &&
            up[0] === right[0] &&
            up[1] === right[1] &&
            up[2] === right[2]
          ) {
            rowCells[gx] = [up[0], up[1], up[2], 255];
          }
        }
      }
    }
  }

  // 生成高分辨率规整网格画布（将原本宽窄不一的 AI 伪像素格重绘为严格等宽等高的标准像素网格）
  const fullCanvas = document.createElement('canvas');
  fullCanvas.width = w;
  fullCanvas.height = h;
  const fctx = fullCanvas.getContext('2d')!;
  const fullImgData = fctx.createImageData(w, h);
  const out = fullImgData.data;

  const uniformStepX = w / gw;
  const uniformStepY = h / gh;

  for (let gy = 0; gy < gh; gy++) {
    const y0 = Math.floor(gy * uniformStepY);
    const y1 = Math.min(h, Math.floor((gy + 1) * uniformStepY));
    const rowCells = gridCells[gy]!;
    for (let gx = 0; gx < gw; gx++) {
      const x0 = Math.floor(gx * uniformStepX);
      const x1 = Math.min(w, Math.floor((gx + 1) * uniformStepX));
      const [cr, cg, cb, ca] = rowCells[gx]!;
      for (let py = y0; py < y1; py++) {
        for (let px = x0; px < x1; px++) {
          const idx = (py * w + px) * 4;
          out[idx] = cr;
          out[idx + 1] = cg;
          out[idx + 2] = cb;
          out[idx + 3] = ca;
        }
      }
    }
  }
  fctx.putImageData(fullImgData, 0, 0);

  // 生成 1:1 原生低分辨率像素画布（如 62x58 / 64x64）
  const nativeCanvas = document.createElement('canvas');
  nativeCanvas.width = gw;
  nativeCanvas.height = gh;
  const nctx = nativeCanvas.getContext('2d')!;
  const nativeImgData = nctx.createImageData(gw, gh);
  const nout = nativeImgData.data;
  const uniqueColorMap = new Set<string>();

  for (let gy = 0; gy < gh; gy++) {
    const rowCells = gridCells[gy]!;
    for (let gx = 0; gx < gw; gx++) {
      const [cr, cg, cb, ca] = rowCells[gx]!;
      const idx = (gy * gw + gx) * 4;
      nout[idx] = cr;
      nout[idx + 1] = cg;
      nout[idx + 2] = cb;
      nout[idx + 3] = ca;
      if (ca > 0) uniqueColorMap.add(`${cr},${cg},${cb}`);
    }
  }
  nctx.putImageData(nativeImgData, 0, 0);

  return {
    canvas: fullCanvas,
    nativeCanvas,
    gw,
    gh,
    detectedBlockSize: blockSize,
    stepX: Number(targetStepX.toFixed(1)),
    stepY: Number(targetStepY.toFixed(1)),
    colorCount: uniqueColorMap.size,
    cutsX,
    cutsY
  };
}

/**
 * 生图前置虚拟网格规范化 (Pre-generation Virtual Pixel Grid Snap)
 * 将草图或参考图预先吸附到严格等宽的逻辑像素网格（如 64×64）并以最近邻无模糊放大回 1024×1024，
 * 从而在图生图/垫图生成阶段引导 AI 的自注意力机制遵循统一像素步长，大幅抑制大小像素混用 (Mixels)。
 */
export async function snapDataUrlToVirtualGrid(
  dataUrl: string,
  targetGrid = 64
): Promise<string> {
  return new Promise((resolve) => {
    const img = new Image();
    img.onload = () => {
      const w = img.naturalWidth || img.width || 1024;
      const h = img.naturalHeight || img.height || 1024;
      const aspect = w / h;
      const gw = aspect >= 1 ? targetGrid : Math.max(16, Math.round(targetGrid * aspect));
      const gh = aspect >= 1 ? Math.max(16, Math.round(targetGrid / aspect)) : targetGrid;

      const lowCv = document.createElement('canvas');
      lowCv.width = gw;
      lowCv.height = gh;
      const lctx = lowCv.getContext('2d')!;
      lctx.imageSmoothingEnabled = false;
      lctx.drawImage(img, 0, 0, gw, gh);

      const outCv = document.createElement('canvas');
      outCv.width = w;
      outCv.height = h;
      const octx = outCv.getContext('2d')!;
      octx.imageSmoothingEnabled = false;
      octx.drawImage(lowCv, 0, 0, w, h);
      resolve(outCv.toDataURL('image/png'));
    };
    img.onerror = () => resolve(dataUrl);
    img.src = dataUrl;
  });
}
