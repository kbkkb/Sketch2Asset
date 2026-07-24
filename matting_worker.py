"""AI 抠图常驻推理进程 (onnxruntime)。由 server.py 作为子进程启动, 不需要手动运行。

用法:
  python matting_worker.py <model.onnx>            # 常驻模式, stdin/stdout 按行收发 JSON
  python matting_worker.py <model.onnx> --selftest # 自检: 合成图跑一遍完整管线
"""
import json
import os
import sys
import tempfile

import numpy as np
import onnxruntime as ort
from PIL import Image

# 尝试导入 cv2 以获得极速的图像膨胀/腐蚀后处理，如果没有则回退到 numpy
try:
    import cv2
    HAS_CV2 = True
except ImportError:
    HAS_CV2 = False

ort.set_default_logger_severity(3)


def model_config(model_path):
    name = os.path.basename(model_path).lower()
    # RMBG-1.4 是 ISNet 架构 (/255 减 0.5), 必须先于通用 rmbg 分支判断
    if "rmbg-1" in name or "rmbg_1" in name:
        return {"size": 1024, "mean": (0.5, 0.5, 0.5), "std": (1.0, 1.0, 1.0),
                "div_max": False, "act": "minmax"}
    # BEN2: ImageNet 归一化, 模型内部已做 sigmoid, 输出直接就是 0..1 (不能再激活)
    if "ben2" in name or "ben_" in name:
        return {"size": 1024, "mean": (0.485, 0.456, 0.406),
                "std": (0.229, 0.224, 0.225), "div_max": False, "act": "none"}
    # BiRefNet / RMBG-2.0: ImageNet 归一化 + 输出 logits 需 sigmoid
    if any(k in name for k in ["birefnet", "rmbg"]):
        return {"size": 1024, "mean": (0.485, 0.456, 0.406),
                "std": (0.229, 0.224, 0.225), "div_max": False, "act": "sigmoid"}
    if "u2net" in name:
        return {"size": 320, "mean": (0.485, 0.456, 0.406),
                "std": (0.229, 0.224, 0.225), "div_max": True, "act": "minmax"}
    # isnet 系 (isnet-general-use / isnetis)
    return {"size": 1024, "mean": (0.5, 0.5, 0.5), "std": (1.0, 1.0, 1.0),
            "div_max": False, "act": "minmax"}


def load_session(model_path, device="auto"):
    """按用户选择的设备加载推理会话。
    device: auto=自动择优 / cpu=强制CPU / gpu=优先显卡(CUDA/DirectML/CoreML, 不可用则回落CPU)。
    注意: DirectML 的提供者 ID 是 "DmlExecutionProvider", 需要 onnxruntime-directml 包
    (setup_matting.py --dml 一键切换)。"""
    available = ort.get_available_providers()
    gpu_pri = [p for p in ("CUDAExecutionProvider", "DmlExecutionProvider",
                           "CoreMLExecutionProvider") if p in available]
    if device == "cpu":
        providers = ["CPUExecutionProvider"]
    elif device == "gpu":
        providers = gpu_pri + ["CPUExecutionProvider"]
    else:
        providers = gpu_pri + ["CPUExecutionProvider"]
    return ort.InferenceSession(model_path, providers=providers)


def run_matte(sess, cfg, img):
    """输入 PIL RGB 图, 返回与原图同尺寸的 float32 matte (0..1)。"""
    ow, oh = img.size
    s = cfg["size"]
    # 使用 LANCZOS 维持边缘平滑度
    x = np.asarray(img.resize((s, s), Image.LANCZOS), dtype=np.float32)
    if cfg["div_max"]:
        x = x / max(float(x.max()), 1e-6)
    else:
        x = x / 255.0
    x = (x - np.asarray(cfg["mean"], np.float32)) / np.asarray(cfg["std"], np.float32)
    x = x.transpose(2, 0, 1)[None]
    inp_name = sess.get_inputs()[0].name
    out = sess.run(None, {inp_name: x})[0]
    m = np.squeeze(out).astype(np.float32)

    if cfg.get("act") == "sigmoid":
        m = 1.0 / (1.0 + np.exp(-np.clip(m, -30.0, 30.0)))
    elif cfg.get("act") == "none":
        m = np.clip(m, 0.0, 1.0)          # 模型内部已激活, 输出即 matte
    else:
        mi, ma = float(m.min()), float(m.max())
        m = (m - mi) / max(ma - mi, 1e-6)

    # 蒙版放回原图尺寸，使用 LANCZOS 消除高清图边缘锯齿
    matte = Image.fromarray((m * 255).astype(np.uint8)).resize((ow, oh), Image.LANCZOS)
    a = np.asarray(matte, np.float32) / 255.0
    return np.clip((a - 0.03) / 0.94, 0.0, 1.0)   # 轻度拉伸, 去掉极淡的残影


def _dilate_fast(mask, iters):
    """膨胀操作：优先使用 OpenCV（快 30 倍），无 cv2 时回退到 NumPy。
    用 MORPH_CROSS(十字核) 保持与 NumPy 4 邻域实现完全一致的语义。"""
    if HAS_CV2:
        kernel = cv2.getStructuringElement(cv2.MORPH_CROSS, (3, 3))
        return cv2.dilate(mask.astype(np.uint8), kernel, iterations=iters).astype(bool)
    else:
        m = mask.copy()
        for _ in range(iters):
            g = m.copy()
            g[1:, :] |= m[:-1, :]
            g[:-1, :] |= m[1:, :]
            g[:, 1:] |= m[:, :-1]
            g[:, :-1] |= m[:, 1:]
            m = g
        return m


def compose_rgba(img, a, enhanced=False, choke=0):
    rgb = np.asarray(img, np.float32)

    # 背景色估算：增强四角采样容错
    border_rgb = np.concatenate([rgb[0], rgb[-1], rgb[:, 0], rgb[:, -1]])
    border_a = np.concatenate([a[0], a[-1], a[:, 0], a[:, -1]])
    sel = border_rgb[border_a < 0.1]
    if len(sel) > 50:
        bgc = np.median(sel, axis=0).astype(np.float32)
    else:
        # 如果边框采样不够，采四角 5x5 区域
        corners = np.concatenate([
            rgb[:5, :5].reshape(-1, 3),
            rgb[:5, -5:].reshape(-1, 3),
            rgb[-5:, :5].reshape(-1, 3),
            rgb[-5:, -5:].reshape(-1, 3)
        ])
        bgc = np.median(corners, axis=0).astype(np.float32)

    d_pc = np.sqrt(((rgb - bgc) ** 2).mean(axis=2))   # 每像素到背景色的距离

    a2 = a.copy()
    if enhanced:
        # 漏检抢救
        chroma = rgb.max(axis=2) - rgb.min(axis=2)
        missed = (a2 < 0.45) & (d_pc > 26.0)
        core = missed & ((chroma > 40.0) | (d_pc > 100.0))
        if core.sum() > max(150.0, a2.size * 0.0008):
            grow = missed & ((chroma > 25.0) | (d_pc > 90.0))
            g = core.copy()
            for _ in range(64):
                nxt = _dilate_fast(g, 1) & grow
                if nxt.sum() == g.sum():
                    break
                g = nxt
            a2 = np.maximum(a2, np.clip((d_pc - 10.0) / 45.0, 0.0, 1.0) * g)
        
        # 收边
        T0, T1 = 6.0, 30.0
        edge = _dilate_fast(a2 < 0.1, 3) & (a2 > 0.02)
        a_color = np.clip((d_pc - T0) / (T1 - T0), 0.0, 1.0)
        a2 = np.where(edge, np.minimum(a2, a_color), a2)
        a2 = np.clip(a2, 0.0, 1.0)

    # 收边 (choke): 灰度腐蚀把整条 alpha 过渡带向内收缩 N 像素。
    # 这是白色物体/白底图去白边的手动手段 —— 颜色证据在白上白时失效, 只能几何收缩
    choke = int(max(0, min(5, choke or 0)))
    if choke:
        if HAS_CV2:
            kernel = cv2.getStructuringElement(cv2.MORPH_CROSS, (3, 3))
            a2 = cv2.erode(a2, kernel, iterations=choke)
        else:
            for _ in range(choke):
                p = a2
                m = p.copy()
                m[1:, :]  = np.minimum(m[1:, :],  p[:-1, :])
                m[:-1, :] = np.minimum(m[:-1, :], p[1:, :])
                m[:, 1:]  = np.minimum(m[:, 1:],  p[:, :-1])
                m[:, :-1] = np.minimum(m[:, :-1], p[:, 1:])
                a2 = m

    a2[a2 <= 0.02] = 0.0

    # 优化后的反解前景色：加入安全分母与平滑过渡，防止 alpha -> 0 时产生极高饱和度噪点
    alpha = a2[..., None]
    safe_alpha = np.maximum(alpha, 0.08) # 防止除以极小值产生爆音噪点
    unmixed = (rgb - (1.0 - alpha) * bgc) / safe_alpha
    
    # alpha < 0.15 时逐步平滑过渡回原 RGB，避免边缘色彩失真
    fade = np.clip(alpha / 0.15, 0.0, 1.0)
    unmixed = unmixed * fade + rgb * (1.0 - fade)
    
    mid = (a2 > 0.02) & (a2 < 0.995)
    out = rgb.copy()
    out[mid] = np.clip(unmixed[mid], 0, 255)

    # 颜色外扩 4px (防游戏引擎纹理采样白边)
    solid = a2 > 0.6
    col = out.copy()
    col[~solid] = 0.0
    wgt = solid.astype(np.float32)

    if HAS_CV2:
        # OpenCV 快速颜色外扩: boxFilter 求 3x3 邻域"加权平均", 与 NumPy 版语义一致。
        # 不能用 cv2.dilate(col): 它是逐通道取最大值, 相邻不同色物体会混出不存在的颜色
        for _ in range(4):
            ws = cv2.boxFilter(wgt, -1, (3, 3), normalize=False)
            cs = cv2.boxFilter(col, -1, (3, 3), normalize=False)
            newly = (wgt == 0) & (ws > 0)
            col[newly] = cs[newly] / ws[newly][..., None]
            wgt[newly] = 1.0
    else:
        # NumPy 兼容回退
        for _ in range(4):
            cs = col.copy()
            ws = wgt.copy()
            cs[:-1, :] += col[1:, :];   ws[:-1, :] += wgt[1:, :]
            cs[1:, :]  += col[:-1, :];  ws[1:, :]  += wgt[:-1, :]
            cs[:, :-1] += col[:, 1:];   ws[:, :-1] += wgt[:, 1:]
            cs[:, 1:]  += col[:, :-1];  ws[:, 1:]  += wgt[:, :-1]
            newly = (wgt == 0) & (ws > 0)
            col[newly] = cs[newly] / ws[newly][..., None]
            wgt[newly] = 1.0

    if enhanced:
        fill = (~solid) & (wgt > 0)
    else:
        fill = (~solid) & (wgt > 0) & (a2 < 0.1)
    out[fill] = col[fill]

    return np.dstack([out, a2 * 255.0]).astype(np.uint8)


def process(sess, cfg, in_path, out_path, enhanced=False, choke=0):
    img = Image.open(in_path).convert("RGB")
    a = run_matte(sess, cfg, img)
    Image.fromarray(compose_rgba(img, a, enhanced, choke), "RGBA").save(out_path)


def selftest(model_path, device="auto"):
    sess = load_session(model_path, device)
    print("selftest provider:", sess.get_providers()[0])
    cfg = model_config(model_path)
    # 领域特化模型 (如动漫角色版) 对通用合成图不适用, 只验证管线与输出格式
    strict = not any(k in os.path.basename(model_path).lower() for k in ("isnetis", "anime"))
    S, w, h, R = 4, 256, 256, 70
    yy, xx = np.mgrid[0:h * S, 0:w * S]
    circle = ((xx - 128 * S) ** 2 + (yy - 128 * S) ** 2) < (R * S) ** 2
    big = np.full((h * S, w * S, 3), 255, np.float32)
    big[circle] = (200, 40, 40)
    arr = big.reshape(h, S, w, S, 3).mean(axis=(1, 3)).astype(np.uint8)

    fd, inp = tempfile.mkstemp(suffix=".png"); os.close(fd)
    fd, outp = tempfile.mkstemp(suffix=".png"); os.close(fd)
    try:
        Image.fromarray(arr).save(inp)
        for label, enhanced in (("标准", False), ("增强", True)):
            process(sess, cfg, inp, outp, enhanced)
            res = np.asarray(Image.open(outp))
            assert res.shape[2] == 4, "输出不是 RGBA"
            a = res[..., 3].astype(np.float32)
            rgbo = res[..., :3].astype(np.float32)
            corner_a, center_a = int(a[2, 2]), int(a[128, 128])
            yy2, xx2 = np.mgrid[0:h, 0:w]
            r = np.sqrt((xx2 - 128) ** 2 + (yy2 - 128) ** 2)
            halo = float(a[(r > R + 3) & (r < R + 8)].mean())
            semi = (a > 30) & (a < 220)
            dist_white = (float(np.sqrt(((rgbo[semi] - 255.0) ** 2).mean(axis=1)).mean())
                          if semi.sum() >= 20 else 999.0)
            print("selftest[%s]: corner=%d center=%d halo_alpha=%.1f semi_px=%d semi_dist_from_white=%.0f"
                  % (label, corner_a, center_a, halo, int(semi.sum()), dist_white))
            if not strict:
                continue          # 特化模型: 跑通且输出 RGBA 即视为通过
            assert corner_a < 30, "角落背景没有变透明"
            assert center_a > 200, "主体中心不应透明"
            assert dist_white > 80, "半透明边缘颜色仍偏白(白边未去除)"
            if enhanced:
                assert halo < 25, "增强模式下主体外圈仍有白晕残留"
        print("SELFTEST-OK")
    finally:
        for p in (inp, outp):
            try:
                os.remove(p)
            except OSError:
                pass


def main():
    if len(sys.argv) < 2:
        print("用法: python matting_worker.py <model.onnx> [--selftest] [--device auto|cpu|gpu]")
        sys.exit(2)
    model_path = sys.argv[1]
    device = "auto"
    if "--device" in sys.argv:
        device = sys.argv[sys.argv.index("--device") + 1]
    if "--selftest" in sys.argv:
        selftest(model_path, device)
        return
    sess = load_session(model_path, device)
    cfg = model_config(model_path)
    print(json.dumps({"ready": True, "provider": sess.get_providers()[0],
                      "available": ort.get_available_providers()}), flush=True)
    for line in sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            req = json.loads(line)
            process(sess, cfg, req["in"], req["out"],
                    bool(req.get("enhanced")), int(req.get("choke") or 0))
            resp = {"ok": True}
        except Exception as e:
            resp = {"error": ("%s: %s" % (type(e).__name__, e))[:300]}
        print(json.dumps(resp, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    main()