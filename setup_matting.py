"""AI 抠图一键安装脚本: 安装依赖 (走国内 pip 镜像) + 下载 ONNX 模型 (多源自动切换) + 自检。

用法:
  python setup_matting.py                       # 默认装 isnet-general-use (通用物体, 推荐)
  python setup_matting.py --model isnet-anime   # 动漫角色特化模型
  python setup_matting.py --model u2net         # 经典 u2net
  python setup_matting.py --list                # 查看可选模型

完成后无需重启 server.py, 网页里直接点「一键透明背景并下载」即走 AI。
"""
import json
import os
import struct
import subprocess
import sys
import time
import urllib.request

ROOT = os.path.dirname(os.path.abspath(__file__))
MODELS_DIR = os.path.join(ROOT, "models")
CONFIG_PATH = os.path.join(ROOT, "matting_config.json")

PIP_MIRRORS = [
    "https://pypi.tuna.tsinghua.edu.cn/simple",
    "https://mirrors.aliyun.com/pypi/simple/",
    None,  # 官方源兜底
]

MODELS = {
    "birefnet": {
        "file": "birefnet-full-fp16.onnx",
        "desc": "BiRefNet 完整版 fp16 (MIT 可商用, 约490MB, 家族最强通用版; 建议配合 GPU)",
        "urls": [
            "https://hf-mirror.com/onnx-community/BiRefNet-ONNX/resolve/main/onnx/model_fp16.onnx",
            "https://huggingface.co/onnx-community/BiRefNet-ONNX/resolve/main/onnx/model_fp16.onnx",
        ],
    },
    "birefnet-hrsod": {
        "file": "birefnet-hrsod-fp16.onnx",
        "desc": "BiRefNet-HRSOD fp16 (高分辨率场景特化训练版, 约490MB; 注: BiRefNet_HR 官方无 ONNX, 此为唯一有官方 ONNX 的高分辨率变体)",
        "urls": [
            "https://hf-mirror.com/onnx-community/BiRefNet-HRSOD_DHU-ONNX/resolve/main/onnx/model_fp16.onnx",
            "https://huggingface.co/onnx-community/BiRefNet-HRSOD_DHU-ONNX/resolve/main/onnx/model_fp16.onnx",
        ],
    },
    "ben2": {
        "file": "ben2-base.onnx",
        "desc": "BEN2 Base (2025 新一代抠图, 约223MB, 效果强; 许可以 HF 页面为准)",
        "urls": [
            "https://hf-mirror.com/PramaLLC/BEN2/resolve/main/BEN2_Base.onnx",
            "https://huggingface.co/PramaLLC/BEN2/resolve/main/BEN2_Base.onnx",
        ],
    },
    "birefnet-lite": {
        "file": "birefnet-lite.onnx",
        "desc": "BiRefNet-Lite 通用抠图 (MIT 可商用, 约224MB, 复杂图效果最好, 推荐)",
        "urls": [
            "https://hf-mirror.com/onnx-community/BiRefNet_lite/resolve/main/onnx/model.onnx",
            "https://huggingface.co/onnx-community/BiRefNet_lite/resolve/main/onnx/model.onnx",
        ],
    },
    "isnet-general-use": {
        "file": "isnet-general-use.onnx",
        "desc": "通用物体抠图 (DIS/ISNet, Apache-2.0, 约180MB, 速度快)",
        "urls": [
            "https://hf-mirror.com/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx",
            "https://huggingface.co/tomjackson2023/rembg/resolve/main/isnet-general-use.onnx",
            "https://github.com/danielgatis/rembg/releases/download/v0.0.0/isnet-general-use.onnx",
        ],
    },
    "isnet-anime": {
        "file": "isnetis.onnx",
        "desc": "动漫角色特化 (skytnt/anime-seg, 约176MB)",
        "urls": [
            "https://hf-mirror.com/skytnt/anime-seg/resolve/main/isnetis.onnx",
            "https://huggingface.co/skytnt/anime-seg/resolve/main/isnetis.onnx",
        ],
    },
}


def ensure_64bit():
    if struct.calcsize("P") * 8 != 64:
        print("错误: 当前 Python 是 32 位, onnxruntime 需要 64 位。")
        print("请用 64 位 Python 重新运行, 例如:  py -3.14 setup_matting.py")
        sys.exit(1)


def have_deps():
    try:
        import onnxruntime, numpy, PIL  # noqa
        return True
    except ImportError:
        return False


def install_deps():
    if have_deps():
        print("[1/3] 依赖已安装, 跳过")
    else:
        pkgs = ["onnxruntime", "numpy", "pillow"]
        ok = False
        for mirror in PIP_MIRRORS:
            cmd = [sys.executable, "-m", "pip", "install", "--disable-pip-version-check"] + pkgs
            if mirror:
                cmd += ["-i", mirror]
            print("[1/3] 安装依赖: pip install %s%s" %
                  (" ".join(pkgs), (" (镜像: %s)" % mirror) if mirror else " (官方源)"))
            if subprocess.call(cmd) == 0 and have_deps():
                ok = True
                break
            print("  该源失败, 换下一个...")
        if not ok:
            print("错误: 依赖安装失败, 请检查网络后重试")
            sys.exit(1)
    # OpenCV 可选 (后处理提速), 装不上不影响功能
    try:
        import cv2  # noqa
    except ImportError:
        print("  [可选] 安装 OpenCV 加速后处理...")
        subprocess.call([sys.executable, "-m", "pip", "install", "--disable-pip-version-check",
                         "-q", "opencv-python-headless", "-i", PIP_MIRRORS[0]])


def install_directml():
    """把 CPU 版 onnxruntime 换成 DirectML 版 (Windows 上用 A/N/I 卡加速推理)。"""
    print("[DML] 切换到 onnxruntime-directml (卸载 CPU 版 -> 安装 DML 版)...")
    subprocess.call([sys.executable, "-m", "pip", "uninstall", "-y", "onnxruntime"])
    for mirror in PIP_MIRRORS:
        cmd = [sys.executable, "-m", "pip", "install", "--disable-pip-version-check",
               "onnxruntime-directml"]
        if mirror:
            cmd += ["-i", mirror]
        if subprocess.call(cmd) == 0:
            print("[DML] 完成。worker 会自动检测并启用 DmlExecutionProvider")
            return
    print("错误: onnxruntime-directml 安装失败")
    sys.exit(1)


def download(urls, dest, token=None):
    if os.path.isfile(dest) and os.path.getsize(dest) > 50 * 1024 * 1024:
        print("[2/3] 模型已存在, 跳过下载: %s (%.0fMB)"
              % (dest, os.path.getsize(dest) / 1e6))
        return
    os.makedirs(MODELS_DIR, exist_ok=True)
    part = dest + ".part"
    for attempt in range(3):                       # 断点续传 + 三轮重试
        for url in urls:
            got = os.path.getsize(part) if os.path.isfile(part) else 0
            print("[2/3] 下载模型%s: %s" % ((" (续传自 %.0fMB)" % (got / 1e6)) if got else "", url))
            try:
                headers = {"User-Agent": "Mozilla/5.0"}
                if token:
                    headers["Authorization"] = "Bearer " + token
                if got:
                    headers["Range"] = "bytes=%d-" % got
                req = urllib.request.Request(url, headers=headers)
                with urllib.request.urlopen(req, timeout=60) as r:
                    mode = "ab"
                    if got and r.status != 206:    # 该源不支持续传 -> 重头下
                        got, mode = 0, "wb"
                    total = int(r.headers.get("Content-Length") or 0) + got
                    with open(part, mode) as f:
                        last_pct = -10
                        while True:
                            chunk = r.read(1024 * 1024)
                            if not chunk:
                                break
                            f.write(chunk)
                            got += len(chunk)
                            if total:
                                pct = got * 100 // total
                                if pct >= last_pct + 5:
                                    last_pct = pct
                                    print("  进度 %3d%%  (%.0f/%.0fMB)"
                                          % (pct, got / 1e6, total / 1e6), flush=True)
                if total and got < total:
                    raise IOError("下载不完整 (%d/%d)" % (got, total))
                os.replace(part, dest)
                print("  下载完成: %s" % dest)
                return
            except Exception as e:
                print("  失败: %s, 换下一个源/稍后续传..." % str(e)[:120])
        time.sleep(2)
    print("错误: 所有下载源都失败。也可手动下载模型放到 %s" % dest)
    sys.exit(1)


def main():
    args = sys.argv[1:]
    if "--list" in args:
        for k, v in MODELS.items():
            print("  %-20s %s" % (k, v["desc"]))
        return
    name = "birefnet-lite"
    if "--model" in args:
        name = args[args.index("--model") + 1]
    token = args[args.index("--hf-token") + 1] if "--hf-token" in args else None
    if "--dml" in args:
        ensure_64bit()
        install_directml()
    if name not in MODELS:
        print("未知模型: %s (用 --list 查看可选项)" % name)
        sys.exit(1)
    info = MODELS[name]
    dest = os.path.join(MODELS_DIR, info["file"])

    print("== Sketch2Asset AI 抠图安装 ==  模型: %s\n   %s" % (name, info["desc"]))
    ensure_64bit()
    install_deps()
    download(info["urls"], dest, token)

    print("[3/3] 自检推理...")
    r = subprocess.run([sys.executable, os.path.join(ROOT, "matting_worker.py"),
                        dest, "--selftest"], capture_output=True, text=True)
    print((r.stdout or "").strip())
    if "SELFTEST-OK" not in (r.stdout or ""):
        print((r.stderr or "").strip()[-800:])
        print("错误: 自检失败")
        sys.exit(1)

    cfg = {"python": sys.executable, "model_path": dest, "model": name}
    try:
        with open(CONFIG_PATH, encoding="utf-8") as f:
            cfg["device"] = json.load(f).get("device", "auto")   # 保留已选设备
    except Exception:
        cfg["device"] = "auto"
    with open(CONFIG_PATH, "w", encoding="utf-8") as f:
        json.dump(cfg, f, ensure_ascii=False, indent=2)
    print("\n安装成功! 配置已写入 matting_config.json")
    print("网页里直接点「一键透明背景并下载」即可使用 AI 抠图 (无需重启服务)。")


if __name__ == "__main__":
    main()
