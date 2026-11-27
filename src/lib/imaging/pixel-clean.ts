export type PixelGridMode =
  | 'auto'
  | '16'
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

export type PixelPaletteMode =
  | 'auto'
  | 'auto_cel'
  | '16'
  | '24'
  | '32'
  | '48'
  | '64'
  | 'pico8'
  | 'gameboy';

export type PixelDetailMode = 'standard' | 'fine2x';
export type PixelAlignMode = 'uniform' | 'adaptive';

export interface PixelCleanOptions {
  grid?: PixelGridMode;
  manualBlockSize?: number; // 2 ~ 64
  /** 网格对齐模式：uniform = 严格等宽相位锁格（SpriteFusion/pixel-perfect 标准，零扭曲），adaptive = 弹性边缘游走 */
  alignMode?: PixelAlignMode;
  /** 可选先验网格提示（如资产记录中的 [grid-64x64] 元信息） */
  hintGrid?: { gw: number; gh: number } | null;
  palette?: PixelPaletteMode;
  hardAlpha?: boolean;
  /** 兼容旧调用字段 */
  denoise?: boolean;
  /**
   * OKLAB 共线夹心抗锯齿剥离强度 (0 ~ 100，默认 0 = 纯单格内统一，跨格零改动)
   * 基于 lovelaced/pixfix 三角不等式共线判定：仅消除严格夹在相邻异色连线中间的 1 格过渡灰边，
   * 数学上不可能凭空变黑或抹除独立高光/五官/阴影阶梯。
   */
  aaStripStrength?: number;
  /** 兼容旧调用字段（若传入且未指定 aaStripStrength，映射为 aaStripStrength） */
  coalesceTolerance?: number;
  /** 细节保真模式：standard = 1x 原网格，fine2x = 2x 子像素细分 */
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
  phaseX: number;
  phaseY: number;
  colorCount: number;
  cutsX: number[];
  cutsY: number[];
  /** 是否命中 1:1 无损网格锁死 */
  exactLocked: boolean;
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

// ── Perceptual OKLAB Color Space (Björn Ottosson exact formulation + LUT) ──
const SRGB_TO_LINEAR = new Float32Array(256);
for (let i = 0; i < 256; i++) {
  const c = i / 255;
  SRGB_TO_LINEAR[i] = c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

export function rgbToOklab(r: number, g: number, b: number): [number, number, number] {
  const lr = SRGB_TO_LINEAR[r & 0xff] ?? 0;
  const lg = SRGB_TO_LINEAR[g & 0xff] ?? 0;
  const lb = SRGB_TO_LINEAR[b & 0xff] ?? 0;

  const l = Math.cbrt(0.4122214708 * lr + 0.5363325363 * lg + 0.0514459929 * lb);
  const m = Math.cbrt(0.2119034982 * lr + 0.6806995451 * lg + 0.1073969566 * lb);
  const s = Math.cbrt(0.0883024619 * lr + 0.2817188376 * lg + 0.6299787005 * lb);

  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  ];
}

function oklabDistSq(
  L1: number,
  a1: number,
  b1: number,
  L2: number,
  a2: number,
  b2: number
): number {
  const dL = L1 - L2;
  const da = a1 - a2;
  const db = b1 - b2;
  return dL * dL + da * da + db * db;
}

interface AxisGridSolution {
  step: number;
  phase: number;
  count: number;
  cuts: number[];
  exact: boolean;
  ratio: number;
}

/**
 * 构建 1D 轴向相邻像素跳变能量谱（不设高门槛截断，连细微明暗阶梯跃迁也一并纳入重构残差评估）
 */
function computeAxisTransitionProfiles(
  imgData: ImageData,
  w: number,
  h: number
): { gradX: Float64Array; gradY: Float64Array } {
  const d = imgData.data;
  const gradX = new Float64Array(w);
  const gradY = new Float64Array(h);

  const strideY = h > 512 ? 2 : 1;
  for (let y = 0; y < h; y += strideY) {
    const rowOff = y * w;
    for (let x = 1; x < w; x++) {
      const i0 = (rowOff + x - 1) * 4;
      const i1 = (rowOff + x) * 4;
      const a0 = d[i0 + 3] ?? 0;
      const a1 = d[i1 + 3] ?? 0;
      if (a0 < 16 && a1 < 16) continue;
      const da = Math.abs(a1 - a0);
      const dc =
        a0 >= 16 && a1 >= 16
          ? Math.abs((d[i1] ?? 0) - (d[i0] ?? 0)) +
            Math.abs((d[i1 + 1] ?? 0) - (d[i0 + 1] ?? 0)) +
            Math.abs((d[i1 + 2] ?? 0) - (d[i0 + 2] ?? 0))
          : 0;
      const diff = dc + da * 2;
      if (diff > 3) {
        gradX[x] = (gradX[x] ?? 0) + diff;
      }
    }
  }

  const strideX = w > 512 ? 2 : 1;
  for (let x = 0; x < w; x += strideX) {
    for (let y = 1; y < h; y++) {
      const i0 = ((y - 1) * w + x) * 4;
      const i1 = (y * w + x) * 4;
      const a0 = d[i0 + 3] ?? 0;
      const a1 = d[i1 + 3] ?? 0;
      if (a0 < 16 && a1 < 16) continue;
      const da = Math.abs(a1 - a0);
      const dc =
        a0 >= 16 && a1 >= 16
          ? Math.abs((d[i1] ?? 0) - (d[i0] ?? 0)) +
            Math.abs((d[i1 + 1] ?? 0) - (d[i0 + 1] ?? 0)) +
            Math.abs((d[i1 + 2] ?? 0) - (d[i0 + 2] ?? 0))
          : 0;
      const diff = dc + da * 2;
      if (diff > 3) {
        gradY[y] = (gradY[y] ?? 0) + diff;
      }
    }
  }

  return { gradX, gradY };
}

function buildSteppedCuts(len: number, step: number, phase: number): number[] {
  const cuts: number[] = [0];
  let pos = phase > 0 ? phase : step;
  // 若首个偏移块过窄 (< 0.35 * step)，并入首格避免产生 1~2px 碎边格
  if (pos < step * 0.35) {
    pos += step;
  }
  while (pos < len - step * 0.35) {
    const c = Math.round(pos);
    if (c > (cuts[cuts.length - 1] ?? 0)) {
      cuts.push(c);
    }
    pos += step;
  }
  cuts.push(len);
  return cuts;
}

/**
 * 基于重构残差 (Reconstruction-Residual Scoring, 借鉴 ventz/pixel-perfect) 求解单轴最优 (step, phase)：
 * 1. 先检查是否存在精确整数步长 + 任意裁剪相位 (S, phase)，使 >=99.3% 的跳变能量严格落在切线上（格内残差 ≈ 0）；
 * 2. 再检查精确有理等分网格 N（如 48×48、96×96 在 1024px 画布上的四舍五入切线）；
 * 3. 若为含抗锯齿/抖动的外部伪像素图，则搜索最大化「切线带能量 / 格心残差能量」比值的 (S, phase)，并执行半频基波校验。
 */
function solveAxisByResidual(grad: Float64Array, len: number): AxisGridSolution {
  let totalEnergy = 0;
  for (let i = 1; i < len; i++) {
    totalEnergy += grad[i] ?? 0;
  }

  if (totalEnergy <= 1e-3 || len < 16) {
    const defaultCount = Math.min(64, Math.max(8, Math.round(len / 16)));
    const cuts = Array.from({ length: defaultCount + 1 }, (_, k) =>
      Math.round((k * len) / defaultCount)
    );
    return {
      step: len / defaultCount,
      phase: 0,
      count: defaultCount,
      cuts,
      exact: false,
      ratio: 1
    };
  }

  const maxIntStep = Math.min(64, Math.floor(len / 8));

  // Tier 1: 精确整数步长 + 任意相位自检（同时覆盖未裁剪原图与任意裁剪后的真像素图！）
  for (let S = maxIntStep; S >= 2; S--) {
    let bestPhase = -1;
    let bestBoundaryEnergy = -1;
    for (let ph = 0; ph < S; ph++) {
      let bEnergy = 0;
      for (let x = ph === 0 ? S : ph; x < len; x += S) {
        bEnergy += grad[x] ?? 0;
      }
      if (bEnergy > bestBoundaryEnergy) {
        bestBoundaryEnergy = bEnergy;
        bestPhase = ph;
      }
    }
    const interiorResidual = totalEnergy - bestBoundaryEnergy;
    if (bestPhase >= 0 && interiorResidual <= totalEnergy * 0.007) {
      const cuts = buildSteppedCuts(len, S, bestPhase);
      return {
        step: S,
        phase: bestPhase,
        count: cuts.length - 1,
        cuts,
        exact: true,
        ratio: 1000
      };
    }
  }

  // Tier 2: 精确有理数等分网格自检（如 48、96、128 等非 2 次幂档位在 1024px 画布上的等分切线）
  for (let N = 12; N <= Math.min(256, Math.floor(len / 3)); N++) {
    let bEnergy = 0;
    for (let k = 1; k < N; k++) {
      const c = Math.round((k * len) / N);
      bEnergy += grad[c] ?? 0;
    }
    if (totalEnergy - bEnergy <= totalEnergy * 0.008) {
      const cuts = Array.from({ length: N + 1 }, (_, k) => Math.round((k * len) / N));
      return {
        step: len / N,
        phase: 0,
        count: N,
        cuts,
        exact: true,
        ratio: 900
      };
    }
  }

  // Tier 3: 含抗锯齿/边缘抖动的 AI 像素图 — 最大化「切线带均能 / 格心残差均能」
  const globalMean = totalEnergy / Math.max(1, len - 1);
  const maxFuzzyStep = Math.min(48, maxIntStep);
  const bestRatioPerS = new Float64Array(maxFuzzyStep + 1);
  const bestPhasePerS = new Int32Array(maxFuzzyStep + 1);

  let topS = 16;
  let topRatio = -1;

  for (let S = 4; S <= maxFuzzyStep; S++) {
    const seamR = S >= 10 ? 1 : 0;
    const bandWidth = 1 + 2 * seamR;
    const coreWidth = Math.max(1, S - bandWidth);

    let bestPh = 0;
    let bestScore = -1;

    for (let ph = 0; ph < S; ph++) {
      let bandSum = 0;
      for (let base = ph; base < len + S; base += S) {
        for (let d = -seamR; d <= seamR; d++) {
          const x = base + d;
          if (x >= 1 && x < len) {
            bandSum += grad[x] ?? 0;
          }
        }
      }
      const coreSum = Math.max(0, totalEnergy - bandSum);
      const numCells = Math.max(1, len / S);
      const meanBand = bandSum / (numCells * bandWidth);
      const meanCore = coreSum / (numCells * coreWidth);
      const ratio = meanBand / (meanCore + 0.12 * globalMean + 1e-6);

      if (ratio > bestScore) {
        bestScore = ratio;
        bestPh = ph;
      }
    }

    bestRatioPerS[S] = bestScore;
    bestPhasePerS[S] = bestPh;
    if (bestScore > topRatio) {
      topRatio = bestScore;
      topS = S;
    }
  }

  // 反二倍频校验：若 S/2 同样具有高切线/格心信噪比，说明 topS 是连续两格平涂造成的二倍频，回落至真实基频 S/2
  const halfS = Math.round(topS / 2);
  if (halfS >= 4 && (bestRatioPerS[halfS] ?? 0) >= topRatio * 0.72) {
    topS = halfS;
    topRatio = bestRatioPerS[halfS] ?? topRatio;
  }

  const chosenPhase = bestPhasePerS[topS] ?? 0;
  const cuts = buildSteppedCuts(len, topS, chosenPhase);
  return {
    step: topS,
    phase: chosenPhase,
    count: cuts.length - 1,
    cuts,
    exact: false,
    ratio: topRatio
  };
}

function findBestPhaseForStep(grad: Float64Array, len: number, S: number): number {
  const stepInt = Math.max(2, Math.round(S));
  const seamR = stepInt >= 10 ? 1 : 0;
  let bestPh = 0;
  let bestSum = -1;
  for (let ph = 0; ph < stepInt; ph++) {
    let sum = 0;
    for (let base = ph; base < len + stepInt; base += stepInt) {
      for (let d = -seamR; d <= seamR; d++) {
        const x = base + d;
        if (x >= 1 && x < len) sum += grad[x] ?? 0;
      }
    }
    if (sum > bestSum) {
      bestSum = sum;
      bestPh = ph;
    }
  }
  return bestPh;
}

/**
 * 精确整数网格快速只读检测（供 Studio 载入图片时即时点亮“1:1 精确锁格”状态徽章）
 */
export function detectExactUniformGrid(
  imgData: ImageData,
  w: number,
  h: number
): { blockSize: number; gw: number; gh: number; phaseX: number; phaseY: number } | null {
  const { gradX, gradY } = computeAxisTransitionProfiles(imgData, w, h);
  const solX = solveAxisByResidual(gradX, w);
  const solY = solveAxisByResidual(gradY, h);
  if (solX.exact && solY.exact) {
    const relDiff = Math.abs(solX.step - solY.step) / Math.max(solX.step, solY.step);
    if (relDiff < 0.08) {
      return {
        blockSize: Math.max(2, Math.round((solX.step + solY.step) / 2)),
        gw: solX.count,
        gh: solY.count,
        phaseX: solX.phase,
        phaseY: solY.phase
      };
    }
  }
  return null;
}

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
  exact: boolean;
  cutsX: number[];
  cutsY: number[];
  gradX: Float32Array;
  gradY: Float32Array;
} {
  const { gradX, gradY } = computeAxisTransitionProfiles(imgData, w, h);
  let solX = solveAxisByResidual(gradX, w);
  let solY = solveAxisByResidual(gradY, h);

  // 若某一轴非精确且两轴步长不一致，向信噪比更高的轴统一正方形像素步长
  if (!(solX.exact && solY.exact)) {
    const relDiff = Math.abs(solX.step - solY.step) / Math.max(solX.step, solY.step);
    if (relDiff > 0.06) {
      if (solX.ratio >= solY.ratio) {
        const s = solX.step;
        const phY = findBestPhaseForStep(gradY, h, s);
        const cutsY = buildSteppedCuts(h, s, phY);
        solY = { step: s, phase: phY, count: cutsY.length - 1, cuts: cutsY, exact: false, ratio: solX.ratio };
      } else {
        const s = solY.step;
        const phX = findBestPhaseForStep(gradX, w, s);
        const cutsX = buildSteppedCuts(w, s, phX);
        solX = { step: s, phase: phX, count: cutsX.length - 1, cuts: cutsX, exact: false, ratio: solY.ratio };
      }
    }
  }

  const gX32 = new Float32Array(w);
  const gY32 = new Float32Array(h);
  for (let i = 0; i < w; i++) gX32[i] = gradX[i] ?? 0;
  for (let i = 0; i < h; i++) gY32[i] = gradY[i] ?? 0;

  return {
    blockSize: Math.max(2, Math.round((solX.step + solY.step) / 2)),
    stepX: solX.step,
    stepY: solY.step,
    phaseX: solX.phase,
    phaseY: solY.phase,
    exact: solX.exact && solY.exact,
    cutsX: solX.cuts,
    cutsY: solY.cuts,
    gradX: gX32,
    gradY: gY32
  };
}

/**
 * 弹性边缘游走切分（Adaptive 模式：借鉴 SpriteFusion grid walker，仅在用户显式开启“弹性纠偏”时处理严重歪斜的伪像素图）
 */
function solveAdaptiveCuts(grad: Float32Array, totalLen: number, targetCount: number): number[] {
  const N = Math.max(4, targetCount);
  const actualStep = totalLen / N;
  if (actualStep < 3.5) {
    return Array.from({ length: N + 1 }, (_, k) => Math.round(k * actualStep));
  }

  const minStep = Math.max(2, Math.floor(actualStep * 0.8));
  const maxStep = Math.max(minStep + 1, Math.ceil(actualStep * 1.22));

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

  const stride = totalLen + 1;
  const dp = new Float32Array((N + 1) * stride);
  dp.fill(-1e9);
  const parent = new Int32Array((N + 1) * stride);
  dp[0] = 0;

  const springWeight = 1.15;
  const maxDrift = Math.max(2, Math.round(actualStep * 0.38));

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
        const dev = (pos - prev - actualStep) / actualStep;
        const val = prevVal - springWeight * dev * dev * 10;
        if (val > bestVal) {
          bestVal = val;
          bestPrev = prev;
        }
      }

      if (bestPrev >= 0) {
        const edgeBonus = pos < totalLen ? (grad[pos] ?? 0) / meanG : 0;
        dp[curRowOffset + pos] = bestVal + edgeBonus;
        parent[curRowOffset + pos] = bestPrev;
      }
    }
  }

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

/**
 * 核心机制：Astropulse `k-Centroid (k=2)` + Majority Vote 单格独立真色提取
 * 每个网格单元完全独立运算（跨格 0 干扰）：
 * 1. 若格内子像素本身已纯平，O(1) 无损原样返回（100% 幂等恒等）；
 * 2. 若格内含抗锯齿过渡、JPEG 噪点或相邻格溢色，在 OKLAB 空间执行 k=2 局部聚类剥离边缝杂色，
 *    并在获胜主簇中选出出现权重最高的【真实原图像素色（Exact Mode / Medoid）】，绝不求模糊均值，也绝不搞暗色越权夺格。
 */
function sampleCellKCentroid(
  src: Uint8ClampedArray,
  w: number,
  h: number,
  x0: number,
  x1: number,
  y0: number,
  y1: number,
  hardAlpha: boolean
): [number, number, number, number] {
  const cw = x1 - x0;
  const ch = y1 - y0;

  // 1. 快速恒等分支：若格内全部像素完全一致，直接 0 损耗返回
  const firstIdx = (y0 * w + x0) * 4;
  const r0 = src[firstIdx] ?? 0;
  const g0 = src[firstIdx + 1] ?? 0;
  const b0 = src[firstIdx + 2] ?? 0;
  const a0 = src[firstIdx + 3] ?? 0;
  let allIdentical = true;

  outerFast: for (let py = y0; py < y1; py++) {
    const rowOff = py * w;
    for (let px = x0; px < x1; px++) {
      const idx = (rowOff + px) * 4;
      if (
        (src[idx] ?? 0) !== r0 ||
        (src[idx + 1] ?? 0) !== g0 ||
        (src[idx + 2] ?? 0) !== b0 ||
        (src[idx + 3] ?? 0) !== a0
      ) {
        allIdentical = false;
        break outerFast;
      }
    }
  }

  if (allIdentical) {
    if (hardAlpha ? a0 < 120 : a0 < 16) return [0, 0, 0, 0];
    return [r0, g0, b0, hardAlpha ? 255 : a0];
  }

  // 2. 统计不透明度权重与候选真实颜色桶（核心区高权重，最外 1px 接缝边降权以滤除邻居溢色与抗锯齿）
  const hasSeamMargin = cw >= 4 && ch >= 4;
  let opaqueWeight = 0;
  let totalWeight = 0;
  let weightedAlphaSum = 0;

  const colorBuckets = new Map<
    number,
    { r: number; g: number; b: number; L: number; oa: number; ob: number; weight: number }
  >();

  for (let py = y0; py < y1; py++) {
    const isEdgeY = hasSeamMargin && (py === y0 || py === y1 - 1);
    const rowOff = py * w;
    for (let px = x0; px < x1; px++) {
      const isEdgeX = hasSeamMargin && (px === x0 || px === x1 - 1);
      const spatialW = isEdgeX && isEdgeY ? 0.25 : isEdgeX || isEdgeY ? 0.45 : 2.0;

      const idx = (rowOff + px) * 4;
      const a = src[idx + 3] ?? 0;
      totalWeight += spatialW;

      if (a < (hardAlpha ? 115 : 24)) continue;

      // 降权高梯度过渡像素（抗锯齿模糊半透明/混色边），让格内平坦实色主导投票
      let localGrad = 0;
      if (px > x0 && px + 1 < x1) {
        const iL = (rowOff + px - 1) * 4;
        const iR = (rowOff + px + 1) * 4;
        localGrad +=
          Math.abs((src[iR] ?? 0) - (src[iL] ?? 0)) +
          Math.abs((src[iR + 1] ?? 0) - (src[iL + 1] ?? 0)) +
          Math.abs((src[iR + 2] ?? 0) - (src[iL + 2] ?? 0));
      }
      if (py > y0 && py + 1 < y1) {
        const iU = ((py - 1) * w + px) * 4;
        const iD = ((py + 1) * w + px) * 4;
        localGrad +=
          Math.abs((src[iD] ?? 0) - (src[iU] ?? 0)) +
          Math.abs((src[iD + 1] ?? 0) - (src[iU + 1] ?? 0)) +
          Math.abs((src[iD + 2] ?? 0) - (src[iU + 2] ?? 0));
      }

      const flatW = 1.0 / (1.0 + localGrad / 64.0);
      const wSample = spatialW * flatW * (a / 255);
      opaqueWeight += spatialW;
      weightedAlphaSum += a * spatialW;

      const r = src[idx] ?? 0;
      const g = src[idx + 1] ?? 0;
      const b = src[idx + 2] ?? 0;
      const key = ((r & 0xff) << 16) | ((g & 0xff) << 8) | (b & 0xff);
      const existing = colorBuckets.get(key);
      if (existing) {
        existing.weight += wSample;
      } else {
        const [L, oa, ob] = rgbToOklab(r, g, b);
        colorBuckets.set(key, { r, g, b, L, oa, ob, weight: wSample });
      }
    }
  }

  if (colorBuckets.size === 0 || opaqueWeight < totalWeight * (hardAlpha ? 0.42 : 0.15)) {
    return [0, 0, 0, 0];
  }

  const outAlpha = hardAlpha ? 255 : Math.min(255, Math.max(1, Math.round(weightedAlphaSum / opaqueWeight)));
  const buckets = Array.from(colorBuckets.values());
  if (buckets.length === 1) {
    const only = buckets[0]!;
    return [only.r, only.g, only.b, outAlpha];
  }

  // 按权重降序排列：最高权重桶作为 k=2 的主锚点 Seed 0
  buckets.sort((a, b) => b.weight - a.weight);
  const seed0 = buckets[0]!;

  // 寻找与 Seed 0 在 OKLAB 空间距离最远的显著次簇锚点 Seed 1
  let seed1 = seed0;
  let maxWeightedDist = -1;
  for (let i = 1; i < buckets.length; i++) {
    const b = buckets[i]!;
    const dSq = oklabDistSq(seed0.L, seed0.oa, seed0.ob, b.L, b.oa, b.ob);
    const score = dSq * Math.pow(b.weight, 0.35);
    if (score > maxWeightedDist) {
      maxWeightedDist = score;
      seed1 = b;
    }
  }

  const initSepSq = oklabDistSq(seed0.L, seed0.oa, seed0.ob, seed1.L, seed1.oa, seed1.ob);

  // 若格内所有颜色本身极为接近（仅 JPEG 微噪点，无第二色块渗入），直接在全格求加权 OKLAB 质心并回贴最近真实桶（Medoid）
  if (initSepSq < 0.0012) {
    let sumL = 0;
    let sumA = 0;
    let sumB = 0;
    let sumW = 0;
    for (const b of buckets) {
      sumL += b.L * b.weight;
      sumA += b.oa * b.weight;
      sumB += b.ob * b.weight;
      sumW += b.weight;
    }
    const cL = sumL / sumW;
    const cA = sumA / sumW;
    const cB = sumB / sumW;

    let bestBucket = seed0;
    let bestCost = Infinity;
    for (const b of buckets) {
      const cost = oklabDistSq(b.L, b.oa, b.ob, cL, cA, cB) / Math.pow(b.weight, 0.25);
      if (cost < bestCost) {
        bestCost = cost;
        bestBucket = b;
      }
    }
    return [bestBucket.r, bestBucket.g, bestBucket.b, outAlpha];
  }

  // k=2 OKLAB 局部聚类迭代（分离格内主体色 vs 隔壁渗入色/描边边角）
  let c0L = seed0.L, c0A = seed0.oa, c0B = seed0.ob;
  let c1L = seed1.L, c1A = seed1.oa, c1B = seed1.ob;
  let w0 = 0;
  let w1 = 0;

  for (let iter = 0; iter < 3; iter++) {
    let s0L = 0, s0A = 0, s0B = 0, sw0 = 0;
    let s1L = 0, s1A = 0, s1B = 0, sw1 = 0;

    for (const b of buckets) {
      const d0 = oklabDistSq(b.L, b.oa, b.ob, c0L, c0A, c0B);
      const d1 = oklabDistSq(b.L, b.oa, b.ob, c1L, c1A, c1B);
      if (d0 <= d1) {
        s0L += b.L * b.weight;
        s0A += b.oa * b.weight;
        s0B += b.ob * b.weight;
        sw0 += b.weight;
      } else {
        s1L += b.L * b.weight;
        s1A += b.oa * b.weight;
        s1B += b.ob * b.weight;
        sw1 += b.weight;
      }
    }

    w0 = sw0;
    w1 = sw1;
    if (sw0 > 0) {
      c0L = s0L / sw0;
      c0A = s0A / sw0;
      c0B = s0B / sw0;
    }
    if (sw1 > 0) {
      c1L = s1L / sw1;
      c1A = s1A / sw1;
      c1B = s1B / sw1;
    }
  }

  // 取总权重占优的主簇，并回贴该簇内的真实像素 Medoid（严禁跨簇求均值产生灰边）
  const winZero = w0 >= w1;
  const winL = winZero ? c0L : c1L;
  const winA = winZero ? c0A : c1A;
  const winB = winZero ? c0B : c1B;

  let bestMedoid = winZero ? seed0 : seed1;
  let bestCost = Infinity;
  for (const b of buckets) {
    const d0 = oklabDistSq(b.L, b.oa, b.ob, c0L, c0A, c0B);
    const d1 = oklabDistSq(b.L, b.oa, b.ob, c1L, c1A, c1B);
    if ((d0 <= d1) !== winZero) continue;
    const dWin = oklabDistSq(b.L, b.oa, b.ob, winL, winA, winB);
    const cost = dWin / Math.pow(b.weight, 0.3);
    if (cost < bestCost) {
      bestCost = cost;
      bestMedoid = b;
    }
  }

  return [bestMedoid.r, bestMedoid.g, bestMedoid.b, outAlpha];
}

/**
 * 可选后处理：lovelaced/pixfix OKLAB 共线夹心去抗锯齿 (Triangle-Inequality Sandwich AA Removal)
 * 仅当某个单格 P 同时满足以下全部几何条件时，才判定为两侧色块交界残留的抗锯齿灰格并吸附至较近邻居：
 * 1. 左右对侧邻居 (A, B)（或上下对侧邻居）均不透明且彼此色差显著 (distAB >= 0.06)；
 * 2. P 在 OKLAB 空间投影严格落在连线 AB 内部 (0.14 <= t <= 0.86)；
 * 3. P 到线段 AB 的垂直偏离极小 (perpDist / distAB <= maxPerpRatio)；
 * 4. P 不是连续线条或色带（四邻中与 P 自身同色的格子数 == 0，确保整条渐变阴影带/高光边 100% 保留！）。
 */
function stripCollinearAntiAliasingCells(
  gridCells: [number, number, number, number][][],
  gw: number,
  gh: number,
  strength: number
): void {
  if (strength <= 0 || gw < 3 || gh < 3) return;
  const s = Math.min(100, Math.max(0, strength)) / 100;
  const maxPerpRatio = 0.035 + s * 0.16;

  // 预计算每个格子的 OKLAB 坐标快照（基于快照只读判定，避免链式传染）
  const labL = new Float32Array(gw * gh);
  const labA = new Float32Array(gw * gh);
  const labB = new Float32Array(gw * gh);
  const snapRgba = gridCells.map((row) =>
    row.map((c): [number, number, number, number] => [c[0], c[1], c[2], c[3]])
  );

  for (let gy = 0; gy < gh; gy++) {
    const row = snapRgba[gy]!;
    for (let gx = 0; gx < gw; gx++) {
      const c = row[gx]!;
      if (c[3] === 0) continue;
      const [L, oa, ob] = rgbToOklab(c[0], c[1], c[2]);
      const idx = gy * gw + gx;
      labL[idx] = L;
      labA[idx] = oa;
      labB[idx] = ob;
    }
  }

  for (let gy = 0; gy < gh; gy++) {
    for (let gx = 0; gx < gw; gx++) {
      const cur = snapRgba[gy]![gx]!;
      if (cur[3] === 0) continue;
      const pIdx = gy * gw + gx;
      const pL = labL[pIdx] ?? 0;
      const pA = labA[pIdx] ?? 0;
      const pB = labB[pIdx] ?? 0;

      // 若四邻中存在与自身同色/极近色的伙伴格，说明这是有意绘制的线条或明暗阶梯带，绝不作为孤立抗锯齿点剥离
      let sameNeighbors = 0;
      const cardinals: [number, number][] = [
        [gx - 1, gy],
        [gx + 1, gy],
        [gx, gy - 1],
        [gx, gy + 1]
      ];
      for (const [nx, ny] of cardinals) {
        if (nx < 0 || nx >= gw || ny < 0 || ny >= gh) continue;
        const nb = snapRgba[ny]![nx]!;
        if (nb[3] === 0) continue;
        const nIdx = ny * gw + nx;
        if (oklabDistSq(pL, pA, pB, labL[nIdx] ?? 0, labA[nIdx] ?? 0, labB[nIdx] ?? 0) < 0.0008) {
          sameNeighbors++;
        }
      }
      if (sameNeighbors > 0) continue;

      const pairs: [[number, number], [number, number]][] = [];
      if (gx > 0 && gx + 1 < gw) pairs.push([[gx - 1, gy], [gx + 1, gy]]);
      if (gy > 0 && gy + 1 < gh) pairs.push([[gx, gy - 1], [gx, gy + 1]]);

      let bestSnap: [number, number, number] | null = null;
      let bestRatio = Infinity;

      for (const [[ax, ay], [bx, by]] of pairs) {
        const cA = snapRgba[ay]![ax]!;
        const cB = snapRgba[by]![bx]!;
        if (cA[3] === 0 || cB[3] === 0) continue;

        const aIdx = ay * gw + ax;
        const bIdx = by * gw + bx;
        const aL = labL[aIdx] ?? 0, aA = labA[aIdx] ?? 0, aB = labB[aIdx] ?? 0;
        const bL = labL[bIdx] ?? 0, bA = labA[bIdx] ?? 0, bB = labB[bIdx] ?? 0;

        const vL = bL - aL;
        const vA = bA - aA;
        const vB = bB - aB;
        const abLenSq = vL * vL + vA * vA + vB * vB;
        if (abLenSq < 0.0036) continue; // distAB >= 0.06

        const wL = pL - aL;
        const wA = pA - aA;
        const wB = pB - aB;
        const t = (wL * vL + wA * vA + wB * vB) / abLenSq;
        if (t < 0.14 || t > 0.86) continue;

        const projL = aL + t * vL;
        const projA = aA + t * vA;
        const projB = aB + t * vB;
        const perpSq = oklabDistSq(pL, pA, pB, projL, projA, projB);
        const ratio = Math.sqrt(perpSq / abLenSq);

        if (ratio <= maxPerpRatio && ratio < bestRatio) {
          bestRatio = ratio;
          bestSnap = t < 0.5 ? [cA[0], cA[1], cA[2]] : [cB[0], cB[1], cB[2]];
        }
      }

      if (bestSnap) {
        gridCells[gy]![gx] = [bestSnap[0], bestSnap[1], bestSnap[2], cur[3]];
      }
    }
  }
}

/**
 * 确定性 OKLAB 显著性增强 + 真实像素 Medoid 调色板量化器（仅在用户主动选择有限调色板时启用）
 */
export function quantizePalette(
  colors: [number, number, number][],
  k: number
): [number, number, number][] {
  if (!colors.length || k < 2) return colors;

  const freqMap = new Map<
    number,
    { r: number; g: number; b: number; L: number; oa: number; ob: number; count: number }
  >();
  for (const c of colors) {
    const key = ((c[0] & 0xff) << 16) | ((c[1] & 0xff) << 8) | (c[2] & 0xff);
    const existing = freqMap.get(key);
    if (existing) {
      existing.count++;
    } else {
      const [L, oa, ob] = rgbToOklab(c[0], c[1], c[2]);
      freqMap.set(key, { r: c[0], g: c[1], b: c[2], L, oa, ob, count: 1 });
    }
  }

  if (freqMap.size <= k) {
    return Array.from(freqMap.values()).map((e) => [e.r, e.g, e.b]);
  }

  const entries = Array.from(freqMap.values()).map((e) => {
    const chroma = Math.sqrt(e.oa * e.oa + e.ob * e.ob);
    return {
      ...e,
      salience: Math.pow(e.count, 0.28) * (1 + chroma * 4.5)
    };
  });

  entries.sort((a, b) => b.count - a.count || a.r - b.r || a.g - b.g || a.b - b.b);

  const targetK = Math.min(k, entries.length);
  const centers: [number, number, number][] = [];
  const chosen = new Uint8Array(entries.length);

  const first = entries[0]!;
  centers.push([first.L, first.oa, first.ob]);
  chosen[0] = 1;

  if (centers.length < targetK) {
    let darkIdx = -1;
    let minL = Infinity;
    for (let i = 0; i < entries.length; i++) {
      if (chosen[i]) continue;
      if (entries[i]!.L < minL) {
        minL = entries[i]!.L;
        darkIdx = i;
      }
    }
    if (darkIdx >= 0) {
      const e = entries[darkIdx]!;
      centers.push([e.L, e.oa, e.ob]);
      chosen[darkIdx] = 1;
    }
  }

  if (centers.length < targetK) {
    let brightIdx = -1;
    let maxL = -Infinity;
    for (let i = 0; i < entries.length; i++) {
      if (chosen[i]) continue;
      if (entries[i]!.L > maxL) {
        maxL = entries[i]!.L;
        brightIdx = i;
      }
    }
    if (brightIdx >= 0) {
      const e = entries[brightIdx]!;
      centers.push([e.L, e.oa, e.ob]);
      chosen[brightIdx] = 1;
    }
  }

  const minD2 = new Float64Array(entries.length);
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i]!;
    let md = Infinity;
    for (const c of centers) {
      const d = oklabDistSq(e.L, e.oa, e.ob, c[0], c[1], c[2]);
      if (d < md) md = d;
    }
    minD2[i] = md;
  }

  while (centers.length < targetK) {
    let bestIdx = -1;
    let bestScore = -1;
    for (let i = 0; i < entries.length; i++) {
      if (chosen[i]) continue;
      const score = (minD2[i] ?? 0) * entries[i]!.salience;
      if (score > bestScore) {
        bestScore = score;
        bestIdx = i;
      }
    }
    if (bestIdx < 0) break;
    chosen[bestIdx] = 1;
    const picked = entries[bestIdx]!;
    const newC: [number, number, number] = [picked.L, picked.oa, picked.ob];
    centers.push(newC);
    for (let i = 0; i < entries.length; i++) {
      if (chosen[i]) continue;
      const e = entries[i]!;
      const d = oklabDistSq(e.L, e.oa, e.ob, newC[0], newC[1], newC[2]);
      if (d < (minD2[i] ?? Infinity)) minD2[i] = d;
    }
  }

  const assign = new Int32Array(entries.length);
  for (let iter = 0; iter < 6; iter++) {
    const sumL = new Float64Array(centers.length);
    const sumA = new Float64Array(centers.length);
    const sumB = new Float64Array(centers.length);
    const sumW = new Float64Array(centers.length);

    for (let i = 0; i < entries.length; i++) {
      const e = entries[i]!;
      let bestC = 0;
      let bestD = Infinity;
      for (let ci = 0; ci < centers.length; ci++) {
        const c = centers[ci]!;
        const d = oklabDistSq(e.L, e.oa, e.ob, c[0], c[1], c[2]);
        if (d < bestD) {
          bestD = d;
          bestC = ci;
        }
      }
      assign[i] = bestC;
      const w = Math.pow(e.count, 0.65);
      sumL[bestC] = (sumL[bestC] ?? 0) + e.L * w;
      sumA[bestC] = (sumA[bestC] ?? 0) + e.oa * w;
      sumB[bestC] = (sumB[bestC] ?? 0) + e.ob * w;
      sumW[bestC] = (sumW[bestC] ?? 0) + w;
    }

    for (let ci = 0; ci < centers.length; ci++) {
      const sw = sumW[ci] ?? 0;
      if (sw > 0) {
        centers[ci] = [(sumL[ci] ?? 0) / sw, (sumA[ci] ?? 0) / sw, (sumB[ci] ?? 0) / sw];
      }
    }
  }

  const palette: [number, number, number][] = Array.from({ length: centers.length }, () => [
    first.r,
    first.g,
    first.b
  ]);
  const bestCost = new Float64Array(centers.length);
  bestCost.fill(Infinity);

  for (let i = 0; i < entries.length; i++) {
    const e = entries[i]!;
    const ci = assign[i] ?? 0;
    const c = centers[ci]!;
    const cost = oklabDistSq(e.L, e.oa, e.ob, c[0], c[1], c[2]) / Math.pow(e.count, 0.15);
    if (cost < (bestCost[ci] ?? Infinity)) {
      bestCost[ci] = cost;
      palette[ci] = [e.r, e.g, e.b];
    }
  }

  return palette;
}

export function cleanPixelArtCanvas(
  sourceCanvas: HTMLCanvasElement,
  opts: PixelCleanOptions = {}
): PixelCleanResult {
  const {
    grid = 'auto',
    manualBlockSize,
    alignMode = 'uniform',
    hintGrid = null,
    palette = 'auto',
    hardAlpha = true,
    aaStripStrength,
    coalesceTolerance = 0,
    detailMode = 'standard'
  } = opts;

  const effectiveAaStrength =
    aaStripStrength !== undefined ? aaStripStrength : coalesceTolerance;

  const w = sourceCanvas.width;
  const h = sourceCanvas.height;
  const ctx = sourceCanvas.getContext('2d', { willReadFrequently: true })!;
  const srcData = ctx.getImageData(0, 0, w, h);
  const src = srcData.data;

  const det = detectPixelGrid(srcData, w, h);

  let blockSize = det.blockSize;
  let targetStepX = det.stepX;
  let targetStepY = det.stepY;
  let phaseX = det.phaseX;
  let phaseY = det.phaseY;
  let exactLocked = false;
  let cutsX: number[] = [];
  let cutsY: number[] = [];

  if (manualBlockSize && manualBlockSize >= 2 && manualBlockSize <= 64) {
    blockSize = manualBlockSize;
    targetStepX = manualBlockSize;
    targetStepY = manualBlockSize;
    const { gradX, gradY } = computeAxisTransitionProfiles(srcData, w, h);
    phaseX = findBestPhaseForStep(gradX, w, manualBlockSize);
    phaseY = findBestPhaseForStep(gradY, h, manualBlockSize);
    cutsX = buildSteppedCuts(w, manualBlockSize, phaseX);
    cutsY = buildSteppedCuts(h, manualBlockSize, phaseY);
    if (det.exact && det.blockSize === manualBlockSize) {
      exactLocked = true;
    }
  } else if (grid !== 'auto') {
    if (grid.startsWith('block')) {
      blockSize = parseInt(grid.slice(5), 10) || 8;
      targetStepX = blockSize;
      targetStepY = blockSize;
      const { gradX, gradY } = computeAxisTransitionProfiles(srcData, w, h);
      phaseX = findBestPhaseForStep(gradX, w, blockSize);
      phaseY = findBestPhaseForStep(gradY, h, blockSize);
      cutsX = buildSteppedCuts(w, blockSize, phaseX);
      cutsY = buildSteppedCuts(h, blockSize, phaseY);
    } else {
      const targetShort = parseInt(grid, 10) || 64;
      const gwTarget =
        w <= h ? Math.max(8, targetShort) : Math.max(8, Math.round((targetShort * w) / Math.max(1, h)));
      const ghTarget =
        w <= h ? Math.max(8, Math.round((targetShort * h) / Math.max(1, w))) : Math.max(8, targetShort);
      targetStepX = w / gwTarget;
      targetStepY = h / ghTarget;
      blockSize = Math.max(2, Math.round((targetStepX + targetStepY) / 2));
      phaseX = 0;
      phaseY = 0;
      cutsX = Array.from({ length: gwTarget + 1 }, (_, k) => Math.round((k * w) / gwTarget));
      cutsY = Array.from({ length: ghTarget + 1 }, (_, k) => Math.round((k * h) / ghTarget));
      if (det.exact && det.cutsX.length - 1 === gwTarget && det.cutsY.length - 1 === ghTarget) {
        exactLocked = true;
      }
    }
  } else if (det.exact) {
    // 残差评分确认原图已是精确网格（含任意裁剪后的像素图）：直接采用零残差切线！
    cutsX = det.cutsX;
    cutsY = det.cutsY;
    exactLocked = true;
  } else if (hintGrid && hintGrid.gw >= 8 && hintGrid.gh >= 8) {
    cutsX = Array.from({ length: hintGrid.gw + 1 }, (_, k) => Math.round((k * w) / hintGrid.gw));
    cutsY = Array.from({ length: hintGrid.gh + 1 }, (_, k) => Math.round((k * h) / hintGrid.gh));
    targetStepX = w / hintGrid.gw;
    targetStepY = h / hintGrid.gh;
    blockSize = Math.max(2, Math.round((targetStepX + targetStepY) / 2));
    phaseX = 0;
    phaseY = 0;
  } else {
    cutsX = det.cutsX;
    cutsY = det.cutsY;
  }

  // 若开启 2× 子像素细分，将每一格对半二等分
  if (detailMode === 'fine2x' && targetStepX >= 4 && targetStepY >= 4) {
    const subX: number[] = [cutsX[0] ?? 0];
    for (let i = 0; i < cutsX.length - 1; i++) {
      const a = cutsX[i] ?? 0;
      const b = cutsX[i + 1] ?? a + 2;
      const mid = Math.round((a + b) / 2);
      if (mid > a && mid < b) subX.push(mid);
      subX.push(b);
    }
    const subY: number[] = [cutsY[0] ?? 0];
    for (let i = 0; i < cutsY.length - 1; i++) {
      const a = cutsY[i] ?? 0;
      const b = cutsY[i + 1] ?? a + 2;
      const mid = Math.round((a + b) / 2);
      if (mid > a && mid < b) subY.push(mid);
      subY.push(b);
    }
    cutsX = subX;
    cutsY = subY;
    targetStepX /= 2;
    targetStepY /= 2;
    blockSize = Math.max(2, Math.round(blockSize / 2));
  }

  let gw = Math.max(4, cutsX.length - 1);
  let gh = Math.max(4, cutsY.length - 1);

  // 仅当用户显式开启 Adaptive 弹性纠偏，且原图并非精确像素网格时，才允许切线弹性浮动
  if (alignMode === 'adaptive' && !exactLocked) {
    cutsX = solveAdaptiveCuts(det.gradX, w, gw);
    cutsY = solveAdaptiveCuts(det.gradY, h, gh);
    gw = cutsX.length - 1;
    gh = cutsY.length - 1;
  }

  // 阶段 2：单格独立 k-Centroid (k=2) 真色提取（跨格零修改！）
  const gridCells: [number, number, number, number][][] = Array.from({ length: gh }, () =>
    Array.from({ length: gw }, (): [number, number, number, number] => [0, 0, 0, 0])
  );

  for (let gy = 0; gy < gh; gy++) {
    const y0 = cutsY[gy] ?? Math.round((gy * h) / gh);
    const y1 = Math.max(y0 + 1, cutsY[gy + 1] ?? Math.round(((gy + 1) * h) / gh));
    const rowCells = gridCells[gy]!;
    for (let gx = 0; gx < gw; gx++) {
      const x0 = cutsX[gx] ?? Math.round((gx * w) / gw);
      const x1 = Math.max(x0 + 1, cutsX[gx + 1] ?? Math.round(((gx + 1) * w) / gw));
      rowCells[gx] = sampleCellKCentroid(src, w, h, x0, x1, y0, y1, hardAlpha);
    }
  }

  // 阶段 3（可选，默认 0 关闭）：OKLAB 共线夹心抗锯齿剥离（lovelaced/pixfix 算法）
  if (effectiveAaStrength > 0) {
    stripCollinearAntiAliasingCells(gridCells, gw, gh, effectiveAaStrength);
  }

  // 阶段 4（可选，默认 auto 不限色关闭）：OKLAB 显著性 Medoid 调色板量化
  let activePalette: [number, number, number][] | null = null;
  if (palette === 'pico8') {
    activePalette = PALETTE_PICO8;
  } else if (palette === 'gameboy') {
    activePalette = PALETTE_GAMEBOY;
  } else if (
    palette === 'auto_cel' ||
    palette === '16' ||
    palette === '24' ||
    palette === '32' ||
    palette === '48' ||
    palette === '64'
  ) {
    const shortDim = Math.min(gw, gh);
    const autoCelK =
      shortDim <= 24 ? 16 : shortDim <= 40 ? 24 : shortDim <= 56 ? 32 : shortDim <= 80 ? 40 : 48;
    const k = palette === 'auto_cel' ? autoCelK : parseInt(palette, 10);
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
    const palLab = activePalette.map((p) => ({
      rgb: p,
      lab: rgbToOklab(p[0], p[1], p[2])
    }));
    const firstPal = palLab[0]!;
    for (let gy = 0; gy < gh; gy++) {
      const rowCells = gridCells[gy]!;
      for (let gx = 0; gx < gw; gx++) {
        const c = rowCells[gx]!;
        if (c[3] === 0) continue;
        const [L, oa, ob] = rgbToOklab(c[0], c[1], c[2]);
        let minDist = Infinity;
        let bestColor = firstPal.rgb;
        for (const p of palLab) {
          const d = oklabDistSq(L, oa, ob, p.lab[0], p.lab[1], p.lab[2]);
          if (d < minDist) {
            minDist = d;
            bestColor = p.rgb;
          }
        }
        rowCells[gx] = [bestColor[0], bestColor[1], bestColor[2], c[3]];
      }
    }
  }

  // 阶段 5：重绘高分辨率画布
  // 若为 Uniform 等宽锁格且精确命中原切线（如含裁剪相位的原图），直接按原切线区间填色以保持零位移；
  // 若为 Adaptive 弹性纠偏，则规整重绘到标准等分网格以拉直歪斜格子。
  const fullCanvas = document.createElement('canvas');
  fullCanvas.width = w;
  fullCanvas.height = h;
  const fctx = fullCanvas.getContext('2d')!;
  const fullImgData = fctx.createImageData(w, h);
  const out = fullImgData.data;

  const useOriginalCutsForRedraw = alignMode === 'uniform' && cutsX.length === gw + 1 && cutsY.length === gh + 1;

  for (let gy = 0; gy < gh; gy++) {
    const y0 = useOriginalCutsForRedraw ? (cutsY[gy] ?? 0) : Math.round((gy * h) / gh);
    const y1 = Math.min(
      h,
      useOriginalCutsForRedraw ? (cutsY[gy + 1] ?? h) : Math.round(((gy + 1) * h) / gh)
    );
    const rowCells = gridCells[gy]!;
    for (let gx = 0; gx < gw; gx++) {
      const x0 = useOriginalCutsForRedraw ? (cutsX[gx] ?? 0) : Math.round((gx * w) / gw);
      const x1 = Math.min(
        w,
        useOriginalCutsForRedraw ? (cutsX[gx + 1] ?? w) : Math.round(((gx + 1) * w) / gw)
      );
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

  // 生成 1:1 原生低分辨率真像素画布
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
    stepX: Number(targetStepX.toFixed(2)),
    stepY: Number(targetStepY.toFixed(2)),
    phaseX,
    phaseY,
    colorCount: uniqueColorMap.size,
    cutsX,
    cutsY,
    exactLocked
  };
}

/**
 * 生图前置虚拟网格规范化 (Pre-generation Virtual Pixel Grid Snap)
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
