# Sketch2Asset — 游戏资产本地生成 & AI 抠图修补工作站

<p align="center">
  <b>专为游戏开发者、独立制作人与美术设计师打造的开源本地 Web 工具</b><br>
  将草图/提示词转化为高精度游戏 Sprite，提供 AI 抠图、物理去白边、像素修补与无损 PNG 导出全流程。
</p>

<p align="center">
  <b><a href="README.md">English</a></b> | <b><a href="README_zh.md">中文</a></b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Python-3.8+-blue.svg" alt="Python Version">
  <img src="https://img.shields.io/badge/ONNX_Runtime-DirectML%2FCUDA-green.svg" alt="ONNX Runtime">
  <img src="https://img.shields.io/badge/UI-Pro_Studio-violet.svg" alt="Pro Studio Layout">
  <img src="https://img.shields.io/badge/Theme-Dark%2FLight-orange.svg" alt="Dual Theme">
  <img src="https://img.shields.io/badge/i18n-ZH%2FEN-red.svg" alt="Multi-Language">
  <img src="https://img.shields.io/badge/License-MIT-brightgreen.svg" alt="License">
</p>

---

## ✨ 核心亮点

### 🎨 1. 草图引导与资产生成 (Sketch & Generation)
- **草图重绘 (Sketch-to-Asset)**：支持上传线稿/概念草图，结合提示词生成保持原姿态的高精游戏道具与 Icon。
- **多元美术风格预设**：预设像素风 (Pixel Art)、美式卡通 (Cartoon)、二次元 (Anime)、低多边形 (Low Poly)、3D 渲染与写实等风格。
- **纯白背景优化**：默认摄影棚白底规则，极大降低后续 AI 抠图与边缘分割瑕疵。
- **主流 API 兼容**：完美兼容 OpenAI 图像接口 (`/v1/images/edits`) 与对话出图接口 (`/v1/chat/completions`)。

### 🛠 2. Pro Studio AI 抠图工作台 (Matting & Editing)
- **SOTA 深度学习抠图**：内置 BiRefNet、BEN2、ISNet 等模型，支持 GPU (DirectML/CUDA) 硬件加速。
- **专业级去白边 & 引擎保护**：
  - **物理前景色反解 (Unmixing)**：还原白底染色的半透明边缘原色。
  - **边缘收边 (Choke 0~3px)**：边缘透明度内缩，解决白色系资产贴白背景时的物理残留。
  - **4px 色彩外扩 (Bleed)**：自动外扩边缘 4px 颜色，防止 Unity / Unreal 双线性采样边缘发白。
- **精细化修补与导出**：支持点选恢复/擦除、笔刷涂抹、自动裁透明边与自定义分辨率无损 PNG 导出。

### 🌐 3. 先锋数字 UI 与国际化 (Design & i18n)
- **Awwwards 三栏 Inspector 架构**：专业游戏引擎级三栏布局，零 Emoji 侵扰，全量采用 Lucide 矢量图标。
- **深浅双主题 (Dark / Light Mode)**：一键切换高奢深色与日间冰白浅色主题，`localStorage` 自动持久化。
- **中英双语国际化 (i18n ZH/EN)**：支持全界面文本、下拉框及提示信息的无缝实时双语切换。

### 📁 4. 项目归档与一致性锁定 (Project Hub)
- **多项目隔离归档**：按项目自动落盘至 `outputs/项目名/`。
- **风格一致性锁定**：每个项目可独立配置提示词后缀与美术设定 (`project.json`)，保证同项目资产风格统一。
- **配置 JSON 导入/导出**：一键备份/迁移 API 配置与项目风格方案（只新增不覆盖）。

---

## 🚀 快速开始

### 1. 克隆仓库与启动服务
```bash
git clone https://github.com/your-username/sketch2asset.git
cd sketch2asset

# Windows 一键启动
start.bat

# 或通过 Python 启动
python server.py
```
启动后在浏览器访问 `http://127.0.0.1:8000`。

### 2. （可选）安装 AI 抠图 GPU 加速环境
```bash
# 安装基础 AI 抠图环境
python setup_matting.py

# 安装 GPU (DirectML) 加速环境（适用于 N卡/A卡/Intel 显卡）
python setup_matting.py --dml
```

---

## 🤖 抠图模型说明

工作台 Inspector 面板支持自由切换 AI 抠图模型：

| 模型 | 特征说明 | 推荐场景 |
|---|---|---|
| **`birefnet`** (默认) | SOTA 级完整版 (490MB)，显著性目标分割最强模型 | **高精度资产、复杂主体、透明材质（首选）** |
| **`ben2`** | 2025 新一代模型，兼顾推理速度与质量 | 复杂图像快速抠图 |
| **`birefnet-hrsod`** | 高分辨率特化训练版 | 大尺寸概念图 |
| **`birefnet-lite`** | 轻量级模型 (CPU 约 6 秒) | 低配 CPU 环境 |
| **`isnet-general-use`**| 极速推理 (CPU 约 0.6 秒) | 简单白底图快速批量抠图 |
| **`isnet-anime`** | 动漫/二次元角色特化模型 | 动漫人设与角色立绘 |

---

## 📁 目录结构

```text
sketch2asset/
├── start.bat                  # Windows 一键启动脚本
├── server.py                 # 本地轻量服务器 (托管 + 代理 API + 项目管理)
├── index.html                # 前端 App (Awwwards 布局 + Canvas 修补 + i18n + 动态主题)
├── setup_matting.py / .bat   # AI 抠图环境安装与模型自动下载脚本
├── matting_worker.py         # AI 抠图常驻推理进程 (ONNX Runtime)
├── models/                   # AI 抠图 ONNX 模型目录
└── outputs/                  # 生成资产与图库归档落盘目录
```

---

## 📄 开源协议

本项目基于 [MIT License](LICENSE) 开源。自由用于个人与商业游戏项目开发。
