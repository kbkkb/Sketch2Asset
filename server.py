"""Sketch2Asset - 本地轻量服务器
职责:
  1. 提供静态页面 index.html
  2. 代理前端的生成请求到第三方 API (避免浏览器 CORS 限制, Key 不进前端代码)
  3. 上游若返回图片 URL, 在服务端下载并转成 base64, 保证前端 canvas 可以抠图
仅用 Python 标准库, 无需 pip 安装任何东西。

用法:  python server.py [--port 8000] [--no-browser]
"""
import base64
import json
import mimetypes
import os
import re
import socket
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
import webbrowser
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.abspath(__file__))
UPSTREAM_TIMEOUT = 300  # 图像生成可能很慢, 给足 5 分钟
MAX_BODY = 40 * 1024 * 1024
MATTING_CONFIG_PATH = os.path.join(ROOT, "matting_config.json")

# 模型注册表复用 setup_matting.py 的定义 (同为纯标准库)
sys.path.insert(0, ROOT)
try:
    from setup_matting import MODELS as MODEL_REGISTRY, MODELS_DIR
except Exception:
    MODEL_REGISTRY, MODELS_DIR = {}, os.path.join(ROOT, "models")


class AppError(Exception):
    """带用户可读信息的业务错误, 会以 {"error": ...} 返回给页面。"""


# ---------------------------------------------------------------- 工具函数

def normalize_base(base):
    base = (base or "").strip().rstrip("/")
    if not base:
        raise AppError("请先在「API 设置」里填写 API 地址 (Base URL)")
    if not re.match(r"^https?://", base):
        base = "https://" + base
    # 末尾没有 /v1 这类版本段时自动补 /v1 (OpenAI 兼容接口的通用约定)
    if not re.search(r"/v\d+[a-z]*$", base):
        base += "/v1"
    return base


def parse_data_url(durl):
    m = re.match(r"^data:([^;,]+)?(;base64)?,(.*)$", durl or "", re.S)
    if not m or not m.group(2):
        raise AppError("草图数据格式不正确 (需要 base64 data URL)")
    mime = m.group(1) or "image/png"
    try:
        return mime, base64.b64decode(m.group(3))
    except Exception:
        raise AppError("草图 base64 解码失败")


def sniff_image_mime(b):
    if b[:8] == b"\x89PNG\r\n\x1a\n":
        return "image/png"
    if b[:3] == b"\xff\xd8\xff":
        return "image/jpeg"
    if b[:4] == b"RIFF" and b[8:12] == b"WEBP":
        return "image/webp"
    if b[:6] in (b"GIF87a", b"GIF89a"):
        return "image/gif"
    return None


def http_request(url, headers, body, method="POST"):
    req = urllib.request.Request(url, data=body, method=method)
    for k, v in headers.items():
        req.add_header(k, v)
    try:
        with urllib.request.urlopen(req, timeout=UPSTREAM_TIMEOUT) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def fetch_as_data_url(url):
    """下载图片 URL 并转为 data URL, 让前端 canvas 不被跨域污染。"""
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0 Sketch2Asset"})
    with urllib.request.urlopen(req, timeout=120) as r:
        data = r.read()
        ctype = (r.headers.get("Content-Type") or "").split(";")[0].strip()
    mime = ctype if ctype.startswith("image/") else sniff_image_mime(data)
    if not mime:
        raise AppError("上游返回的链接不是图片: " + url[:200])
    return "data:%s;base64,%s" % (mime, base64.b64encode(data).decode())


def to_data_url(u):
    if not isinstance(u, str) or not u:
        return None
    if u.startswith("data:"):
        return u
    if re.match(r"^https?://", u):
        return fetch_as_data_url(u)
    return "data:image/png;base64," + u  # 裸 base64


def upstream_error(status, raw):
    text = raw.decode("utf-8", "replace")
    msg = text
    try:
        j = json.loads(text)
        err = j.get("error")
        if isinstance(err, dict):
            msg = err.get("message") or text
        elif isinstance(err, str):
            msg = err
        elif j.get("message"):
            msg = j["message"]
    except Exception:
        pass
    raise AppError("上游接口返回 HTTP %d：%s" % (status, msg[:600]))


def build_multipart(fields, files):
    boundary = "----Sketch2Asset" + uuid.uuid4().hex
    parts = []
    for k, v in fields.items():
        parts.append(
            ("--%s\r\nContent-Disposition: form-data; name=\"%s\"\r\n\r\n%s\r\n"
             % (boundary, k, v)).encode("utf-8"))
    for name, fname, mime, data in files:
        parts.append(
            ("--%s\r\nContent-Disposition: form-data; name=\"%s\"; filename=\"%s\"\r\n"
             "Content-Type: %s\r\n\r\n" % (boundary, name, fname, mime)).encode("utf-8"))
        parts.append(data)
        parts.append(b"\r\n")
    parts.append(("--%s--\r\n" % boundary).encode("utf-8"))
    return boundary, b"".join(parts)


# ---------------------------------------------------------------- 生成逻辑

def images_generate(base, auth, model, prompt, image, size):
    """OpenAI 兼容图像接口: 有草图走 /images/edits, 无草图走 /images/generations。"""
    if image:
        mime, data = parse_data_url(image)
        ext = {"image/png": "png", "image/jpeg": "jpg", "image/webp": "webp"}.get(mime, "png")
        fields = {"model": model, "prompt": prompt}
        if size and size != "auto":
            fields["size"] = size
        boundary, body = build_multipart(fields, [("image", "sketch." + ext, mime, data)])
        headers = dict(auth)
        headers["Content-Type"] = "multipart/form-data; boundary=" + boundary
        via = "images/edits"
        status, raw = http_request(base + "/images/edits", headers, body)
    else:
        payload = {"model": model, "prompt": prompt}
        if size and size != "auto":
            payload["size"] = size
        headers = dict(auth)
        headers["Content-Type"] = "application/json"
        via = "images/generations"
        status, raw = http_request(base + "/images/generations", headers,
                                   json.dumps(payload).encode("utf-8"))
    if status != 200:
        upstream_error(status, raw)
    try:
        data_j = json.loads(raw)
    except Exception:
        raise AppError("上游返回的不是 JSON：" + raw[:300].decode("utf-8", "replace"))
    items = data_j.get("data") or []
    item = items[0] if items else {}
    if item.get("b64_json"):
        return "data:image/png;base64," + item["b64_json"], via
    if item.get("url"):
        return fetch_as_data_url(item["url"]), via
    raise AppError("上游返回里没有图片数据：" + json.dumps(data_j, ensure_ascii=False)[:400])


def chat_text(data):
    try:
        content = data["choices"][0]["message"].get("content")
    except Exception:
        return ""
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(p.get("text", "") for p in content
                         if isinstance(p, dict) and p.get("type") == "text")
    return ""


def extract_image_from_chat(data):
    try:
        msg = data["choices"][0]["message"]
    except (KeyError, IndexError, TypeError):
        return None
    # 常见中转格式: message.images = [{image_url:{url: "data:..."}}]
    for im in msg.get("images") or []:
        u = None
        if isinstance(im, dict):
            iu = im.get("image_url")
            if isinstance(iu, dict):
                u = iu.get("url")
            u = u or im.get("url") or im.get("b64_json")
        elif isinstance(im, str):
            u = im
        got = to_data_url(u)
        if got:
            return got
    content = msg.get("content")
    texts = []
    if isinstance(content, list):
        for p in content:
            if not isinstance(p, dict):
                continue
            if p.get("type") in ("image_url", "image", "output_image"):
                iu = p.get("image_url")
                u = iu.get("url") if isinstance(iu, dict) else p.get("url")
                got = to_data_url(u)
                if got:
                    return got
            elif p.get("type") == "text":
                texts.append(p.get("text") or "")
        content = "\n".join(texts)
    if isinstance(content, str) and content:
        m = re.search(r"data:image/[a-zA-Z0-9.+-]+;base64,[A-Za-z0-9+/=]+", content)
        if m:
            return m.group(0)
        # markdown 里的图片链接: 逐个尝试下载, 成功即用
        for m in re.finditer(r"https?://[^\s)\]\"'<>]+", content):
            try:
                return fetch_as_data_url(m.group(0))
            except Exception:
                continue
    return None


def chat_generate(base, auth, model, prompt, image):
    """OpenAI 兼容对话接口出图 (如 gemini-2.5-flash-image / nano-banana 类模型)。"""
    parts = [{"type": "text", "text": prompt}]
    if image:
        parts.append({"type": "image_url", "image_url": {"url": image}})
    body = {"model": model,
            "messages": [{"role": "user", "content": parts}],
            "modalities": ["image", "text"]}  # OpenRouter 等需要; 不支持的会在下面重试
    headers = dict(auth)
    headers["Content-Type"] = "application/json"
    url = base + "/chat/completions"
    status, raw = http_request(url, headers, json.dumps(body).encode("utf-8"))
    if status == 400:
        body.pop("modalities", None)
        status, raw = http_request(url, headers, json.dumps(body).encode("utf-8"))
    if status != 200:
        upstream_error(status, raw)
    try:
        data = json.loads(raw)
    except Exception:
        raise AppError("上游返回的不是 JSON：" + raw[:300].decode("utf-8", "replace"))
    img = extract_image_from_chat(data)
    if not img:
        reply = chat_text(data)
        raise AppError("接口没有返回图片，模型可能不支持图像输出。模型回复：" +
                       (reply[:300] or json.dumps(data, ensure_ascii=False)[:300]))
    return img, "chat/completions"


def do_generate(p):
    base = normalize_base(p.get("apiBase"))
    key = (p.get("apiKey") or "").strip()
    if not key:
        raise AppError("请先在「API 设置」里填写 API Key")
    mode = p.get("mode") or "images"
    model = (p.get("model") or "").strip()
    if not model:
        model = "gpt-image-1" if mode == "images" else "gemini-2.5-flash-image"
    prompt = (p.get("prompt") or "").strip() or "Turn this sketch into a polished game asset."
    auth = {"Authorization": "Bearer " + key}
    if mode == "chat":
        return chat_generate(base, auth, model, prompt, p.get("image"))
    return images_generate(base, auth, model, prompt, p.get("image"), (p.get("size") or "").strip())


def list_models(p):
    """调上游 GET /models, 返回该 Key 实际可用的模型 ID 列表。"""
    base = normalize_base(p.get("apiBase"))
    key = (p.get("apiKey") or "").strip()
    if not key:
        raise AppError("请先在「API 设置」里填写 API Key")
    status, raw = http_request(base + "/models",
                               {"Authorization": "Bearer " + key}, None, method="GET")
    if status != 200:
        upstream_error(status, raw)
    try:
        j = json.loads(raw)
    except Exception:
        raise AppError("上游 /models 返回的不是 JSON：" + raw[:300].decode("utf-8", "replace"))
    items = None
    if isinstance(j, dict):
        items = j.get("data") if isinstance(j.get("data"), list) else j.get("models")
    elif isinstance(j, list):
        items = j
    ids = []
    for it in items or []:
        if isinstance(it, str):
            ids.append(it)
        elif isinstance(it, dict):
            mid = it.get("id") or it.get("model") or it.get("name")
            if mid:
                ids.append(str(mid))
    ids = sorted(set(ids), key=str.lower)
    if not ids:
        raise AppError("上游 /models 没有返回模型。原始返回：" +
                       json.dumps(j, ensure_ascii=False)[:300])
    return ids


def safe_project(name):
    """校验并返回项目名; 空返回 None。项目 = outputs/ 下的子目录。"""
    name = (name or "").strip()
    if not name:
        return None
    if name.startswith("_") or len(name) > 40 or not re.fullmatch(r"[\w一-鿿\- ]+", name):
        raise AppError("项目名只能包含中英文、数字、空格、-、_，不以 _ 开头，且不超过 40 字")
    return name


def list_projects():
    base = os.path.join(ROOT, "outputs")
    items = []
    if os.path.isdir(base):
        for fn in sorted(os.listdir(base)):
            d = os.path.join(base, fn)
            if not os.path.isdir(d):
                continue
            meta = {}
            try:
                with open(os.path.join(d, "project.json"), encoding="utf-8") as f:
                    meta = json.load(f)
            except Exception:
                pass
            n_img = len([x for x in os.listdir(d) if x.lower().endswith(".png")])
            items.append({"name": fn, "style": meta.get("style", "none"),
                          "stylePrompt": meta.get("stylePrompt", ""), "count": n_img})
    return {"projects": items}


def save_history(data_url, prefix="gen", project=None):
    """把图片落盘到 outputs/[项目/], 返回可访问的相对 URL (失败返回 None, 不影响主流程)。"""
    try:
        _, data = parse_data_url(data_url)
        d = os.path.join(ROOT, "outputs")
        if project:
            d = os.path.join(d, project)
        os.makedirs(d, exist_ok=True)
        name = "%s_%s_%s.png" % (prefix, time.strftime("%Y%m%d_%H%M%S"), uuid.uuid4().hex[:6])
        with open(os.path.join(d, name), "wb") as f:
            f.write(data)
        return "/outputs/" + ((project + "/") if project else "") + name
    except Exception:
        return None


# ---------------------------------------------------------------- AI 抠图

class Matting:
    """管理常驻的 matting_worker.py 子进程 (按需启动, 崩溃自动重启)。"""

    def __init__(self):
        self.lock = threading.Lock()
        self.proc = None
        self.cfg = None
        self.cfg_mtime = None

    def load_cfg(self):
        try:
            mt = os.path.getmtime(MATTING_CONFIG_PATH)
        except OSError:
            self.cfg = None
            return None
        if self.cfg is None or mt != self.cfg_mtime:
            self._stop()
            try:
                with open(MATTING_CONFIG_PATH, encoding="utf-8") as f:
                    cfg = json.load(f)
                if os.path.isfile(cfg.get("python", "")) and os.path.isfile(cfg.get("model_path", "")):
                    self.cfg, self.cfg_mtime = cfg, mt
                else:
                    self.cfg = None
            except Exception:
                self.cfg = None
        return self.cfg

    def status(self):
        cfg = self.load_cfg()
        if not cfg:
            return {"available": False}
        return {"available": True, "model": cfg.get("model"),
                "device": cfg.get("device") or "auto",
                "provider": getattr(self, "provider", None)}

    def set_device(self, device):
        if device not in ("auto", "cpu", "gpu"):
            raise AppError("device 必须是 auto / cpu / gpu")
        if not os.path.isfile(MATTING_CONFIG_PATH):
            raise AppError("AI 抠图未安装")
        with open(MATTING_CONFIG_PATH, encoding="utf-8") as f:
            cfg = json.load(f)
        cfg["device"] = device
        with open(MATTING_CONFIG_PATH, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
        return self.status()   # 配置 mtime 变化 -> 下次调用自动用新设备重启 worker

    def _stop(self):
        if self.proc:
            try:
                self.proc.kill()
            except Exception:
                pass
            self.proc = None

    def _readline(self, timeout):
        box = []

        def rd():
            try:
                box.append(self.proc.stdout.readline())
            except Exception:
                box.append("")
        t = threading.Thread(target=rd, daemon=True)
        t.start()
        t.join(timeout)
        return box[0].strip() if box else None

    def _ensure(self):
        worker = os.path.join(ROOT, "matting_worker.py")
        try:
            wmt = os.path.getmtime(worker)
        except OSError:
            wmt = 0
        if self.proc and self.proc.poll() is None:
            if wmt == getattr(self, "worker_mtime", None):
                return
            self._stop()          # worker 代码更新了 -> 重启换新逻辑
        self.worker_mtime = wmt
        log = open(os.path.join(ROOT, "matting_worker.log"), "ab")
        self.proc = subprocess.Popen(
            [self.cfg["python"], worker, self.cfg["model_path"],
             "--device", (self.cfg.get("device") or "auto")],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log,
            cwd=ROOT, text=True, encoding="utf-8")
        line = self._readline(180)          # 等模型加载完的 ready 信号
        if not line or "ready" not in line:
            self._stop()
            raise AppError("AI 抠图进程启动失败, 详见 matting_worker.log")
        try:
            self.provider = json.loads(line).get("provider")
        except Exception:
            self.provider = None

    def matte(self, image_data_url, enhanced=False, choke=0):
        with self.lock:
            cfg = self.load_cfg()
            if not cfg:
                raise AppError("AI 抠图未安装: 请先运行  python setup_matting.py")
            self._ensure()
            _, data = parse_data_url(image_data_url)
            fd, inp = tempfile.mkstemp(suffix=".png"); os.close(fd)
            fd, outp = tempfile.mkstemp(suffix=".png"); os.close(fd)
            try:
                with open(inp, "wb") as f:
                    f.write(data)
                self.proc.stdin.write(json.dumps(
                    {"in": inp, "out": outp, "enhanced": bool(enhanced),
                     "choke": int(choke or 0)}) + "\n")
                self.proc.stdin.flush()
                line = self._readline(180)
                if not line:
                    self._stop()
                    raise AppError("AI 抠图进程无响应/崩溃, 详见 matting_worker.log")
                resp = json.loads(line)
                if resp.get("error"):
                    raise AppError("AI 抠图失败: " + resp["error"])
                with open(outp, "rb") as f:
                    return "data:image/png;base64," + base64.b64encode(f.read()).decode()
            finally:
                for p in (inp, outp):
                    try:
                        os.remove(p)
                    except OSError:
                        pass


MATTING = Matting()


class ModelDownloads:
    """后台跑 setup_matting.py 下载模型, 供网页轮询进度。同一时间只允许一个下载。"""

    def __init__(self):
        self.lock = threading.Lock()
        self.name = None
        self.progress = ""
        self.error = None
        self.proc = None

    def start(self, name, python_exe):
        with self.lock:
            if self.proc and self.proc.poll() is None:
                raise AppError("已有模型在下载中: %s, 请等它完成" % self.name)
            self.name, self.progress, self.error = name, "启动下载...", None
            self.proc = subprocess.Popen(
                [python_exe, os.path.join(ROOT, "setup_matting.py"), "--model", name],
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                cwd=ROOT, text=True, encoding="utf-8", errors="replace")
            threading.Thread(target=self._pump, daemon=True).start()

    def _pump(self):
        p = self.proc
        for line in p.stdout:
            line = line.strip()
            if line:
                self.progress = line[-120:]
        rc = p.wait()
        with self.lock:
            if rc != 0:
                self.error = "下载/自检失败: " + self.progress
            self.name = None

    def state(self):
        active = self.proc is not None and self.proc.poll() is None
        return {"downloading": self.name if active else None,
                "progress": self.progress if active else None,
                "error": self.error}


DOWNLOADS = ModelDownloads()


def models_state():
    cfg = MATTING.load_cfg() or {}
    active = cfg.get("model")
    items = []
    for name, info in MODEL_REGISTRY.items():
        fp = os.path.join(MODELS_DIR, info["file"])
        items.append({
            "name": name,
            "desc": info["desc"],
            "downloaded": os.path.isfile(fp) and os.path.getsize(fp) > 50 * 1024 * 1024,
            "active": name == active,
        })
    st = DOWNLOADS.state()
    st.update({"models": items, "active": active})
    return st


def switch_model(name):
    if name not in MODEL_REGISTRY:
        raise AppError("未知模型: %s" % name)
    info = MODEL_REGISTRY[name]
    fp = os.path.join(MODELS_DIR, info["file"])
    if os.path.isfile(fp) and os.path.getsize(fp) > 50 * 1024 * 1024:
        # 已下载: 直接改配置 (保留 python/device), worker 下次调用自动重启
        if not os.path.isfile(MATTING_CONFIG_PATH):
            raise AppError("请先运行一次 python setup_matting.py 完成初始安装")
        with open(MATTING_CONFIG_PATH, encoding="utf-8") as f:
            cfg = json.load(f)
        cfg["model"], cfg["model_path"] = name, fp
        with open(MATTING_CONFIG_PATH, "w", encoding="utf-8") as f:
            json.dump(cfg, f, ensure_ascii=False, indent=2)
        DOWNLOADS.error = None      # 成功的切换清掉上一次下载的陈旧错误提示
    else:
        # 未下载: 后台跑 setup 下载, 完成后 setup 会写配置自动切换
        py_exe = sys.executable
        try:
            with open(MATTING_CONFIG_PATH, encoding="utf-8") as f:
                py_exe = json.load(f).get("python") or py_exe
        except Exception:
            pass
        DOWNLOADS.start(name, py_exe)
    return models_state()


# ---------------------------------------------------------------- HTTP 服务

class Handler(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, fmt, *args):
        sys.stderr.write("[%s] %s\n" % (time.strftime("%H:%M:%S"), fmt % args))

    def _send_json(self, obj, status=200):
        data = json.dumps(obj, ensure_ascii=False).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_GET(self):
        path = urllib.parse.unquote(self.path.split("?", 1)[0])
        if path == "/api/matting/status":
            self._send_json(MATTING.status())
            return
        if path == "/api/matting/models":
            self._send_json(models_state())
            return
        if path == "/api/projects":
            self._send_json(list_projects())
            return
        if path == "/api/history":
            qs = urllib.parse.parse_qs(self.path.split("?", 1)[1]) if "?" in self.path else {}
            proj = urllib.parse.unquote((qs.get("project") or [""])[0])
            base = os.path.join(ROOT, "outputs")
            items = []

            def add_dir(d, rel):
                if not os.path.isdir(d):
                    return
                for fn in os.listdir(d):
                    if fn.lower().endswith(".png"):
                        try:
                            items.append((os.path.getmtime(os.path.join(d, fn)),
                                          "/outputs/" + rel + fn))
                        except OSError:
                            pass
            try:
                if proj == "_all":
                    add_dir(base, "")
                    if os.path.isdir(base):
                        for fn in os.listdir(base):
                            if os.path.isdir(os.path.join(base, fn)):
                                add_dir(os.path.join(base, fn), fn + "/")
                elif proj:
                    p = safe_project(proj)
                    add_dir(os.path.join(base, p), p + "/")
                else:
                    add_dir(base, "")
            except AppError as e:
                self._send_json({"error": str(e)})
                return
            items.sort(reverse=True)
            self._send_json({"items": [u for _, u in items[:60]]})
            return
        if path == "/":
            path = "/index.html"
        fp = os.path.normpath(os.path.join(ROOT, path.lstrip("/")))
        if not fp.startswith(ROOT) or not os.path.isfile(fp):
            self._send_json({"error": "not found"}, 404)
            return
        mime = mimetypes.guess_type(fp)[0] or "application/octet-stream"
        if mime.startswith("text/"):
            mime += "; charset=utf-8"
        with open(fp, "rb") as f:
            data = f.read()
        self.send_response(200)
        self.send_header("Content-Type", mime)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        path = self.path.split("?", 1)[0]
        if path not in ("/api/generate", "/api/models", "/api/matting", "/api/save",
                        "/api/matting/device", "/api/matting/model",
                        "/api/projects", "/api/projects/assign", "/api/projects/rename"):
            self._send_json({"error": "not found"}, 404)
            return
        try:
            length = int(self.headers.get("Content-Length") or 0)
        except ValueError:
            length = 0
        if length <= 0 or length > MAX_BODY:
            self._send_json({"error": "请求体为空或超过 40MB 限制"})
            return
        raw = self.rfile.read(length)
        try:
            payload = json.loads(raw)
        except Exception:
            self._send_json({"error": "请求体不是合法 JSON"})
            return
        try:
            if path == "/api/models":
                self._send_json({"models": list_models(payload)})
            elif path == "/api/matting":
                if not payload.get("image"):
                    raise AppError("缺少 image 参数")
                self._send_json({"image": MATTING.matte(payload["image"],
                                                        bool(payload.get("enhanced")),
                                                        int(payload.get("choke") or 0))})
            elif path == "/api/matting/device":
                self._send_json(MATTING.set_device((payload.get("device") or "").strip()))
            elif path == "/api/matting/model":
                self._send_json(switch_model((payload.get("model") or "").strip()))
            elif path == "/api/save":
                if not payload.get("image"):
                    raise AppError("缺少 image 参数")
                saved = save_history(payload["image"], "edit", safe_project(payload.get("project")))
                if not saved:
                    raise AppError("保存失败, 请检查磁盘权限")
                self._send_json({"saved": saved})
            elif path == "/api/projects":
                name = safe_project(payload.get("name"))
                if not name:
                    raise AppError("项目名不能为空")
                d = os.path.join(ROOT, "outputs", name)
                os.makedirs(d, exist_ok=True)
                meta_path = os.path.join(d, "project.json")
                meta = {}
                try:
                    with open(meta_path, encoding="utf-8") as f:
                        meta = json.load(f)
                except Exception:
                    pass
                for k in ("style", "stylePrompt"):
                    if k in payload:
                        meta[k] = payload.get(k) or ""
                meta["name"] = name
                with open(meta_path, "w", encoding="utf-8") as f:
                    json.dump(meta, f, ensure_ascii=False, indent=2)
                self._send_json(list_projects())
            elif path == "/api/projects/rename":
                old = safe_project(payload.get("old"))
                new = safe_project(payload.get("new"))
                if not old or not new:
                    raise AppError("缺少项目名")
                src = os.path.join(ROOT, "outputs", old)
                dst = os.path.join(ROOT, "outputs", new)
                if not os.path.isdir(src):
                    raise AppError("项目不存在: " + old)
                if os.path.exists(dst):
                    raise AppError("已存在同名项目: " + new)
                os.rename(src, dst)
                mp = os.path.join(dst, "project.json")
                meta = {}
                try:
                    with open(mp, encoding="utf-8") as f:
                        meta = json.load(f)
                except Exception:
                    pass
                meta["name"] = new
                with open(mp, "w", encoding="utf-8") as f:
                    json.dump(meta, f, ensure_ascii=False, indent=2)
                self._send_json(list_projects())
            elif path == "/api/projects/assign":
                url = payload.get("url") or ""
                proj = safe_project(payload.get("project"))
                if not url.startswith("/outputs/"):
                    raise AppError("url 不合法")
                outbase = os.path.abspath(os.path.join(ROOT, "outputs"))
                src = os.path.abspath(os.path.join(
                    outbase, urllib.parse.unquote(url[len("/outputs/"):])))
                if not src.startswith(outbase) or not os.path.isfile(src):
                    raise AppError("找不到该图片")
                dd = os.path.join(outbase, proj) if proj else outbase
                os.makedirs(dd, exist_ok=True)
                dst = os.path.join(dd, os.path.basename(src))
                if dst != src:
                    os.replace(src, dst)
                self._send_json({"url": "/outputs/" + ((proj + "/") if proj else "")
                                        + os.path.basename(src)})
            else:
                project = safe_project(payload.get("project"))   # 先校验, 避免白花生成费
                image, via = do_generate(payload)
                self._send_json({"image": image, "via": via,
                                 "saved": save_history(image, "gen", project)})
        except AppError as e:
            self._send_json({"error": str(e)})
        except TimeoutError:
            self._send_json({"error": "请求上游接口超时（图像生成较慢，可稍后重试）"})
        except urllib.error.URLError as e:
            reason = getattr(e, "reason", e)
            if "timed out" in str(reason).lower():
                self._send_json({"error": "请求上游接口超时（图像生成较慢，可稍后重试）"})
            else:
                self._send_json({"error": "无法连接上游接口：%s" % reason})
        except Exception as e:
            self._send_json({"error": "服务器内部错误：%r" % e})


def pick_port(start):
    for p in range(start, start + 20):
        with socket.socket() as s:
            try:
                s.bind(("127.0.0.1", p))
                return p
            except OSError:
                continue
    raise SystemExit("在 %d-%d 范围内没有可用端口" % (start, start + 19))


def main():
    args = sys.argv[1:]
    port = 8000
    if "--port" in args:
        port = int(args[args.index("--port") + 1])
    port = pick_port(port)
    httpd = ThreadingHTTPServer(("127.0.0.1", port), Handler)
    url = "http://127.0.0.1:%d" % port
    print("=" * 50)
    print("  Sketch2Asset  草图 -> 游戏资产 本地工具")
    print("  地址: %s   (按 Ctrl+C 退出)" % url)
    print("=" * 50)
    if "--no-browser" not in args:
        threading.Timer(0.6, lambda: webbrowser.open(url)).start()
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        print("\n已退出")


if __name__ == "__main__":
    main()
