# Sketch2Asset — Local Game Asset Generator & AI Matting Studio

<p align="center">
  <b>An open-source local Web tool designed for game developers, indie creators, and digital artists</b><br>
  Transform sketches/prompts into high-quality game sprites with SOTA AI matting, edge choke, pixel retouching, and lossless PNG export.
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

## ✨ Key Features

### 🎨 1. Sketch-Guided Asset Generation
- **Sketch-to-Asset**: Upload line art or concept sketches to generate high-resolution game items/icons while maintaining pose and structure.
- **Multiple Art Styles**: Built-in presets for Pixel Art, Cartoon, Anime, Low Poly, 3D Render, Realistic, and Watercolor styles.
- **Studio White Background**: Enforces pure white background generation to eliminate edge artifacts in post-processing.
- **OpenAI Compatible**: Seamlessly supports OpenAI Images API (`/v1/images/edits`) and Chat Completions API (`/v1/chat/completions`).

### 🛠 2. Pro Studio AI Matting & Editing
- **SOTA Deep Learning Matting**: Integrated with BiRefNet, BEN2, and ISNet models with GPU hardware acceleration (DirectML/CUDA).
- **Edge Protection & Anti-Fringing**:
  - **Foreground Unmixing**: Recalculates semi-transparent boundary pixels to remove white background bleeding.
  - **Edge Choke (0~3px)**: Shrinks alpha boundaries inward to eliminate white fringes on light-colored assets.
  - **4px Color Bleed**: Expands edge pixels outward by 4px to prevent bilinear filtering bleed in game engines like Unity/Unreal.
- **Pixel-Level Retouching**: Interactive color-picker restore/erase, custom brush tools, auto-trimming, and resolution-customized PNG export.

### 🌐 3. Avant-Garde UI & Full i18n
- **Awwwards-Level 3-Column Inspector**: Professional engine-inspired layout powered by Lucide vector icons (Zero Emojis).
- **Dual Theme System**: Toggle between sleek Dark Mode and clean Light Mode with instant `localStorage` persistence.
- **100% i18n Multi-Language**: Smooth real-time switching between English and Chinese across all text nodes, options, and dynamic statuses.

### 📁 4. Project Archiving & Style Locking
- **Project Isolation**: Automatically archives assets into dedicated `outputs/project_name/` folders.
- **Style Consistency Locking**: Lock prompt suffixes and style presets per project (`project.json`) for cohesive asset batches.
- **Config JSON Export/Import**: Easily backup and transfer API profiles and project style settings (append-only mode).

---

## 🚀 Quick Start

### 1. Clone & Run
```bash
git clone https://github.com/your-username/sketch2asset.git
cd sketch2asset

# Windows One-Click Start
start.bat

# Or run via Python
python server.py
```
Open `http://127.0.0.1:8000` in your browser.

### 2. (Optional) Setup AI Matting & GPU Acceleration
```bash
# Setup basic AI Matting environment
python setup_matting.py

# Setup GPU (DirectML) Acceleration (Supports NVIDIA, AMD & Intel GPUs)
python setup_matting.py --dml
```

---

## 🤖 AI Matting Models

Easily switch AI matting models in the Studio Inspector sidebar:

| Model | Highlights | Recommended Use Cases |
|---|---|---|
| **`birefnet`** (Default) | SOTA Full fp16 (490MB), top-tier salient object segmentation | **Complex subjects, transparent materials, high-precision assets (Recommended)** |
| **`ben2`** | 2025 next-gen model balancing speed & quality | Fast matting for complex images |
| **`birefnet-hrsod`** | High-resolution scene fine-tuned model | Large concept artwork |
| **`birefnet-lite`** | Lightweight model (~6s on CPU) | Low-spec CPU environments |
| **`isnet-general-use`**| Ultra-fast inference (~0.6s on CPU) | Batch matting for simple white backgrounds |
| **`isnet-anime`** | Anime character specialized model | 2D Anime characters & character art |

---

## 📁 Directory Structure

```text
sketch2asset/
├── start.bat                  # Windows one-click start & auto-install script
├── requirements.txt           # Python dependency requirements list
├── server.py                 # Local server (hosting + API proxy + project manager)
├── index.html                # Single-page App (Awwwards layout + Canvas retouch + i18n + Dual Theme)
├── setup_matting.py / .bat   # Environment setup & ONNX model downloader
├── matting_worker.py         # AI Matting inference worker (ONNX Runtime)
├── models/                   # AI Matting ONNX models folder
└── outputs/                  # Asset outputs & gallery directory
```

---

## 📄 License

This project is licensed under the [MIT License](LICENSE). Free for personal and commercial game development.
