/**
 * Magic Wand flood-fill repair:
 * Uses the clicked pixel's color in `orig` as the seed and floods 4-connected neighbors
 * whose original color is within `tol` Euclidean distance.
 * - `restore`: only modifies currently transparent pixels (`proc alpha < 128`), restoring them to `orig` RGBA.
 * - `erase`: only modifies currently opaque pixels (`proc alpha >= 128`), setting their alpha to `0`.
 */
export function applyWandFloodFix(
  orig: ImageData,
  proc: ImageData,
  w: number,
  h: number,
  sx: number,
  sy: number,
  tol: number,
  mode: 'restore' | 'erase'
): number {
  const o = orig.data;
  const p = proc.data;
  const wantTransparent = mode === 'restore';
  const okA = (i: number) => (wantTransparent ? (p[i + 3] ?? 0) < 128 : (p[i + 3] ?? 0) >= 128);
  const si = (sy * w + sx) * 4;
  if (!okA(si)) return 0;

  const sr = o[si] ?? 0;
  const sg = o[si + 1] ?? 0;
  const sb = o[si + 2] ?? 0;
  const T = tol * tol * 3;
  const match = (i: number) => {
    const dr = (o[i] ?? 0) - sr;
    const dg = (o[i + 1] ?? 0) - sg;
    const db = (o[i + 2] ?? 0) - sb;
    return dr * dr + dg * dg + db * db <= T;
  };

  const seen = new Uint8Array(w * h);
  const stack = new Int32Array(w * h);
  let sp = 0;
  let count = 0;
  const start = sy * w + sx;
  seen[start] = 1;
  stack[sp++] = start;

  while (sp > 0) {
    const q = stack[--sp]!;
    const i = q * 4;
    if (!okA(i) || !match(i)) continue;
    if (mode === 'restore') {
      p[i] = o[i] ?? 0;
      p[i + 1] = o[i + 1] ?? 0;
      p[i + 2] = o[i + 2] ?? 0;
      p[i + 3] = 255;
    } else {
      p[i + 3] = 0;
    }
    count++;
    const x = q % w;
    const y = (q / w) | 0;
    if (x > 0 && !seen[q - 1]) {
      seen[q - 1] = 1;
      stack[sp++] = q - 1;
    }
    if (x < w - 1 && !seen[q + 1]) {
      seen[q + 1] = 1;
      stack[sp++] = q + 1;
    }
    if (y > 0 && !seen[q - w]) {
      seen[q - w] = 1;
      stack[sp++] = q - w;
    }
    if (y < h - 1 && !seen[q + w]) {
      seen[q + w] = 1;
      stack[sp++] = q + w;
    }
  }
  return count;
}

/**
 * Pure-color connected-component cutout + 3px edge unmixing (instant CPU fallback or quick solid-bg removal):
 * 1. Estimates dominant border color via 16-level histogram.
 * 2. Removes connected components touching the border (and optionally interior holes >= minArea).
 * 3. Unmixes the 3px transition ring around removed regions to eliminate white/gray halos.
 */
export function removeSolidBackground(
  sourceCanvas: HTMLCanvasElement,
  tol: number,
  removeHoles = true
): HTMLCanvasElement {
  const w = sourceCanvas.width;
  const h = sourceCanvas.height;
  const cv = document.createElement('canvas');
  cv.width = w;
  cv.height = h;
  const ctx = cv.getContext('2d', { willReadFrequently: true })!;
  ctx.drawImage(sourceCanvas, 0, 0);
  const id = ctx.getImageData(0, 0, w, h);
  removeBgCore(id.data, w, h, tol, removeHoles);
  ctx.putImageData(id, 0, 0);
  return cv;
}

function removeBgCore(
  d: Uint8ClampedArray,
  w: number,
  h: number,
  tol: number,
  removeHoles: boolean
): void {
  const n = w * h;
  const buckets = new Map<number, { n: number; r: number; g: number; bl: number }>();

  const feed = (x: number, y: number) => {
    const i = (y * w + x) * 4;
    const r = d[i] ?? 0;
    const g = d[i + 1] ?? 0;
    const bl = d[i + 2] ?? 0;
    const key = ((r >> 4) << 8) | ((g >> 4) << 4) | (bl >> 4);
    let b = buckets.get(key);
    if (!b) {
      b = { n: 0, r: 0, g: 0, bl: 0 };
      buckets.set(key, b);
    }
    b.n++;
    b.r += r;
    b.g += g;
    b.bl += bl;
  };

  for (let x = 0; x < w; x++) {
    feed(x, 0);
    feed(x, h - 1);
  }
  for (let y = 0; y < h; y++) {
    feed(0, y);
    feed(w - 1, y);
  }

  let best = { n: 1, r: 255, g: 255, bl: 255 };
  for (const b of buckets.values()) {
    if (b.n > best.n) best = b;
  }
  const bR = best.r / best.n;
  const bG = best.g / best.n;
  const bB = best.bl / best.n;

  const tol2 = tol * tol * 3;
  const dist2 = (i: number) => {
    const dr = (d[i] ?? 0) - bR;
    const dg = (d[i + 1] ?? 0) - bG;
    const db = (d[i + 2] ?? 0) - bB;
    return dr * dr + dg * dg + db * db;
  };

  const label = new Int32Array(n);
  const stack = new Int32Array(n);
  const compArea: number[] = [];
  const compBorder: boolean[] = [];

  for (let p = 0; p < n; p++) {
    if (label[p] || dist2(p * 4) > tol2) continue;
    const cid = compArea.length + 1;
    let area = 0;
    let border = false;
    let sp = 0;
    label[p] = cid;
    stack[sp++] = p;
    while (sp > 0) {
      const q = stack[--sp]!;
      area++;
      const x = q % w;
      const y = (q / w) | 0;
      if (x === 0 || y === 0 || x === w - 1 || y === h - 1) border = true;
      if (x > 0 && !label[q - 1] && dist2((q - 1) * 4) <= tol2) {
        label[q - 1] = cid;
        stack[sp++] = q - 1;
      }
      if (x < w - 1 && !label[q + 1] && dist2((q + 1) * 4) <= tol2) {
        label[q + 1] = cid;
        stack[sp++] = q + 1;
      }
      if (y > 0 && !label[q - w] && dist2((q - w) * 4) <= tol2) {
        label[q - w] = cid;
        stack[sp++] = q - w;
      }
      if (y < h - 1 && !label[q + w] && dist2((q + w) * 4) <= tol2) {
        label[q + w] = cid;
        stack[sp++] = q + w;
      }
    }
    compArea.push(area);
    compBorder.push(border);
  }

  const minArea = Math.max(12, Math.round(n * 0.00003));
  const removed = new Uint8Array(n);
  for (let p = 0; p < n; p++) {
    const L = label[p] ?? 0;
    if (L > 0 && (compBorder[L - 1] || (removeHoles && (compArea[L - 1] ?? 0) >= minArea))) {
      removed[p] = 1;
      d[p * 4 + 3] = 0;
    }
  }

  const orig = new Uint8ClampedArray(d);
  const odist2 = (i: number) => {
    const dr = (orig[i] ?? 0) - bR;
    const dg = (orig[i + 1] ?? 0) - bG;
    const db = (orig[i + 2] ?? 0) - bB;
    return dr * dr + dg * dg + db * db;
  };

  const unmix = (p: number) => {
    const i = p * 4;
    const x = p % w;
    const y = (p / w) | 0;
    let fi = i;
    let fd = odist2(i);
    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= w || ny >= h) continue;
        const np = ny * w + nx;
        if (removed[np]) continue;
        const D = odist2(np * 4);
        if (D > fd) {
          fd = D;
          fi = np * 4;
        }
      }
    }
    const fr = (orig[fi] ?? 0) - bR;
    const fg = (orig[fi + 1] ?? 0) - bG;
    const fb = (orig[fi + 2] ?? 0) - bB;
    const den = fr * fr + fg * fg + fb * fb;
    let a: number;
    if (den < 1e-6) {
      a = odist2(i) > tol2 ? 1 : 0;
    } else {
      a =
        (((orig[i] ?? 0) - bR) * fr +
          ((orig[i + 1] ?? 0) - bG) * fg +
          ((orig[i + 2] ?? 0) - bB) * fb) /
        den;
    }
    if (a <= 0.06) {
      d[i + 3] = 0;
      return;
    }
    if (a >= 0.94) return;
    d[i] = ((orig[i] ?? 0) - (1 - a) * bR) / a;
    d[i + 1] = ((orig[i + 1] ?? 0) - (1 - a) * bG) / a;
    d[i + 2] = ((orig[i + 2] ?? 0) - (1 - a) * bB) / a;
    d[i + 3] = a * 255;
  };

  const ring = new Uint8Array(n);
  let frontier: number[] = [];
  for (let p = 0; p < n; p++) {
    if (!removed[p]) continue;
    const x = p % w;
    const y = (p / w) | 0;
    if (
      (x > 0 && !removed[p - 1]) ||
      (x < w - 1 && !removed[p + 1]) ||
      (y > 0 && !removed[p - w]) ||
      (y < h - 1 && !removed[p + w])
    ) {
      frontier.push(p);
    }
  }

  for (let dist = 1; dist <= 3 && frontier.length > 0; dist++) {
    const next: number[] = [];
    for (const q of frontier) {
      const x = q % w;
      const y = (q / w) | 0;
      const nbs = [
        x > 0 ? q - 1 : -1,
        x < w - 1 ? q + 1 : -1,
        y > 0 ? q - w : -1,
        y < h - 1 ? q + w : -1
      ];
      for (const nb of nbs) {
        if (nb < 0 || removed[nb] || ring[nb]) continue;
        ring[nb] = dist;
        unmix(nb);
        next.push(nb);
      }
    }
    frontier = next;
  }
}

export function computeAlphaBounds(
  d: Uint8ClampedArray,
  w: number,
  h: number,
  threshold = 8
): [number, number, number, number] | null {
  let x0 = w;
  let y0 = h;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if ((d[(y * w + x) * 4 + 3] ?? 0) > threshold) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  return x1 < 0 ? null : [x0, y0, x1 - x0 + 1, y1 - y0 + 1];
}
