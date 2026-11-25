pub const S2A_WORKER_PY: &str = r####"import argparse
import json
import os
import subprocess
import sys
import time
import shutil

os.environ.setdefault("HF_HUB_DISABLE_SYMLINKS_WARNING", "1")
os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "8")
os.environ.setdefault("MKL_NUM_THREADS", "8")
os.environ.setdefault("OMP_NUM_THREADS", "8")


def emit(stage: str, percent: int, message: str, extra: dict | None = None):
    payload = {"stage": stage, "percent": int(percent), "message": message}
    if extra:
        payload.update(extra)
    print(f"S2A_EVENT {json.dumps(payload, ensure_ascii=False)}", flush=True)


class _StepProgressBar:
    def __init__(self, total: int, stage: str, p_start: int, p_end: int, label: str, cool_ms: int):
        self.total = max(1, int(total) if total else 1)
        self.stage = stage
        self.p_start = int(p_start)
        self.p_end = int(p_end)
        self.label = label
        self.cool_ms = max(0, int(cool_ms))
        self.cur = 0
        self.t0 = time.time()

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        return False

    def set_description(self, *_args, **_kwargs):
        pass

    def update(self, n: int = 1):
        self.cur += int(n)
        clamped = min(self.cur, self.total)
        ratio = float(clamped) / float(self.total)
        pct = self.p_start + int(round((self.p_end - self.p_start) * ratio))
        elapsed = time.time() - self.t0
        emit(self.stage, pct, f"{self.label} ({clamped}/{self.total} 步 · {elapsed:.1f}s)")
        if self.cool_ms > 0:
            import torch
            torch.cuda.synchronize()
            time.sleep(self.cool_ms / 1000.0)


def attach_step_progress(pipe, stage: str, p_start: int, p_end: int, label: str, cool_ms: int):
    def _factory(total=None, **_kwargs):
        t = total if total else 1
        return _StepProgressBar(t, stage, p_start, p_end, label, cool_ms)
    pipe.progress_bar = _factory


def _crop_head(img, xywh):
    x, y, w, h = xywh
    ih, iw = img.shape[:2]
    x1 = x
    y1 = y
    x2 = x + w
    y2 = y + h
    if w < iw // 2:
        px = min(iw - x - w, x, w // 3)
        x1 = min(max(x - px, 0), iw)
        x2 = min(max(x + w + px, 0), iw)
    if h < ih // 2:
        py = min(ih - y - h, y, h // 3)
        y2 = min(max(y + h + py, 0), ih)
        y1 = min(max(y - py, 0), ih)
    return img[y1: y2, x1: x2], (x1, y1, x2, y2)


def recover_face_from_head(saved: str) -> bool:
    import cv2
    import numpy as np
    from PIL import Image

    head_path = os.path.join(saved, "head.png")
    if not os.path.isfile(head_path):
        return False

    head_rgba = np.array(Image.open(head_path).convert("RGBA"))
    head_alpha = head_rgba[..., 3]
    head_mask = head_alpha > 15
    if int(head_mask.sum()) < 300:
        return False

    h, w = head_rgba.shape[:2]
    feat_mask = np.zeros((h, w), dtype=np.uint8)
    for tag in ["eyewhite", "irides", "eyelash", "eyebrow", "nose", "mouth"]:
        p = os.path.join(saved, f"{tag}.png")
        if os.path.isfile(p):
            arr = np.array(Image.open(p).convert("RGBA"))
            if arr.shape[:2] == (h, w):
                feat_mask[arr[..., 3] > 20] = 255

    # Remove drawn facial features from head skin layer cleanly when face pass dropped out
    kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
    feat_dilated = cv2.dilate(feat_mask, kernel, iterations=2)
    inpaint_mask = ((feat_dilated > 0) & head_mask).astype(np.uint8) * 255

    rgb = head_rgba[..., :3].copy()
    if int((inpaint_mask > 0).sum()) > 10:
        # cv2.inpaint uses BGR ordering seamlessly on 3-channel arrays
        rgb = cv2.inpaint(rgb, inpaint_mask, 5, cv2.INPAINT_TELEA)

    out_rgba = np.dstack([rgb, head_alpha])
    Image.fromarray(out_rgba).save(os.path.join(saved, "face.png"))
    return True


def run_layerdiff_safe(pipeline, imgp, save_dir, seed, num_inference_steps, resolution, cool_ms):
    import cv2
    import numpy as np
    import torch
    from PIL import Image
    from utils.cv import center_square_pad_resize, smart_resize

    saved = os.path.join(save_dir, os.path.splitext(os.path.basename(imgp))[0])
    os.makedirs(saved, exist_ok=True)
    input_img = np.array(Image.open(imgp).convert("RGBA"))
    fullpage, pad_size, pad_pos = center_square_pad_resize(input_img, resolution, return_pad_info=True)
    scale = pad_size[0] / resolution
    Image.fromarray(fullpage).save(os.path.join(saved, "src_img.png"))

    exec_dev = getattr(pipeline, "_execution_device", None)
    if exec_dev is None or (isinstance(exec_dev, torch.device) and exec_dev.type == "cpu"):
        exec_dev = torch.device("cuda" if torch.cuda.is_available() else "cpu")

    # Pass 1: Full-body 13 semantic layers
    rng = torch.Generator(device=exec_dev).manual_seed(seed)
    body_tag_list = [
        "front hair", "back hair", "head", "neck", "neckwear",
        "topwear", "handwear", "bottomwear", "legwear", "footwear",
        "tail", "wings", "objects",
    ]
    attach_step_progress(pipeline, "layerdiff", 25, 48, "身体与服装语义拆解", cool_ms)
    body_out = pipeline(
        strength=1.0,
        num_inference_steps=num_inference_steps,
        batch_size=1,
        generator=rng,
        guidance_scale=1.0,
        prompt=body_tag_list,
        negative_prompt="",
        fullpage=fullpage,
        group_index=0,
        show_progress=True,
    )
    for rst, tag in zip(body_out.images, body_tag_list):
        Image.fromarray(rst).save(os.path.join(saved, f"{tag}.png"))
    head_img = body_out.images[2]

    # Locate head crop cleanly (gracefully skip head pass for non-humanoid/headless subjects)
    head_tag_list = [
        "headwear", "face", "irides", "eyebrow", "eyewhite",
        "eyelash", "eyewear", "ears", "earwear", "nose", "mouth",
    ]
    head_mask = (head_img[..., -1] > 15).astype(np.uint8)
    head_px = int(head_mask.sum())
    nz_head = cv2.findNonZero(head_mask) if head_px > 0 else None
    if head_px == 0 or nz_head is None:
        emit("layerdiff", 70, "未检出独立头部五官区域，已保留全身主体与部件分层并跳过五官细化")
        return False

    hx0, hy0, hw, hh = cv2.boundingRect(nz_head)
    hx = int(hx0 * scale) - pad_pos[0]
    hy = int(hy0 * scale) - pad_pos[1]
    hw = int(hw * scale)
    hh = int(hh * scale)
    src_h, src_w = input_img.shape[:2]
    hx2 = min(hx + hw, src_w)
    hy2 = min(hy + hh, src_h)
    hx = max(hx, 0)
    hy = max(hy, 0)
    if hx2 <= hx or hy2 <= hy:
        emit("layerdiff", 70, "头部包围盒有效面积为空，已保留全身分层并跳过五官细化")
        return False

    input_head, (hx1, hy1, hx2, hy2) = _crop_head(input_img, [hx, hy, hx2 - hx, hy2 - hy])
    if input_head.size == 0 or input_head.shape[0] == 0 or input_head.shape[1] == 0:
        emit("layerdiff", 70, "裁剪得到的头部区域为空，已保留全身分层并跳过五官细化")
        return False

    hx1 = max(0, int(hx1 / scale + pad_pos[0] / scale))
    hy1 = max(0, int(hy1 / scale + pad_pos[1] / scale))
    ih, iw = input_head.shape[:2]
    input_head, h_pad_size, h_pad_pos = center_square_pad_resize(input_head, resolution, return_pad_info=True)
    Image.fromarray(input_head).save(os.path.join(saved, "src_head.png"))

    canvas = np.zeros((resolution, resolution, 4), dtype=np.uint8)
    py1, py2, px1, px2 = (
        np.array([h_pad_pos[1], h_pad_pos[1] + ih, h_pad_pos[0], h_pad_pos[0] + iw]) / scale
    ).astype(np.int64)
    scale_size = (int(h_pad_size[0] / scale), int(h_pad_size[1] / scale))

    def _run_head_once(head_seed: int, p_start: int, p_end: int, label: str):
        hrng = torch.Generator(device=exec_dev).manual_seed(head_seed)
        attach_step_progress(pipeline, "layerdiff", p_start, p_end, label, cool_ms)
        hout = pipeline(
            strength=1.0,
            num_inference_steps=num_inference_steps,
            batch_size=1,
            generator=hrng,
            guidance_scale=1.0,
            prompt=head_tag_list,
            negative_prompt="",
            fullpage=input_head,
            group_index=1,
            show_progress=True,
        )
        placed = {}
        for rst, tag in zip(hout.images, head_tag_list):
            cropped_rst = smart_resize(rst, scale_size)[py1: py2, px1: px2]
            full = canvas.copy()
            y_end = min(resolution, hy1 + cropped_rst.shape[0])
            x_end = min(resolution, hx1 + cropped_rst.shape[1])
            rh = max(0, y_end - hy1)
            rw = max(0, x_end - hx1)
            if rh > 0 and rw > 0:
                full[hy1:y_end, hx1:x_end] = cropped_rst[:rh, :rw]
            placed[tag] = full
        return placed

    head_parts = _run_head_once(seed, 48, 66, "头部五官精细拆解")
    face_px = int((head_parts["face"][..., 3] > 15).sum())
    min_face_px = max(300, int(head_px * 0.25))
    face_recovered = False

    if head_px > 500 and face_px < min_face_px:
        emit("layerdiff", 66, "检测到脸部皮肤层漏检，正在自动换种子重试头部拆解...")
        retry_parts = _run_head_once(seed + 137, 66, 71, "脸部皮肤补全重试")
        retry_face_px = int((retry_parts["face"][..., 3] > 15).sum())
        if retry_face_px > face_px:
            head_parts["face"] = retry_parts["face"]
            face_px = retry_face_px
            # Also adopt other features only if previously missing
            for t in head_tag_list:
                if t != "face" and int((head_parts[t][..., 3] > 15).sum()) == 0:
                    head_parts[t] = retry_parts[t]

    for tag, arr in head_parts.items():
        Image.fromarray(arr).save(os.path.join(saved, f"{tag}.png"))

    if head_px > 500 and face_px < min_face_px:
        if recover_face_from_head(saved):
            face_recovered = True
            emit("layerdiff", 71, "已从头部皮肤壳体自动重构完整脸皮层，防止面部发黑")

    if refine_face_skin_fidelity(saved, face_recovered):
        face_recovered = True
        emit("layerdiff", 71, "已完成面部肤色校准与遮挡区纹理重构，消除灰面伪影")

    return face_recovered


def refine_face_skin_fidelity(saved: str, face_recovered: bool) -> bool:
    import cv2
    import numpy as np
    from PIL import Image

    src_p = os.path.join(saved, "src_img.png")
    face_p = os.path.join(saved, "face.png")
    if not (os.path.isfile(src_p) and os.path.isfile(face_p)):
        return False

    src_rgba = np.array(Image.open(src_p).convert("RGBA"))
    face_rgba = np.array(Image.open(face_p).convert("RGBA"))
    if src_rgba.shape[:2] != face_rgba.shape[:2]:
        return False

    h, w = face_rgba.shape[:2]
    face_alpha = face_rgba[..., 3].copy()
    face_mask = face_alpha > 15
    if int(face_mask.sum()) < 300:
        return False

    # Strip leaked temple/side hair blobs outside head mask (before skin tone stats)
    hair_occ = np.zeros((h, w), dtype=bool)
    for htag in ["front hair", "back hair"]:
        hp = os.path.join(saved, f"{htag}.png")
        if os.path.isfile(hp):
            harr = np.array(Image.open(hp).convert("RGBA"))
            if harr.shape[:2] == (h, w):
                hair_occ |= (harr[..., 3] > 20)

    trimmed_hair = False
    head_p = os.path.join(saved, "head.png")
    if os.path.isfile(head_p):
        head_rgba = np.array(Image.open(head_p).convert("RGBA"))
        if head_rgba.shape[:2] == (h, w):
            head_m = head_rgba[..., 3] > 15
            if int(head_m.sum()) >= 300:
                k_hsz = max(5, (h // 160) | 1)
                k_head = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (k_hsz, k_hsz))
                head_allow = cv2.dilate(head_m.astype(np.uint8), k_head, iterations=1) > 0
                skin_ref_m = face_mask & head_m & (~hair_occ) & (src_rgba[..., 3] > 200)
                if int(skin_ref_m.sum()) < 50:
                    skin_ref_m = face_mask & head_m
                if int(skin_ref_m.sum()) >= 20:
                    rough_skin_med = np.median(face_rgba[skin_ref_m, :3].astype(np.float32), axis=0)
                    outside_head = face_mask & (~head_allow)
                    color_far = (
                        np.mean(np.abs(face_rgba[..., :3].astype(np.float32) - rough_skin_med), axis=-1)
                        > 30.0
                    )
                    leaked = outside_head & (hair_occ | color_far)
                    if int(leaked.sum()) > 20:
                        face_alpha[leaked] = 0
                        num_c, lbl_c, st_c, _ = cv2.connectedComponentsWithStats(
                            (face_alpha > 15).astype(np.uint8), connectivity=8
                        )
                        if num_c > 2:
                            max_area = int(st_c[1:, cv2.CC_STAT_AREA].max())
                            keep_m = np.zeros_like(face_alpha, dtype=bool)
                            for ci in range(1, num_c):
                                if int(st_c[ci, cv2.CC_STAT_AREA]) >= max(120, int(max_area * 0.10)):
                                    keep_m |= (lbl_c == ci)
                            if int(keep_m.sum()) >= 300:
                                face_alpha[~keep_m] = 0
                        face_mask = face_alpha > 15
                        trimmed_hair = True

    if int(face_mask.sum()) < 300:
        return False

    # Collect foreground layers that occlude face in original portrait
    occ_feat = np.zeros((h, w), dtype=np.uint8)
    for tag in ["eyewhite", "irides", "eyelash", "eyebrow", "eyewear", "nose", "mouth"]:
        tp = os.path.join(saved, f"{tag}.png")
        if os.path.isfile(tp):
            arr = np.array(Image.open(tp).convert("RGBA"))
            if arr.shape[:2] == (h, w):
                occ_feat[arr[..., 3] > 18] = 255
    occ_hair = np.zeros((h, w), dtype=np.uint8)
    for tag in ["front hair", "headwear"]:
        tp = os.path.join(saved, f"{tag}.png")
        if os.path.isfile(tp):
            arr = np.array(Image.open(tp).convert("RGBA"))
            if arr.shape[:2] == (h, w):
                occ_hair[arr[..., 3] > 18] = 255

    k5 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
    occ_layers = np.maximum(
        cv2.dilate(occ_feat, k5, iterations=1),
        cv2.dilate(occ_hair, k5, iterations=2),
    )

    vis_skin_mask = face_mask & (occ_layers == 0) & (src_rgba[..., 3] > 200)
    if int(vis_skin_mask.sum()) < 100:
        return False

    k3 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
    face_inner = cv2.erode(face_mask.astype(np.uint8), k3, iterations=2) > 0

    src_rgb = src_rgba[..., :3].astype(np.float32)
    face_rgb = face_rgba[..., :3].astype(np.float32)
    src_gray = cv2.cvtColor(src_rgba[..., :3], cv2.COLOR_RGB2GRAY)

    # Exclude dark outlines near chin/jaw when computing clean skin tone stats
    vis_skin_clean = vis_skin_mask & face_inner & (src_gray > 110)
    if int(vis_skin_clean.sum()) < 50:
        vis_skin_clean = vis_skin_mask

    drift = float(np.mean(np.abs(face_rgb[vis_skin_clean] - src_rgb[vis_skin_clean])))
    skin_median = np.median(src_rgb[vis_skin_clean], axis=0)
    occ_face = face_mask & (~vis_skin_mask)
    occ_grey_diff = 0.0
    if int(occ_face.sum()) > 30:
        occ_grey_diff = float(np.mean(np.abs(face_rgb[occ_face].mean(axis=0) - skin_median)))

    # 2) Fix occluded / dirty regions purely from crisp src_img skin without blending grey face_rgb back
    if face_recovered or drift > 12.0 or occ_grey_diff > 22.0:
        seed_rgb = np.empty((h, w, 3), dtype=np.uint8)
        seed_rgb[:, :] = np.clip(skin_median, 0, 255).astype(np.uint8)
        seed_rgb[vis_skin_mask] = src_rgba[vis_skin_mask, :3]

        occ_inpaint_mask = (occ_face & face_inner).astype(np.uint8) * 255
        if int((occ_inpaint_mask > 0).sum()) > 10:
            inpainted = cv2.inpaint(seed_rgb, occ_inpaint_mask, 7, cv2.INPAINT_TELEA)
            inpainted_soft = cv2.GaussianBlur(inpainted, (5, 5), 0)
            result_rgb = np.where((occ_inpaint_mask > 0)[..., None], inpainted_soft, seed_rgb)
        else:
            result_rgb = seed_rgb
        out = np.dstack([result_rgb, face_alpha])
        Image.fromarray(out).save(face_p)
        return True

    vis_weight = cv2.GaussianBlur(vis_skin_mask.astype(np.float32), (5, 5), 0)[..., None] * face_mask[..., None].astype(np.float32)
    blended_rgb = face_rgb * (1.0 - vis_weight) + src_rgb * vis_weight

    if int(occ_face.sum()) > 10 and drift > 2.0:
        delta = src_rgb[vis_skin_clean].mean(axis=0) - face_rgb[vis_skin_clean].mean(axis=0)
        cal_occ = np.clip(face_rgb + delta[None, None, :] * 0.75, 0, 255)
        result_rgb = np.where(occ_face[..., None], cal_occ, blended_rgb)
        out = np.dstack([np.clip(result_rgb, 0, 255).astype(np.uint8), face_alpha])
        Image.fromarray(out).save(face_p)
        return True

    if trimmed_hair:
        out = np.dstack([face_rgba[..., :3], face_alpha])
        Image.fromarray(out).save(face_p)
        return True

    return False


def enforce_v3_face_zorder(
    manifest_layers: list[dict],
    psd_path: str,
    rebuild_py: str,
    saved_dir: str = "",
) -> bool:
    import numpy as np
    from PIL import Image

    ordered = sorted(manifest_layers, key=lambda x: int(x["zIndex"]))
    changed = False

    # Step 1: Ensure face is above back hair, neck, topwear and ears without pulling headwear above face
    face_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "face"), -1)
    if face_idx >= 0:
        behind_face = {"back hair", "neck", "topwear", "ears", "ears-l", "ears-r"}
        max_behind_idx = max(
            (i for i, l in enumerate(ordered) if l.get("name", "").lower() in behind_face),
            default=-1,
        )
        if face_idx < max_behind_idx:
            face_item = ordered.pop(face_idx)
            ordered.insert(max_behind_idx, face_item)
            face_idx = max_behind_idx
            changed = True

        # Step 2: Sort facial foreground tiers strictly in anatomical Live2D order above face
        tier_map = {
            "mouth": 0,
            "nose": 0,
            "eyewhite": 1,
            "eyewhite-l": 1,
            "eyewhite-r": 1,
            "irides": 2,
            "irides-l": 2,
            "irides-r": 2,
            "eyelash": 3,
            "eyelash-l": 3,
            "eyelash-r": 3,
            "eyebrow": 4,
            "eyebrow-l": 4,
            "eyebrow-r": 4,
            "eyewear": 5,
            "front hair": 6,
        }
        face_fg = [(i, l) for i, l in enumerate(ordered) if l.get("name", "").lower() in tier_map]
        if face_fg:
            prev_anchor = face_idx
            prev_tier = -1
            needs_tier_fix = False
            for idx_pos, item in face_fg:
                cur_tier = tier_map[item.get("name", "").lower()]
                if idx_pos <= face_idx or cur_tier < prev_tier or idx_pos < prev_anchor:
                    needs_tier_fix = True
                    break
                prev_anchor = idx_pos
                prev_tier = cur_tier

            if needs_tier_fix:
                fg_ids = {id(item) for _, item in face_fg}
                first_fg_slot = next(
                    (i for i, l in enumerate(ordered) if id(l) in fg_ids and i > face_idx),
                    face_idx + 1,
                )
                insert_anchor_item = ordered[first_fg_slot - 1] if first_fg_slot > 0 else ordered[face_idx]
                remaining = [l for l in ordered if id(l) not in fg_ids]
                sorted_fg = [
                    l for _, l in sorted(
                        face_fg,
                        key=lambda pair: (tier_map[pair[1].get("name", "").lower()], pair[0]),
                    )
                ]
                anchor_idx = next(
                    (i for i, l in enumerate(remaining) if id(l) == id(insert_anchor_item)),
                    next((i for i, l in enumerate(remaining) if l.get("name", "").lower() == "face"), len(remaining) - 1),
                )
                face_pos = next((i for i, l in enumerate(remaining) if l.get("name", "").lower() == "face"), 0)
                insert_pos = max(anchor_idx + 1, face_pos + 1)
                for offset, item in enumerate(sorted_fg):
                    remaining.insert(insert_pos + offset, item)
                ordered = remaining
                changed = True

    src_p = os.path.join(saved_dir, "src_img.png") if saved_dir else ""
    src_rgba = None
    if src_p and os.path.isfile(src_p):
        src_rgba = np.array(Image.open(src_p).convert("RGBA"))

    # Step 3: Validate footwear vs legwear depth order against src_img overlap error
    fw_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "footwear"), -1)
    lw_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "legwear"), -1)
    if src_rgba is not None and fw_idx >= 0 and lw_idx >= 0 and fw_idx < lw_idx:
        try:
            fw_arr = np.array(Image.open(ordered[fw_idx]["imagePath"]).convert("RGBA"))
            lw_arr = np.array(Image.open(ordered[lw_idx]["imagePath"]).convert("RGBA"))
            if fw_arr.shape[:2] == src_rgba.shape[:2] and lw_arr.shape[:2] == src_rgba.shape[:2]:
                overlap = (fw_arr[..., 3] > 120) & (lw_arr[..., 3] > 120) & (src_rgba[..., 3] > 120)
                if int(overlap.sum()) > 80:
                    src_rgb = src_rgba[overlap, :3].astype(np.float32)
                    err_fw = float(np.mean(np.abs(fw_arr[overlap, :3].astype(np.float32) - src_rgb)))
                    err_lw = float(np.mean(np.abs(lw_arr[overlap, :3].astype(np.float32) - src_rgb)))
                    if err_fw + 3.0 < err_lw:
                        fw_item = ordered.pop(fw_idx)
                        new_lw_idx = next(i for i, l in enumerate(ordered) if l.get("name", "").lower() == "legwear")
                        ordered.insert(new_lw_idx + 1, fw_item)
                        changed = True
        except Exception:
            pass

    # Step 4: Auto-hide hallucinated headwear OR lift valid foreground headwear above hair when occluded
    hw_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "headwear"), -1)
    hw_item = ordered[hw_idx] if hw_idx >= 0 else None
    if src_rgba is not None and hw_item is not None:
        try:
            hw_arr = np.array(Image.open(hw_item["imagePath"]).convert("RGBA"))
            if hw_arr.shape[:2] == src_rgba.shape[:2]:
                hw_m = hw_arr[..., 3] > 100
                hw_cnt = int(hw_m.sum())
                if hw_cnt > 120:
                    hw_err = float(
                        np.mean(
                            np.abs(
                                hw_arr[hw_m, :3].astype(np.float32)
                                - src_rgba[hw_m, :3].astype(np.float32)
                            )
                        )
                    )
                    base_union = np.zeros(src_rgba.shape[:2], dtype=bool)
                    for ref_item in ordered:
                        nm = ref_item.get("name", "").lower()
                        if nm.startswith(("front hair", "back hair", "face")) and os.path.isfile(ref_item["imagePath"]):
                            ref_arr = np.array(Image.open(ref_item["imagePath"]).convert("RGBA"))
                            if ref_arr.shape[:2] == src_rgba.shape[:2]:
                                base_union |= (ref_arr[..., 3] > 80)
                    ov_ratio = float((hw_m & base_union).sum()) / float(hw_cnt)
                    if hw_err > 38.0 and ov_ratio > 0.70:
                        if hw_item.get("visible", True):
                            hw_item["visible"] = False
                            changed = True
                    elif hw_item.get("visible", True):
                        max_hair_idx = max(
                            (
                                i
                                for i, l in enumerate(ordered)
                                if l.get("name", "").lower().startswith(("front hair", "back hair"))
                                and l.get("visible", True)
                            ),
                            default=-1,
                        )
                        if max_hair_idx >= 0 and hw_idx < max_hair_idx:
                            hair_comp = Image.new("RGBA", (src_rgba.shape[1], src_rgba.shape[0]), (0, 0, 0, 0))
                            for hi in range(hw_idx + 1, len(ordered)):
                                hl = ordered[hi]
                                hnm = hl.get("name", "").lower()
                                if hnm.startswith(("front hair", "back hair", "face")) and hl.get("visible", True) and os.path.isfile(hl["imagePath"]):
                                    himg = Image.open(hl["imagePath"]).convert("RGBA")
                                    if himg.size == (src_rgba.shape[1], src_rgba.shape[0]):
                                        hair_comp.alpha_composite(himg)
                            hair_comp_arr = np.array(hair_comp)
                            hair_ov = hw_m & (hair_comp_arr[..., 3] > 110) & (src_rgba[..., 3] > 120)
                            if int(hair_ov.sum()) >= 40:
                                src_ov = src_rgba[hair_ov, :3].astype(np.float32)
                                err_hw_ov = float(np.mean(np.abs(hw_arr[hair_ov, :3].astype(np.float32) - src_ov)))
                                err_hair_ov = float(np.mean(np.abs(hair_comp_arr[hair_ov, :3].astype(np.float32) - src_ov)))
                                if err_hw_ov + 4.0 < err_hair_ov:
                                    lifted_hw = ordered.pop(hw_idx)
                                    new_max_hair = max(
                                        (
                                            i
                                            for i, l in enumerate(ordered)
                                            if l.get("name", "").lower().startswith(("front hair", "back hair"))
                                            and l.get("visible", True)
                                        ),
                                        default=len(ordered) - 1,
                                    )
                                    ordered.insert(new_max_hair + 1, lifted_hw)
                                    changed = True
        except Exception:
            pass

    if not changed:
        return False

    manifest_layers[:] = ordered
    for idx, item in enumerate(manifest_layers):
        item["zIndex"] = idx
    return True


def _bbox_from_alpha(alpha):
    import numpy as np
    ys, xs = np.where(alpha > 10)
    if len(xs) == 0:
        return [0, 0, 0, 0]
    return [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1]


def _update_layer_bbox(item: dict, rgba) -> None:
    l, t, r, b = _bbox_from_alpha(rgba[..., 3])
    item["left"] = l
    item["top"] = t
    item["right"] = r
    item["bottom"] = b


def _composite_layers(layers: list[dict], canvas_w: int, canvas_h: int):
    import numpy as np
    from PIL import Image

    comp = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    for item in sorted(layers, key=lambda x: int(x.get("zIndex", 0))):
        if item.get("visible", True) and os.path.isfile(item["imagePath"]):
            li = Image.open(item["imagePath"]).convert("RGBA")
            if li.size != (canvas_w, canvas_h):
                li = li.resize((canvas_w, canvas_h), Image.Resampling.LANCZOS)
            comp.alpha_composite(li)
    return np.array(comp)


def postprocess_layers_fidelity(
    manifest_layers: list[dict],
    saved_dir: str,
    layers_dir: str,
) -> bool:
    import cv2
    import numpy as np
    from PIL import Image

    src_p = os.path.join(saved_dir, "src_img.png")
    if not os.path.isfile(src_p):
        return False

    src_rgba = np.array(Image.open(src_p).convert("RGBA"))
    h, w = src_rgba.shape[:2]
    src_fg = src_rgba[..., 3] > 20
    src_fg_total = int(src_fg.sum())
    if src_fg_total < 200:
        return False

    fg_x1, fg_y1, fg_x2, fg_y2 = _bbox_from_alpha(src_rgba[..., 3])
    fg_h = max(1, fg_y2 - fg_y1)
    k3 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
    k5 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
    k7 = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (7, 7))
    changed = False

    # Pass A: Clamp every layer to src_img silhouette dilated by 1px (prevents empty-gap hallucinations)
    src_allow = cv2.dilate((src_rgba[..., 3] > 8).astype(np.uint8), k3, iterations=1) > 0
    for item in manifest_layers:
        p = item.get("imagePath", "")
        if not os.path.isfile(p):
            continue
        arr = np.array(Image.open(p).convert("RGBA"))
        if arr.shape[:2] != (h, w):
            continue
        out_m = (arr[..., 3] > 20) & (~src_allow)
        if int(out_m.sum()) > 10:
            arr[..., 3][~src_allow] = 0
            Image.fromarray(arr).save(p)
            _update_layer_bbox(item, arr)
            changed = True

    # Pass B: Split bottomwear vs legwear when coat lining behind thighs covers legwear
    ordered = sorted(manifest_layers, key=lambda x: int(x.get("zIndex", 0)))
    bw_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "bottomwear"), -1)
    lw_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower() == "legwear"), -1)
    has_bw_back = any(l.get("name", "").lower() == "bottomwear-back" for l in ordered)
    if bw_idx >= 0 and lw_idx >= 0 and bw_idx > lw_idx and not has_bw_back:
        bw_item = ordered[bw_idx]
        lw_item = ordered[lw_idx]
        bw_arr = np.array(Image.open(bw_item["imagePath"]).convert("RGBA"))
        lw_arr = np.array(Image.open(lw_item["imagePath"]).convert("RGBA"))
        if bw_arr.shape[:2] == (h, w) and lw_arr.shape[:2] == (h, w):
            overlap = (bw_arr[..., 3] > 100) & (lw_arr[..., 3] > 100) & (src_rgba[..., 3] > 120)
            src_rgb = src_rgba[..., :3].astype(np.float32)
            err_bw = np.mean(np.abs(bw_arr[..., :3].astype(np.float32) - src_rgb), axis=-1)
            err_lw = np.mean(np.abs(lw_arr[..., :3].astype(np.float32) - src_rgb), axis=-1)
            lw_wins = overlap & ((err_lw + 14.0) < err_bw) & (err_bw > 22.0)
            lw_clean = cv2.morphologyEx(lw_wins.astype(np.uint8) * 255, cv2.MORPH_OPEN, k5)
            lw_clean = cv2.morphologyEx(lw_clean, cv2.MORPH_CLOSE, k7) > 0
            if int(lw_clean.sum()) >= 350:
                split_y_top = int(np.where(lw_clean)[0].min())
                bw_back = bw_arr.copy()
                cut_y = max(0, split_y_top - 14)
                if cut_y > 0:
                    bw_back[:cut_y, :, 3] = 0
                bw_back_path = os.path.join(layers_dir, "bottomwear-back.png").replace("\\", "/")
                Image.fromarray(bw_back).save(bw_back_path)
                bl, bt, br, bb = _bbox_from_alpha(bw_back[..., 3])

                punch = cv2.dilate(lw_clean.astype(np.uint8), k3, iterations=1).astype(np.float32)
                punch_soft = cv2.GaussianBlur(punch, (3, 3), 0)
                bw_arr[..., 3] = np.clip(bw_arr[..., 3].astype(np.float32) * (1.0 - punch_soft), 0, 255).astype(np.uint8)
                Image.fromarray(bw_arr).save(bw_item["imagePath"])
                _update_layer_bbox(bw_item, bw_arr)

                ordered.insert(
                    lw_idx,
                    {
                        "id": f"layer_bw_back_{lw_idx}",
                        "name": "bottomwear-back",
                        "zIndex": lw_idx,
                        "visible": bool(bw_item.get("visible", True)),
                        "opacity": float(bw_item.get("opacity", 1.0)),
                        "left": bl,
                        "top": bt,
                        "right": br,
                        "bottom": bb,
                        "offsetX": 0,
                        "offsetY": 0,
                        "imagePath": bw_back_path,
                        "depthPath": bw_item.get("depthPath"),
                    },
                )
                for idx_r, it in enumerate(ordered):
                    it["zIndex"] = idx_r
                changed = True

    # Pass C: Recover missing/vanished head accessory (e.g. red hair bow) when headwear is empty or hidden
    hw_item = next((l for l in ordered if l.get("name", "").lower() == "headwear"), None)
    hw_visible_px = 0
    if hw_item is not None and hw_item.get("visible", True) and os.path.isfile(hw_item["imagePath"]):
        hw_check = np.array(Image.open(hw_item["imagePath"]).convert("RGBA"))
        hw_visible_px = int((hw_check[..., 3] > 20).sum())

    if hw_visible_px < 80:
        hx1, hy1, hx2, hy2 = 0, 0, 0, 0
        head_raw_mask = None
        head_raw_p = os.path.join(saved_dir, "head.png")
        if os.path.isfile(head_raw_p):
            head_raw = np.array(Image.open(head_raw_p).convert("RGBA"))
            if head_raw.shape[:2] == (h, w) and int((head_raw[..., 3] > 15).sum()) > 100:
                head_raw_mask = head_raw[..., 3] > 15
                hx1, hy1, hx2, hy2 = _bbox_from_alpha(head_raw[..., 3])
        if hx2 <= hx1 or hy2 <= hy1:
            face_it = next((l for l in ordered if l.get("name", "").lower() == "face"), None)
            if face_it is not None:
                hx1, hy1, hx2, hy2 = int(face_it["left"]), int(face_it["top"]), int(face_it["right"]), int(face_it["bottom"])

        if hx2 > hx1 and hy2 > hy1:
            head_w = max(24, hx2 - hx1)
            head_h = max(24, hy2 - hy1)
            zy1 = max(0, hy1 - int(0.8 * head_h))
            zy2 = min(h, hy2 + int(0.6 * head_h))
            zx1 = max(0, hx1 - int(1.6 * head_w))
            zx2 = min(w, hx2 + int(1.6 * head_w))
            head_zone = np.zeros((h, w), dtype=bool)
            head_zone[zy1:zy2, zx1:zx2] = True

            comp_rgba = _composite_layers(ordered, w, h)
            prot_names = (
                "face", "eyewhite", "irides", "eyelash", "eyebrow",
                "nose", "mouth", "ears", "neck", "topwear", "handwear",
            )
            face_protect = np.zeros((h, w), dtype=np.uint8)
            for it in ordered:
                nm = it.get("name", "").lower()
                if nm.startswith(prot_names) and os.path.isfile(it["imagePath"]):
                    pa = np.array(Image.open(it["imagePath"]).convert("RGBA"))
                    if pa.shape[:2] == (h, w):
                        face_protect[pa[..., 3] > 20] = 255
            face_protect_b = cv2.dilate(face_protect, k3, iterations=1) > 0

            head_missing = head_zone & (~face_protect_b) & (src_rgba[..., 3] > 35) & (comp_rgba[..., 3] < 35)
            rgb_diff = np.mean(
                np.abs(src_rgba[..., :3].astype(np.float32) - comp_rgba[..., :3].astype(np.float32)),
                axis=-1,
            )
            head_mismatch = (
                head_zone
                & (~face_protect_b)
                & (src_rgba[..., 3] > 140)
                & (comp_rgba[..., 3] > 60)
                & (rgb_diff > 44.0)
            )
            cand = cv2.morphologyEx(((head_missing | head_mismatch).astype(np.uint8) * 255), cv2.MORPH_OPEN, k3)
            cand = cv2.morphologyEx(cand, cv2.MORPH_CLOSE, k5)

            num_cc, lbl_cc, st_cc, cent_cc = cv2.connectedComponentsWithStats((cand > 0).astype(np.uint8), connectivity=8)
            hair_px_list = []
            skin_px_list = []
            for it_h in ordered:
                nm_h = it_h.get("name", "").lower()
                if not it_h.get("visible", True) or not os.path.isfile(it_h["imagePath"]):
                    continue
                if nm_h in ("front hair", "back hair", "face", "neck"):
                    ha = np.array(Image.open(it_h["imagePath"]).convert("RGBA"))
                    if ha.shape[:2] != (h, w):
                        continue
                    if nm_h in ("front hair", "back hair"):
                        hm_z = head_zone & (ha[..., 3] > 150) & (~face_protect_b)
                        if int(hm_z.sum()) > 50:
                            hair_px_list.append(ha[hm_z, :3].astype(np.float32))
                    else:
                        sm_z = ha[..., 3] > 180
                        if int(sm_z.sum()) > 50:
                            skin_px_list.append(ha[sm_z, :3].astype(np.float32))
            hair_med_rgb = (
                np.median(np.concatenate(hair_px_list, axis=0), axis=0)
                if hair_px_list
                else None
            )
            skin_med_rgb = (
                np.median(np.concatenate(skin_px_list, axis=0), axis=0)
                if skin_px_list
                else None
            )
            src_skin_z = (
                np.median(src_rgba[head_zone & face_protect_b & (src_rgba[..., 3] > 200), :3].astype(np.float32), axis=0)
                if int((head_zone & face_protect_b & (src_rgba[..., 3] > 200)).sum()) > 50
                else None
            )

            kept_hw = np.zeros((h, w), dtype=bool)
            for ci in range(1, num_cc):
                area = int(st_cc[ci, cv2.CC_STAT_AREA])
                if area < 150:
                    continue
                cy_cc = float(cent_cc[ci][1])
                if cy_cc > float(hy1) + 0.72 * float(head_h):
                    continue
                cm = lbl_cc == ci
                if head_raw_mask is not None and float((cm & head_raw_mask).sum()) / float(area) > 0.65:
                    continue
                mismatch_ov = int((cm & head_mismatch).sum())
                miss_ov = int((cm & head_missing).sum())
                mean_diff = float(rgb_diff[cm].mean()) if area > 0 else 0.0
                if mismatch_ov < 90 and not (miss_ov >= 160 and mean_diff > 55.0):
                    continue
                src_cm = cm & (src_rgba[..., 3] > 140)
                if int(src_cm.sum()) >= 30:
                    cm_rgb = np.median(src_rgba[src_cm, :3].astype(np.float32), axis=0)
                    if hair_med_rgb is not None and float(np.linalg.norm(cm_rgb - hair_med_rgb)) < 40.0:
                        continue
                    if skin_med_rgb is not None and float(np.linalg.norm(cm_rgb - skin_med_rgb)) < 42.0:
                        continue
                    if src_skin_z is not None and float(np.linalg.norm(cm_rgb - src_skin_z)) < 42.0:
                        continue
                kept_hw |= cm

            if int(kept_hw.sum()) >= 180:
                hw_edge = (cv2.dilate(kept_hw.astype(np.uint8), k3, iterations=1) > 0) & (src_rgba[..., 3] > 15) & (~face_protect_b)
                hw_out = np.zeros((h, w, 4), dtype=np.uint8)
                hw_out[hw_edge] = src_rgba[hw_edge]
                hw_save_p = os.path.join(layers_dir, "recovered_headwear.png").replace("\\", "/")
                Image.fromarray(hw_out).save(hw_save_p)
                hl, ht, hr, hb = _bbox_from_alpha(hw_out[..., 3])

                if hw_item is not None:
                    hw_item["imagePath"] = hw_save_p
                    hw_item["visible"] = True
                    hw_item["opacity"] = 1.0
                    hw_item["left"], hw_item["top"], hw_item["right"], hw_item["bottom"] = hl, ht, hr, hb
                    ordered.remove(hw_item)
                    new_hw = hw_item
                else:
                    new_hw = {
                        "id": "layer_recovered_headwear",
                        "name": "headwear",
                        "zIndex": 0,
                        "visible": True,
                        "opacity": 1.0,
                        "left": hl,
                        "top": ht,
                        "right": hr,
                        "bottom": hb,
                        "offsetX": 0,
                        "offsetY": 0,
                        "imagePath": hw_save_p,
                        "depthPath": None,
                    }

                fh_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower().startswith("front hair")), -1)
                if fh_idx >= 0:
                    ordered.insert(fh_idx + 1, new_hw)
                else:
                    bh_idx = next((i for i, l in enumerate(ordered) if l.get("name", "").lower().startswith("back hair")), len(ordered) - 1)
                    ordered.insert(max(0, bh_idx + 1), new_hw)
                for idx_r, it in enumerate(ordered):
                    it["zIndex"] = idx_r
                changed = True

    # Pass D: Recover missing contiguous source silhouette patches (e.g. cut-off boot tips)
    comp_after = _composite_layers(ordered, w, h)
    comp_alpha = comp_after[..., 3]
    missing = (src_rgba[..., 3] > 35) & (comp_alpha < 35)
    min_patch_area = max(60, int(src_fg_total * 0.0008))
    if int(missing.sum()) >= min_patch_area:
        k_touch = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (13, 13))
        exclude_targets = ("face", "eyewhite", "irides", "eyelash", "eyebrow", "nose", "mouth")
        layer_cache: dict[str, tuple] = {}
        for it in ordered:
            if it.get("visible", True) and os.path.isfile(it["imagePath"]):
                nm = it.get("name", "").lower()
                if not nm.startswith(exclude_targets):
                    la = np.array(Image.open(it["imagePath"]).convert("RGBA"))
                    if la.shape[:2] == (h, w):
                        layer_cache[it["id"]] = (it, la, la[..., 3] > 20)

        num_m, lbl_m, st_m, cent_m = cv2.connectedComponentsWithStats(missing.astype(np.uint8), connectivity=8)
        for ci in range(1, num_m):
            area = int(st_m[ci, cv2.CC_STAT_AREA])
            if area < min_patch_area:
                continue
            cm = lbl_m == ci
            patch_mask = (cv2.dilate(cm.astype(np.uint8), k3, iterations=1) > 0) & (src_rgba[..., 3] > 12) & (comp_alpha < 128)
            touch_zone = cv2.dilate(cm.astype(np.uint8), k_touch, iterations=1) > 0

            best_id = None
            best_score = 9
            for lid, (it, _la, lm_fg) in layer_cache.items():
                score = int((touch_zone & lm_fg).sum())
                if score > best_score:
                    best_score = score
                    best_id = lid

            if best_id is not None:
                it, la, _lm = layer_cache[best_id]
                la[patch_mask] = src_rgba[patch_mask]
                Image.fromarray(la).save(it["imagePath"])
                _update_layer_bbox(it, la)
                layer_cache[best_id] = (it, la, la[..., 3] > 20)
                comp_alpha[patch_mask] = src_rgba[patch_mask, 3]
                changed = True
            else:
                cy = float(cent_m[ci][1])
                has_fw = any(l.get("name", "").lower() == "footwear" and l.get("visible", True) for l in ordered)
                if cy >= fg_y1 + 0.72 * fg_h and not has_fw:
                    fw_out = np.zeros((h, w, 4), dtype=np.uint8)
                    fw_out[patch_mask] = src_rgba[patch_mask]
                    fw_p = os.path.join(layers_dir, "recovered_footwear.png").replace("\\", "/")
                    Image.fromarray(fw_out).save(fw_p)
                    fl, ft, fr, fb = _bbox_from_alpha(fw_out[..., 3])
                    new_fw = {
                        "id": "layer_recovered_footwear",
                        "name": "footwear",
                        "zIndex": 0,
                        "visible": True,
                        "opacity": 1.0,
                        "left": fl,
                        "top": ft,
                        "right": fr,
                        "bottom": fb,
                        "offsetX": 0,
                        "offsetY": 0,
                        "imagePath": fw_p,
                        "depthPath": None,
                    }
                    cur_lw = next((i for i, l in enumerate(ordered) if l.get("name", "").lower().startswith("legwear")), -1)
                    ordered.insert(cur_lw + 1 if cur_lw >= 0 else len(ordered), new_fw)
                    for idx_r, it in enumerate(ordered):
                        it["zIndex"] = idx_r
                    layer_cache["layer_recovered_footwear"] = (new_fw, fw_out, fw_out[..., 3] > 20)
                    comp_alpha[patch_mask] = src_rgba[patch_mask, 3]
                    changed = True

    if changed:
        manifest_layers[:] = ordered
        for idx, item in enumerate(manifest_layers):
            item["zIndex"] = idx
    return changed


def main():
    parser = argparse.ArgumentParser(description="Sketch2Asset See-Through Worker")
    parser.add_argument("--repo_root", required=True)
    parser.add_argument("--srcp", required=True)
    parser.add_argument("--out_dir", required=True)
    parser.add_argument("--model_layerdiff", required=True)
    parser.add_argument("--model_depth", required=True)
    parser.add_argument("--quant_mode", default="nf4", choices=["nf4", "none"])
    parser.add_argument("--resolution", type=int, default=1024)
    parser.add_argument("--resolution_depth", type=int, default=768)
    parser.add_argument("--steps", type=int, default=20)
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument("--tblr_split", action="store_true")
    parser.add_argument("--cpu_offload", action="store_true")
    parser.add_argument("--group_offload", default="auto", choices=["auto", "on", "off"])
    parser.add_argument("--cool_ms", type=int, default=60)
    args = parser.parse_args()

    emit("init", 2, "正在初始化 CUDA 与 TF32 TensorCore 推理环境...")
    sys.path.insert(0, args.repo_root)
    sys.path.insert(0, os.path.join(args.repo_root, "common"))
    sys.path.insert(0, os.path.join(args.repo_root, "inference", "scripts"))

    common_assets = os.path.join(args.repo_root, "common", "assets")
    root_assets = os.path.join(args.repo_root, "assets")
    if os.path.isdir(common_assets) and not os.path.exists(root_assets):
        shutil.copytree(common_assets, root_assets)

    import torch
    if not torch.cuda.is_available():
        raise RuntimeError("未检测到可用 NVIDIA CUDA GPU，本地拆图无法在纯 CPU 下稳定执行。")

    torch.backends.cuda.matmul.allow_tf32 = True
    torch.backends.cudnn.allow_tf32 = True

    # Warmup cuBLAS handle ahead of bitsandbytes 4-bit initialization
    a = torch.randn((16, 16), device="cuda", dtype=torch.bfloat16)
    b = torch.randn((16, 16), device="cuda", dtype=torch.bfloat16)
    _ = a @ b
    del a, b, _
    torch.cuda.synchronize()

    free_bytes, total_bytes = torch.cuda.mem_get_info()
    free_gb = free_bytes / (1024.0 ** 3)
    total_gb = total_bytes / (1024.0 ** 3)

    if args.quant_mode == "none":
        threshold_gb = 15.0 if args.resolution >= 1200 else 12.0
    else:
        threshold_gb = 11.0 if args.resolution >= 1200 else (10.2 if args.resolution >= 960 else 9.0)

    use_group_offload = False
    if not args.cpu_offload:
        if args.group_offload == "on":
            use_group_offload = True
        elif args.group_offload == "auto" and free_gb < threshold_gb:
            use_group_offload = True

    from modules.layerdiffuse.diffusers_kdiffusion_sdxl import KDiffusionStableDiffusionXLPipeline
    from utils.torch_utils import seed_everything
    from utils.inference_utils import further_extr
    import inference_psd_quantized as q
    from psd_tools import PSDImage

    # Patch prompt encoder for accelerate cpu_offload compatibility
    def _patched_encode(self, prompt):
        device = self._execution_device if getattr(self, "_all_hooks", None) else self.text_encoder.device
        if device.type == "cpu" and torch.cuda.is_available():
            device = torch.device("cuda")
        tokenizers = [self.tokenizer, self.tokenizer_2]
        text_encoders = [self.text_encoder, self.text_encoder_2]
        pooled_prompt_embeds = None
        prompt_embeds_list = []
        for tokenizer, text_encoder in zip(tokenizers, text_encoders):
            text_input_ids = tokenizer(
                prompt,
                padding="max_length",
                max_length=tokenizer.model_max_length,
                truncation=True,
                return_tensors="pt",
            ).input_ids
            with torch.inference_mode():
                prompt_embeds = text_encoder(
                    text_input_ids.to(device), output_hidden_states=True, return_dict=False
                )
            pooled_prompt_embeds = prompt_embeds[0]
            prompt_embeds = prompt_embeds[-1][-2]
            bs_embed, seq_len, _ = prompt_embeds.shape
            prompt_embeds = prompt_embeds.view(bs_embed, seq_len, -1)
            prompt_embeds_list.append(prompt_embeds)
        prompt_embeds = torch.concat(prompt_embeds_list, dim=-1).to(dtype=self.unet.dtype, device=device)
        pooled_prompt_embeds = pooled_prompt_embeds.view(bs_embed, -1)
        return prompt_embeds, pooled_prompt_embeds

    KDiffusionStableDiffusionXLPipeline.encode_cropped_prompt_77tokens = _patched_encode

    os.makedirs(args.out_dir, exist_ok=True)
    src_stem = os.path.splitext(os.path.basename(args.srcp))[0]
    saved_dir = os.path.join(args.out_dir, src_stem)

    run_args = argparse.Namespace(
        srcp=args.srcp,
        save_dir=args.out_dir,
        seed=args.seed,
        resolution=args.resolution,
        resolution_depth=args.resolution_depth,
        num_inference_steps=args.steps,
        save_to_psd=True,
        tblr_split=args.tblr_split,
        quant_mode=args.quant_mode,
        repo_id_layerdiff=args.model_layerdiff,
        repo_id_depth=args.model_depth,
        cpu_offload=args.cpu_offload,
        group_offload=use_group_offload,
    )

    torch.cuda.reset_peak_memory_stats()
    t0 = time.time()

    if use_group_offload:
        emit(
            "load_layerdiff",
            10,
            f"可用显存 {free_gb:.1f}/{total_gb:.1f} GB，已启用 Group Offload 防换页并加载权重...",
        )
    else:
        emit(
            "load_layerdiff",
            10,
            f"可用显存充足 ({free_gb:.1f}/{total_gb:.1f} GB)，全速加载 LayerDiff3D 权重...",
        )

    seed_everything(args.seed)
    try:
        pipe = q.build_layerdiff_pipeline(run_args)
    except Exception:
        torch.cuda.empty_cache()
        run_args.group_offload = False
        run_args.cpu_offload = True
        use_group_offload = False
        emit("load_layerdiff", 14, "显存模式触发回退，正在以 CPU Offload 安全模式加载模型...")
        pipe = q.build_layerdiff_pipeline(run_args)

    emit("layerdiff", 24, f"开始 23 层身体与头部遮挡拆解（{args.resolution}px · {args.steps}步）...")
    t1 = time.time()
    face_recovered = run_layerdiff_safe(
        pipe, args.srcp, args.out_dir, args.seed, args.steps, args.resolution, args.cool_ms
    )
    layerdiff_sec = round(time.time() - t1, 1)

    del pipe
    torch.cuda.empty_cache()

    emit("marigold", 72, "正在加载 Marigold 伪深度模型进行遮挡排序...")
    t2 = time.time()
    try:
        marigold_pipe = q.build_marigold_pipeline(run_args)
    except Exception:
        torch.cuda.empty_cache()
        run_args.group_offload = False
        run_args.cpu_offload = True
        marigold_pipe = q.build_marigold_pipeline(run_args)

    attach_step_progress(marigold_pipe, "marigold", 73, 89, "伪深度估计与层次排序", args.cool_ms)
    q.run_marigold(marigold_pipe, args.srcp, args.out_dir, args.seed, args.resolution_depth)
    marigold_sec = round(time.time() - t2, 1)

    del marigold_pipe
    torch.cuda.empty_cache()

    emit("psd", 90, "正在合成独立散件 PNG 与分层 PSD 文件...")
    further_extr(saved_dir, rotate=False, save_to_psd=True, tblr_split=args.tblr_split)

    psd_path = os.path.join(args.out_dir, f"{src_stem}.psd")
    if not os.path.isfile(psd_path):
        raise RuntimeError(f"PSD output missing at {psd_path}")

    # Export aligned full-canvas PNGs ordered strictly matching the PSD layer stack
    layers_dir = os.path.join(args.out_dir, "layers")
    os.makedirs(layers_dir, exist_ok=True)
    psd = PSDImage.open(psd_path)
    canvas_w, canvas_h = psd.width, psd.height
    manifest_layers = []

    from PIL import Image
    for z_idx, layer in enumerate(psd):
        safe_name = "".join(c if c.isalnum() or c in "-_" else "_" for c in layer.name)
        layer_file = f"{z_idx:02d}_{safe_name}.png"
        layer_abs = os.path.join(layers_dir, layer_file)
        canvas_img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
        part_img = layer.composite()
        if part_img is not None:
            part_rgba = part_img.convert("RGBA")
            canvas_img.paste(part_rgba, (layer.left, layer.top), part_rgba)
        canvas_img.save(layer_abs)

        depth_candidate = os.path.join(saved_dir, f"{layer.name}_depth.png")
        if not os.path.isfile(depth_candidate):
            base_tag = layer.name.rsplit("-", 1)[0]
            alt = os.path.join(saved_dir, f"{base_tag}_depth.png")
            depth_candidate = alt if os.path.isfile(alt) else ""

        manifest_layers.append({
            "id": f"layer_{z_idx:02d}_{safe_name}",
            "name": layer.name,
            "zIndex": z_idx,
            "visible": bool(layer.visible),
            "opacity": round(float(layer.opacity) / 255.0, 3),
            "left": int(layer.left),
            "top": int(layer.top),
            "right": int(layer.right),
            "bottom": int(layer.bottom),
            "offsetX": 0,
            "offsetY": 0,
            "imagePath": layer_abs.replace("\\", "/"),
            "depthPath": depth_candidate.replace("\\", "/") if depth_candidate else None,
        })

    rebuild_py = os.path.join(os.path.dirname(os.path.abspath(__file__)), "s2a_rebuild_psd.py")
    zorder_fixed = enforce_v3_face_zorder(manifest_layers, psd_path, rebuild_py, saved_dir)
    post_fixed = postprocess_layers_fidelity(manifest_layers, saved_dir, layers_dir)
    needs_rebuild = zorder_fixed or post_fixed

    recon_src = os.path.join(saved_dir, "reconstruction.png")
    recon_dest = os.path.join(args.out_dir, "reconstruction.png")
    if needs_rebuild:
        comp = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
        for item in sorted(manifest_layers, key=lambda x: int(x["zIndex"])):
            if item.get("visible", True):
                li = Image.open(item["imagePath"]).convert("RGBA")
                comp.alpha_composite(li)
        comp.save(recon_dest)
    elif os.path.isfile(recon_src):
        shutil.copyfile(recon_src, recon_dest)

    manifest = {
        "width": canvas_w,
        "height": canvas_h,
        "psdPath": psd_path.replace("\\", "/"),
        "reconstructionPath": recon_dest.replace("\\", "/"),
        "peakVramGb": round(torch.cuda.max_memory_allocated() / 1024**3, 2),
        "elapsedSec": round(time.time() - t0, 1),
        "layerdiffSec": layerdiff_sec,
        "marigoldSec": marigold_sec,
        "faceRecovered": bool(face_recovered),
        "usedGroupOffload": bool(run_args.group_offload),
        "layers": manifest_layers,
    }
    manifest_path = os.path.join(args.out_dir, "manifest.json")
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    if needs_rebuild and os.path.isfile(rebuild_py):
        try:
            subprocess.run(
                [sys.executable, rebuild_py, "--manifest", manifest_path, "--out_psd", psd_path],
                check=False,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
        except Exception:
            pass

    emit("done", 100, f"拆图完成（共 {len(manifest_layers)} 层 · 耗时 {manifest['elapsedSec']}s）", {"manifestPath": manifest_path.replace("\\", "/")})


if __name__ == "__main__":
    main()
"####;

pub const S2A_REBUILD_PSD_PY: &str = r####"import argparse
import json
from PIL import Image
from psd_tools import PSDImage
from psd_tools.api.layers import PixelLayer


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--out_psd", required=True)
    args = parser.parse_args()

    with open(args.manifest, "r", encoding="utf-8") as f:
        data = json.load(f)

    w = int(data["width"])
    h = int(data["height"])
    psd = PSDImage.new(mode="RGBA", size=(w, h))

    layers = sorted(data.get("layers", []), key=lambda x: int(x.get("zIndex", 0)))
    for item in layers:
        img = Image.open(item["imagePath"]).convert("RGBA")
        if img.size != (w, h):
            img = img.resize((w, h), Image.Resampling.LANCZOS)
        ox = int(item.get("offsetX", 0))
        oy = int(item.get("offsetY", 0))
        if ox != 0 or oy != 0:
            shifted = Image.new("RGBA", (w, h), (0, 0, 0, 0))
            shifted.paste(img, (ox, oy))
            img = shifted
        layer = PixelLayer.frompil(img, psd, layer_name=str(item.get("name", "layer")))
        layer.visible = bool(item.get("visible", True))
        opacity = float(item.get("opacity", 1.0))
        layer.opacity = max(0, min(255, int(round(opacity * 255))))
        psd.append(layer)

    psd.save(args.out_psd)
    print("OK:", args.out_psd)


if __name__ == "__main__":
    main()
"####;

pub const S2A_SPLIT_LAYER_PY: &str = r####"import argparse
import json
import os
import sys
import uuid

import cv2
import numpy as np
from PIL import Image


def compute_bbox(alpha: np.ndarray):
    ys, xs = np.where(alpha > 10)
    if len(xs) == 0:
        return [0, 0, 0, 0]
    return [int(xs.min()), int(ys.min()), int(xs.max()) + 1, int(ys.max()) + 1]


def split_lr(rgba: np.ndarray):
    alpha = rgba[..., 3]
    fg = (alpha > 15).astype(np.uint8)
    total_fg = int(fg.sum())
    if total_fg < 40:
        raise RuntimeError("当前图层有效像素过少，无法进行左右拆分")

    num_labels, labels, stats, centroids = cv2.connectedComponentsWithStats(fg, connectivity=8)
    min_area = max(25, int(total_fg * 0.01))
    valid_ids = [idx for idx in range(1, num_labels) if int(stats[idx, cv2.CC_STAT_AREA]) >= min_area]

    mask_left_canvas = np.zeros_like(alpha, dtype=bool)
    mask_right_canvas = np.zeros_like(alpha, dtype=bool)

    if len(valid_ids) >= 2:
        valid_ids.sort(key=lambda i: int(stats[i, cv2.CC_STAT_AREA]), reverse=True)
        seed_a, seed_b = valid_ids[0], valid_ids[1]
        cx_a, cy_a = centroids[seed_a]
        cx_b, cy_b = centroids[seed_b]
        # Screen-left corresponds to character's right (-r) and screen-right to character's left (-l)
        if cx_a <= cx_b:
            r_anchor, l_anchor = (cx_a, cy_a), (cx_b, cy_b)
        else:
            r_anchor, l_anchor = (cx_b, cy_b), (cx_a, cy_a)

        for idx in range(1, num_labels):
            cx, cy = centroids[idx]
            d_r = (cx - r_anchor[0]) ** 2 + (cy - r_anchor[1]) ** 2
            d_l = (cx - l_anchor[0]) ** 2 + (cy - l_anchor[1]) ** 2
            if d_r <= d_l:
                mask_left_canvas |= (labels == idx)
            else:
                mask_right_canvas |= (labels == idx)
    else:
        ys, xs = np.where(fg > 0)
        x1, x2 = int(xs.min()), int(xs.max())
        w = max(1, x2 - x1 + 1)
        if w < 16:
            raise RuntimeError("当前图层过窄，无法垂直切分为左右两层")
        lo = x1 + int(w * 0.30)
        hi = x1 + int(w * 0.70)
        col_density = fg[:, lo:hi + 1].sum(axis=0)
        split_x = lo + int(np.argmin(col_density))
        xs_grid = np.arange(rgba.shape[1])[None, :]
        mask_left_canvas = (fg > 0) & (xs_grid <= split_x)
        mask_right_canvas = (fg > 0) & (xs_grid > split_x)

    if int(mask_left_canvas.sum()) < 20 or int(mask_right_canvas.sum()) < 20:
        raise RuntimeError("左右两侧其中一边无有效像素，未能完成对称拆分")

    out_r = rgba.copy()
    out_r[~mask_left_canvas, 3] = 0
    out_l = rgba.copy()
    out_l[~mask_right_canvas, 3] = 0
    return [("-r", out_r), ("-l", out_l)]


def split_depth(rgba: np.ndarray, depth_path: str | None):
    from sklearn.cluster import KMeans

    if not depth_path or not os.path.isfile(depth_path):
        raise RuntimeError("当前图层缺少关联的伪深度图，无法按前后景深拆分")

    h, w = rgba.shape[:2]
    alpha = rgba[..., 3]
    fg_mask = alpha > 15
    fg_count = int(fg_mask.sum())
    if fg_count < 80:
        raise RuntimeError("当前图层有效像素过少，无法按深度拆分")

    depth_img = Image.open(depth_path).convert("L")
    if depth_img.size != (w, h):
        depth_img = depth_img.resize((w, h), Image.Resampling.BILINEAR)
    depth_arr = np.array(depth_img, dtype=np.float32)

    fg_depths = depth_arr[fg_mask]
    if float(np.std(fg_depths)) < 3.5:
        raise RuntimeError("当前图层伪深度层次过于平缓，无法按前后拆分")

    if len(fg_depths) > 4000:
        step = max(1, len(fg_depths) // 4000)
        samples = fg_depths[::step][:4000].reshape(-1, 1)
    else:
        samples = fg_depths.reshape(-1, 1)

    km = KMeans(n_clusters=2, n_init="auto", random_state=42).fit(samples)
    centers = km.cluster_centers_.flatten()
    pred = km.predict(fg_depths.reshape(-1, 1))

    # Marigold depth convention: smaller value = closer to camera (front), larger = farther (back)
    front_cluster = int(np.argmin(centers))
    back_cluster = 1 - front_cluster

    front_mask = np.zeros((h, w), dtype=bool)
    back_mask = np.zeros((h, w), dtype=bool)
    ys, xs = np.where(fg_mask)
    front_mask[ys[pred == front_cluster], xs[pred == front_cluster]] = True
    back_mask[ys[pred == back_cluster], xs[pred == back_cluster]] = True

    if int(front_mask.sum()) < 25 or int(back_mask.sum()) < 25:
        raise RuntimeError("前后聚类后的其中一层面积过小，未能拆出有效分层")

    # Fill convex enclosed pockets between back parts AND a 15px adjacent band behind front layer
    pts = cv2.findNonZero(back_mask.astype(np.uint8))
    hull_mask = np.zeros((h, w), dtype=np.uint8)
    if pts is not None and len(pts) >= 3:
        hull = cv2.convexHull(pts)
        cv2.fillConvexPoly(hull_mask, hull, 1)
    enclosed_occ = (hull_mask > 0) & front_mask

    k_band = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (15, 15))
    near_band = (cv2.dilate(back_mask.astype(np.uint8), k_band) > 0) & front_mask
    occ_fill = (enclosed_occ | near_band) & fg_mask & (~back_mask)

    seed_rgb = np.empty((h, w, 3), dtype=np.uint8)
    back_median = np.median(rgba[back_mask, :3], axis=0)
    seed_rgb[:, :] = np.clip(back_median, 0, 255).astype(np.uint8)
    seed_rgb[back_mask] = rgba[back_mask, :3]

    if int(occ_fill.sum()) > 0:
        back_rgb = cv2.inpaint(seed_rgb, occ_fill.astype(np.uint8) * 255, 7, cv2.INPAINT_TELEA)
    else:
        back_rgb = seed_rgb

    back_alpha = np.where(back_mask | occ_fill, alpha, 0).astype(np.uint8)
    out_back = np.dstack([back_rgb, back_alpha])

    out_front = rgba.copy()
    out_front[~front_mask, 3] = 0

    # Return back layer first (lower Z), front layer second (higher Z)
    return [("-back", out_back), ("-front", out_front)]


def main():
    parser = argparse.ArgumentParser(description="Sketch2Asset Sub-Layer Splitter")
    parser.add_argument("--manifest", required=True)
    parser.add_argument("--layer_id", required=True)
    parser.add_argument("--mode", required=True, choices=["lr", "depth"])
    args = parser.parse_args()

    manifest_path = os.path.abspath(args.manifest)
    with open(manifest_path, "r", encoding="utf-8") as f:
        manifest = json.load(f)

    layers = sorted(manifest.get("layers", []), key=lambda x: int(x.get("zIndex", 0)))
    target_idx = next((i for i, l in enumerate(layers) if l.get("id") == args.layer_id), -1)
    if target_idx < 0:
        raise RuntimeError(f"未在清单中找到目标图层: {args.layer_id}")

    target = layers[target_idx]
    rgba = np.array(Image.open(target["imagePath"]).convert("RGBA"))

    if args.mode == "lr":
        parts = split_lr(rgba)
    else:
        parts = split_depth(rgba, target.get("depthPath"))

    layers_dir = os.path.dirname(target["imagePath"])
    os.makedirs(layers_dir, exist_ok=True)
    base_name = target["name"]
    new_items = []
    uid_tag = uuid.uuid4().hex[:6]

    for suffix, arr in parts:
        new_name = f"{base_name}{suffix}"
        safe_name = "".join(c if c.isalnum() or c in "-_" else "_" for c in new_name)
        new_id = f"layer_{safe_name}_{uid_tag}"
        out_png = os.path.join(layers_dir, f"{safe_name}_{uid_tag}.png")
        Image.fromarray(arr).save(out_png)
        l, t, r, b = compute_bbox(arr[..., 3])
        new_items.append({
            "id": new_id,
            "name": new_name,
            "zIndex": 0,
            "visible": bool(target.get("visible", True)),
            "opacity": float(target.get("opacity", 1.0)),
            "left": l,
            "top": t,
            "right": r,
            "bottom": b,
            "offsetX": int(target.get("offsetX", 0)),
            "offsetY": int(target.get("offsetY", 0)),
            "imagePath": out_png.replace("\\", "/"),
            "depthPath": target.get("depthPath"),
        })

    updated = layers[:target_idx] + new_items + layers[target_idx + 1:]
    for z_idx, item in enumerate(updated):
        item["zIndex"] = z_idx

    manifest["layers"] = updated
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    print("S2A_SPLIT_OK", flush=True)


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"S2A_SPLIT_ERR: {exc}", file=sys.stderr, flush=True)
        sys.exit(1)
"####;

pub const S2A_PIP_PACKAGES: &[&str] = &[
    "diffusers==0.37.0",
    "transformers==5.0.0",
    "huggingface-hub==1.7.2",
    "accelerate==1.13.0",
    "safetensors==0.7.0",
    "numpy==2.2.6",
    "opencv-python==4.13.0.92",
    "pillow==12.1.1",
    "pillow-jxl-plugin==1.3.7",
    "scikit-learn==1.7.2",
    "scipy==1.15.3",
    "scikit-image==0.25.2",
    "einops==0.8.2",
    "kornia==0.8.2",
    "omegaconf==2.3.0",
    "psd-tools[composite]==1.14.2",
    "pycocotools==2.0.11",
    "matplotlib==3.9.4",
    "pandas==2.3.3",
    "natsort==8.4.0",
    "termcolor==3.3.0",
    "click==8.3.1",
    "tqdm==4.67.3",
    "timm==1.0.31",
    "bitsandbytes==0.49.2",
    "simple-parsing==0.1.8",
    "tokenizers==0.22.2",
];
