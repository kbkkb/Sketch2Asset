use image::{Rgba, RgbaImage};

/// Check whether `haystack` (typically lowercased) contains `word` bounded by non-ASCII-alphanumeric
/// characters on both sides so substrings like `"hit"` inside `"white"` do not falsely match.
fn contains_ascii_word(haystack: &str, word: &str) -> bool {
    if word.is_empty() || word.len() > haystack.len() {
        return false;
    }
    let h_bytes = haystack.as_bytes();
    let w_len = word.len();
    let mut search_from = 0usize;
    while let Some(rel) = haystack[search_from..].find(word) {
        let start = search_from + rel;
        let end = start + w_len;
        let left_ok = start == 0 || !h_bytes[start - 1].is_ascii_alphanumeric();
        let right_ok = end == h_bytes.len() || !h_bytes[end].is_ascii_alphanumeric();
        if left_ok && right_ok {
            return true;
        }
        let next_char_len = haystack[start..]
            .chars()
            .next()
            .map(|c| c.len_utf8())
            .unwrap_or(1);
        search_from = start + next_char_len;
    }
    false
}

fn contains_any_ascii_word(haystack: &str, words: &[&str]) -> bool {
    words.iter().any(|&w| contains_ascii_word(haystack, w))
}

/// Infer `(rows, cols)` grid layout from explicit arguments or prompt text.
/// Defaults to `(2, 3)` (6 frames) if "6" is mentioned, `(1, 4)` if "4" is mentioned, or `(2, 4)` (8 frames).
pub fn infer_sprite_grid(
    prompt: &str,
    explicit_rows: Option<u32>,
    explicit_cols: Option<u32>,
) -> (u32, u32) {
    if let (Some(r), Some(c)) = (explicit_rows, explicit_cols) {
        if r >= 1 && c >= 1 && r <= 8 && c <= 12 {
            return (r, c);
        }
    }

    let lower = prompt.to_lowercase();

    // Check explicit RxC patterns like "2x3", "1x6", "2x4", "3x3"
    let grid_patterns = [
        ("1x4", (1, 4)),
        ("1×4", (1, 4)),
        ("1x6", (1, 6)),
        ("1×6", (1, 6)),
        ("1x8", (1, 8)),
        ("2x2", (2, 2)),
        ("2×2", (2, 2)),
        ("2x3", (2, 3)),
        ("2×3", (2, 3)),
        ("3x2", (3, 2)),
        ("2x4", (2, 4)),
        ("2×4", (2, 4)),
        ("3x3", (3, 3)),
        ("3×3", (3, 3)),
        ("4x2", (4, 2)),
        ("4x4", (4, 4)),
    ];
    for (pat, rc) in grid_patterns {
        if contains_ascii_word(&lower, pat) {
            return rc;
        }
    }

    if contains_any_ascii_word(
        &lower,
        &[
            "6-frame",
            "6 frames",
            "6 frame",
            "six-frame",
            "six frames",
            "six frame",
        ],
    ) || lower.contains("6帧")
        || lower.contains("六帧")
    {
        if contains_any_ascii_word(&lower, &["horizontal", "single row"])
            || lower.contains("单行")
            || lower.contains("横向一排")
        {
            return (1, 6);
        }
        return (2, 3);
    }

    if contains_any_ascii_word(
        &lower,
        &[
            "4-frame",
            "4 frames",
            "4 frame",
            "four-frame",
            "four frames",
            "four frame",
        ],
    ) || lower.contains("4帧")
        || lower.contains("四帧")
    {
        return (1, 4);
    }

    if contains_any_ascii_word(
        &lower,
        &[
            "9-frame",
            "9 frames",
            "9 frame",
            "nine-frame",
            "nine frames",
            "nine frame",
        ],
    ) || lower.contains("9帧")
    {
        return (3, 3);
    }

    (2, 4)
}

/// Detect whether the action is a jump / airborne motion whose vertical elevation arc should be preserved.
pub fn is_airborne_motion(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    contains_any_ascii_word(
        &lower,
        &[
            "jump",
            "jumps",
            "jumping",
            "jumped",
            "leap",
            "leaps",
            "leaping",
            "leaped",
            "leapt",
            "hop",
            "hops",
            "hopping",
            "hopped",
            "airborne",
        ],
    ) || lower.contains("跳跃")
        || lower.contains("起跳")
        || lower.contains("腾空")
        || lower.contains("滞空")
}

/// Generate a strict per-frame keyframe choreography breakdown so diffusion models never draw repetitive poses in adjacent cells,
/// while preserving the caller's full character, weapon, and style description.
pub fn build_keyframe_choreography_prompt(user_prompt: &str, rows: u32, cols: u32) -> String {
    let total = (rows * cols).clamp(1, 16);
    let lower = user_prompt.to_lowercase();

    let is_death = contains_any_ascii_word(
        &lower,
        &[
            "death",
            "deaths",
            "die",
            "dies",
            "dying",
            "died",
            "defeat",
            "defeats",
            "defeated",
            "defeating",
            "dead",
            "collapse",
            "collapses",
            "collapsing",
            "collapsed",
            "knockdown",
            "knockdowns",
            "knock-down",
        ],
    ) || lower.contains("死亡")
        || lower.contains("倒地")
        || lower.contains("阵亡")
        || lower.contains("战败")
        || lower.contains("击倒");

    let is_hurt = !is_death
        && (contains_any_ascii_word(
            &lower,
            &[
                "hurt",
                "hurts",
                "hurting",
                "hit",
                "hits",
                "hitting",
                "flinch",
                "flinches",
                "flinching",
                "flinched",
                "damage",
                "damages",
                "damaging",
                "damaged",
                "knockback",
                "knock-back",
            ],
        ) || lower.contains("受击")
            || lower.contains("受伤")
            || lower.contains("挨打")
            || lower.contains("硬直"));

    let is_jump = is_airborne_motion(user_prompt);

    let is_attack = !is_death
        && !is_hurt
        && (contains_any_ascii_word(
            &lower,
            &[
                "attack",
                "attacks",
                "attacking",
                "attacked",
                "slash",
                "slashes",
                "slashing",
                "slashed",
                "strike",
                "strikes",
                "striking",
                "struck",
                "swing",
                "swings",
                "swinging",
                "swung",
                "thrust",
                "thrusts",
                "thrusting",
                "stab",
                "stabs",
                "stabbing",
                "stabbed",
                "punch",
                "punches",
                "punching",
                "punched",
            ],
        ) || lower.contains("攻击")
            || lower.contains("挥砍")
            || lower.contains("劈砍")
            || lower.contains("刺")
            || lower.contains("出招")
            || lower.contains("斩"));

    let is_walk_or_run = !is_death
        && !is_hurt
        && !is_jump
        && !is_attack
        && (contains_any_ascii_word(
            &lower,
            &[
                "walk",
                "walks",
                "walking",
                "walked",
                "run",
                "runs",
                "running",
                "move",
                "moves",
                "moving",
                "moved",
                "sprint",
                "sprints",
                "sprinting",
                "sprinted",
                "dash",
                "dashes",
                "dashing",
                "dashed",
            ],
        ) || lower.contains("行走")
            || lower.contains("奔跑")
            || lower.contains("移动")
            || lower.contains("跑步")
            || lower.contains("走路"));

    let is_idle = !is_death
        && !is_hurt
        && !is_jump
        && !is_attack
        && !is_walk_or_run
        && (contains_any_ascii_word(
            &lower,
            &[
                "idle",
                "idles",
                "idling",
                "breath",
                "breathe",
                "breathes",
                "breathing",
                "stand",
                "stands",
                "standing",
            ],
        ) || lower.contains("待机")
            || lower.contains("站立")
            || lower.contains("呼吸"));

    let pose_list: Vec<&str> = if is_death {
        if total <= 4 {
            vec![
                "Fatal Impact Stagger — struck heavily, torso recoiling backward, losing footing",
                "Buckling to Knees — legs giving out, dropping onto both knees, upper body slumping downward",
                "Collapsing Toward Ground — losing all support, body tipping horizontally toward the floor",
                "Defeated Lying Flat on Ground — character lying completely horizontal and motionless flat on the ground baseline (FINAL NON-LOOPING POSE — do NOT stand back up)",
            ]
        } else {
            vec![
                "Fatal Impact Recoil — chest and head snapping back from lethal blow, losing balance",
                "Staggering Weakness — knees buckling inward, guard dropping",
                "Dropping to Knees — collapsing down onto knees, torso slumping forward",
                "Tipping Over Horizontally — body falling forward/sideways toward the floor",
                "Hitting the Ground — landing horizontally along the ground baseline",
                "Motionless Defeated Corpse Pose — lying completely flat and still on the ground baseline, weapon resting beside body (FINAL NON-LOOPING STATE — do NOT return to standing)",
                "Motionless Final Rest — remaining flat on the ground",
                "Motionless Final Rest — complete stillness on the ground",
            ]
        }
    } else if is_hurt {
        vec![
            "Initial Hit Impact — sharp flinch, head and torso snapping backward from strike",
            "Peak Knockback Recoil — weight pushed onto back heel, arms shielding chest, weapon held tight",
            "Stagger Bracing — crouching slightly to absorb momentum and regain footing",
            "Recovery to Guard — returning smoothly to alert combat stance with weapon ready",
            "Secondary Recoil Stabilization — settling footwork",
            "Balanced Combat Ready — upright guard posture",
        ]
    } else if is_jump {
        if total <= 4 {
            vec![
                "Deep Crouch Take-Off Preparation (ON GROUND BASELINE) — knees bent low, arms swung back coiled to jump, feet planted on ground line",
                "Explosive Upward Launch & Ascending Tuck (AIRBORNE HIGH IN CELL) — body rising into upper half of cell, knees tucked high, feet lifted far above ground baseline",
                "Peak Airborne Apex & Descending Stretch (HIGHEST POINT IN AIR) — suspended high near top of cell safe zone far above ground baseline, legs beginning to extend downward for landing",
                "Landing Impact Cushion & Recovery (BACK ON GROUND BASELINE) — feet touching back down on ground line with bent knees absorbing landing impact",
            ]
        } else {
            vec![
                "Deep Crouch Preparation (ON GROUND BASELINE) — knees bent low, arms swung back preparing to launch",
                "Explosive Upward Launch (LEAVING GROUND) — legs fully extended pushing off ground, body rising steeply",
                "Ascending Mid-Air Tuck (AIRBORNE HIGH) — knees tucked upward in upper cell region, feet high above ground baseline",
                "High Apex Float (PEAK AIRBORNE HEIGHT) — suspended at highest point of jump arc far above ground baseline",
                "Descending Fall (FALLING FROM AIR) — body tilting downward, legs extending toward ground",
                "Landing Impact Cushion (BACK ON GROUND BASELINE) — feet touching back down on ground line with bent knees absorbing landing force",
                "Rebound Stabilization (ON GROUND) — torso rising from crouch back to standing balance",
                "Neutral Standing Recovery (ON GROUND) — upright balanced posture",
            ]
        }
    } else if is_walk_or_run {
        vec![
            "Full Stride Contact A — front leg extended far forward (heel landing), back leg stretched far behind, opposite arm swung forward, held weapon clearly visible",
            "Down / Recoil Absorption A — front knee deeply bent taking full weight, torso at lowest crouch point, weapon held firmly in hand",
            "Mid-Stride Passing A — standing leg straight under hip, other knee lifted high crossing torso midline, compact vertical silhouette with weapon intact",
            "Push-Off / High-Point A — back toe pushing forcefully off ground, leading knee driving high upward, torso at peak stride",
            "Full Stride Contact B (Opposite Phase) — opposite leg extended far forward landing, first leg stretched far behind, reversed arm swing, weapon kept consistent",
            "Down / Recoil B — opposite knee bent absorbing impact, body lowered, cape/hair trailing dynamically",
            "Mid-Stride Passing B — opposite leg supporting upright, trailing leg swinging rapidly past midline",
            "Push-Off Recovery B — forceful rear toe push-off returning smoothly toward Frame 1 posture",
        ]
    } else if is_attack {
        vec![
            "Combat Ready Stance — balanced guard posture, weapon held clearly inside cell safe zone",
            "Deep Wind-Up / Anticipation — torso coiled back, weapon drawn behind shoulder building maximum tension",
            "Explosive Forward Initiation — stepping forward, weapon accelerating mid-swing across chest",
            "Full Extension Impact Frame — weapon extended forward with dynamic slash/thrust inside this cell's safe zone (do NOT cross into adjacent cell)",
            "Follow-Through Overshoot — weapon completing arc, body weight shifted onto front foot",
            "Guard Recovery — pulling weapon back into balanced fighting stance",
            "Secondary Follow-Up Strike — reverse slash with distinct silhouette",
            "Final Stance Return — settling back into balanced combat guard",
        ]
    } else if is_idle {
        vec![
            "Neutral Upright Stance — chest relaxed, held weapon at rest",
            "Inhale Expansion — chest and shoulders rising slightly, cape/hair swaying gently",
            "Peak Breath Hold — subtle upward posture shift, alert expression, slight knee flex",
            "Exhale Release — shoulders settling down smoothly back toward neutral rest",
            "Subtle Weight Shift — slight stance adjustment keeping silhouette alive",
            "Return to Neutral — balanced ready posture completing seamless loop",
            "Secondary Micro-Motion — gentle weapon glint or cloth sway",
            "Loop Closure Frame — matching neutral stance",
        ]
    } else {
        vec![
            "Initial Pose / Preparation (0% phase) — clear starting silhouette matching prompt",
            "Early Motion Build-Up (25% phase) — major limb angle shift from Frame 1",
            "Mid-Action Climax (50% phase) — dynamic expressive peak posture",
            "Follow-Through Phase (75% phase) — secondary motion and momentum progression",
            "Late Action Progression (90% phase) — distinct transitional pose",
            "Final Action Pose (100% phase) — clear completion of described motion",
            "Extended Follow-Through — distinct pose",
            "Final Settling Pose — clean completion",
        ]
    };

    let mut frame_lines = Vec::new();
    for idx in 0..total {
        let r = idx / cols + 1;
        let c = idx % cols + 1;
        let desc = pose_list[(idx as usize) % pose_list.len()];
        frame_lines.push(format!(
            "- Frame #{} (Row {}, Col {}): {}",
            idx + 1,
            r,
            c,
            desc
        ));
    }

    let baseline_rule = if is_jump {
        "2. GROUND VS AIRBORNE VERTICAL ARC: Frame #1 (take-off) and the final Frame (landing) MUST have feet planted on the bottom ground baseline (86% cell height), while intermediate airborne frames MUST be drawn physically higher up in their cell with feet clearly elevated above the ground baseline to depict a true jump arc."
    } else if is_death {
        "2. NON-LOOPING COLLAPSE TO GROUND: Progress strictly from standing hit in Frame #1 to lying completely flat and motionless on the ground baseline in the final frame. Do NOT loop back to standing."
    } else {
        "2. SHARED GROUND BASELINE & SCALE: Within each row, all characters MUST stand on the exact same horizontal foot baseline with identical head-to-body scale."
    };

    format!(
        "CHARACTER & ACTION SPECIFICATION (MUST FOLLOW STRICTLY): {user_prompt}\n\n\
        STRICT {rows}x{cols} SPRITE SHEET GRID & KEYFRAME CHOREOGRAPHY ({total} FRAMES TOTAL, reading left-to-right, top-to-bottom):\n\
        1. UNIFORM CELL DIVISION & SAFE ZONE: Divide the canvas into an exact {rows} rows by {cols} columns invisible grid. Place exactly ONE full-body character (including any extended sword/weapon tip) strictly inside the center 75% safe zone of each cell so weapons NEVER touch or cross into neighboring cells.\n\
        {baseline_rule}\n\
        3. WEAPON, PROP & IDENTITY CONSISTENCY: Keep the character's held weapon, armor/outfit, colors, facing direction, and proportions 100% identical across all {total} frames. NEVER omit or drop held weapons.\n\
        4. MANDATORY FRAME-BY-FRAME POSE BREAKDOWN (NO DUPLICATE ADJACENT POSES):\n{}\n\
        5. NO GRID LINES OR TEXT: Pure solid background (#FFFFFF), zero ground shadow, NO visible grid lines, boxes, or numbers.",
        frame_lines.join("\n")
    )
}

/// Detect whether the prompt or archetype requests pixel art quantization.
pub fn is_pixel_art_requested(archetype: &str, prompt: &str, style_prompt: &str) -> bool {
    if archetype.eq_ignore_ascii_case("pixel_sprite") {
        return true;
    }
    let combined = format!("{} {}", prompt, style_prompt).to_lowercase();
    contains_any_ascii_word(
        &combined,
        &[
            "pixel art",
            "pixel-art",
            "pixel_art",
            "pixel sprite",
            "pixel_sprite",
            "16-bit",
            "8-bit",
        ],
    ) || combined.contains("像素")
}

#[derive(Debug, Clone, Copy)]
struct PivotBox {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
    center_x: f32,
    has_content: bool,
}

#[inline]
fn pixel_chroma(px: Rgba<u8>) -> i32 {
    let max_c = px[0].max(px[1]).max(px[2]) as i32;
    let min_c = px[0].min(px[1]).min(px[2]) as i32;
    max_c - min_c
}

fn is_foreground_pixel(px: Rgba<u8>, has_alpha: bool, bg_rgb: (i32, i32, i32)) -> bool {
    if has_alpha {
        // Keep opaque/semi-opaque sprite details plus thin translucent chromatic slash/glow VFX (alpha >= 14),
        // while still filtering low-alpha neutral white/gray anti-aliased fringe.
        px[3] > 28 || (px[3] >= 14 && pixel_chroma(px) >= 30)
    } else {
        if px[3] <= 28 {
            return false;
        }
        let diff = (px[0] as i32 - bg_rgb.0).abs()
            + (px[1] as i32 - bg_rgb.1).abs()
            + (px[2] as i32 - bg_rgb.2).abs();
        diff > 28 || (diff > 16 && pixel_chroma(px) >= 30)
    }
}

fn sample_corner_bg(img: &RgbaImage) -> (bool, (i32, i32, i32)) {
    let w = img.width();
    let h = img.height();
    let mut transparent_count = 0usize;
    let step_x = (w / 16).max(1);
    let step_y = (h / 16).max(1);
    for y in (0..h).step_by(step_y as usize) {
        for x in (0..w).step_by(step_x as usize) {
            if img.get_pixel(x, y)[3] < 200 {
                transparent_count += 1;
            }
        }
    }
    let has_alpha = transparent_count >= 4;

    let corners = [
        (1.min(w - 1), 1.min(h - 1)),
        (w.saturating_sub(2), 1.min(h - 1)),
        (1.min(w - 1), h.saturating_sub(2)),
        (w.saturating_sub(2), h.saturating_sub(2)),
    ];
    let (mut r_sum, mut g_sum, mut b_sum) = (0i32, 0i32, 0i32);
    for (cx, cy) in corners {
        let p = img.get_pixel(cx, cy);
        r_sum += p[0] as i32;
        g_sum += p[1] as i32;
        b_sum += p[2] as i32;
    }
    (has_alpha, (r_sum / 4, g_sum / 4, b_sum / 4))
}

/// Strip accidental full-span vertical/horizontal grid border lines drawn by the model near cell boundaries.
fn strip_grid_border_lines(
    img: &mut RgbaImage,
    rows: u32,
    cols: u32,
    has_alpha: bool,
    bg_rgb: (i32, i32, i32),
) {
    let w = img.width();
    let h = img.height();
    let bg_fill = if has_alpha {
        Rgba([0, 0, 0, 0])
    } else {
        Rgba([bg_rgb.0 as u8, bg_rgb.1 as u8, bg_rgb.2 as u8, 255])
    };

    let band_x = ((w as f32 * 0.012).round() as i32).max(3);
    for c in 0..=cols {
        let nom_x = ((c * w) as f32 / cols as f32).round() as i32;
        let x0 = (nom_x - band_x).max(0) as u32;
        let x1 = (nom_x + band_x).min(w as i32 - 1) as u32;
        for x in x0..=x1 {
            let mut fg_cnt = 0u32;
            for y in (0..h).step_by(2) {
                if is_foreground_pixel(*img.get_pixel(x, y), has_alpha, bg_rgb) {
                    fg_cnt += 1;
                }
            }
            if fg_cnt as f32 > (h as f32 / 2.0) * 0.65 {
                for y in 0..h {
                    img.put_pixel(x, y, bg_fill);
                }
            }
        }
    }

    let band_y = ((h as f32 * 0.012).round() as i32).max(3);
    for r in 0..=rows {
        let nom_y = ((r * h) as f32 / rows as f32).round() as i32;
        let y0 = (nom_y - band_y).max(0) as u32;
        let y1 = (nom_y + band_y).min(h as i32 - 1) as u32;
        for y in y0..=y1 {
            let mut fg_cnt = 0u32;
            for x in (0..w).step_by(2) {
                if is_foreground_pixel(*img.get_pixel(x, y), has_alpha, bg_rgb) {
                    fg_cnt += 1;
                }
            }
            if fg_cnt as f32 > (w as f32 / 2.0) * 0.65 {
                for x in 0..w {
                    img.put_pixel(x, y, bg_fill);
                }
            }
        }
    }
}

/// Compute bounding box, foot baseline (`max_y`), and lower-body squared-density horizontal center (`center_x`).
fn compute_character_foot_pivot(
    img: &RgbaImage,
    has_alpha: bool,
    bg_rgb: (i32, i32, i32),
) -> PivotBox {
    let w = img.width() as i32;
    let h = img.height() as i32;
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = -1i32;
    let mut max_y = -1i32;

    for y in 0..h {
        for x in 0..w {
            let px = *img.get_pixel(x as u32, y as u32);
            if is_foreground_pixel(px, has_alpha, bg_rgb) {
                if x < min_x {
                    min_x = x;
                }
                if x > max_x {
                    max_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if max_x < min_x || max_y < min_y {
        return PivotBox {
            min_x: 0,
            min_y: 0,
            max_x: w - 1,
            max_y: h - 1,
            center_x: w as f32 / 2.0,
            has_content: false,
        };
    }

    let char_h = (max_y - min_y + 1).max(1);
    let lower_top = min_y + ((char_h as f32 * 0.35).floor() as i32);
    let mut weighted_sum_x = 0.0f64;
    let mut total_weight = 0.0f64;

    for x in min_x..=max_x {
        let mut col_count = 0i64;
        for y in lower_top..=max_y {
            let px = *img.get_pixel(x as u32, y as u32);
            if is_foreground_pixel(px, has_alpha, bg_rgb) {
                col_count += 1;
            }
        }
        if col_count > 0 {
            let weight = (col_count * col_count) as f64;
            weighted_sum_x += (x as f64) * weight;
            total_weight += weight;
        }
    }

    let bbox_center_x = (min_x + max_x) as f32 / 2.0;
    let core_center_x = if total_weight > 0.0 {
        (weighted_sum_x / total_weight) as f32
    } else {
        bbox_center_x
    };
    let center_x = core_center_x * 0.8 + bbox_center_x * 0.2;

    PivotBox {
        min_x,
        min_y,
        max_x,
        max_y,
        center_x,
        has_content: true,
    }
}

/// Slice a raw AI-generated sprite sheet using 8-connected island clustering and re-align every frame's
/// horizontal body center (`center_x`) and foot baseline (`max_y`) onto a strictly uniform `cols × rows` grid!
/// Returns `(aligned_sheet, individual_aligned_frames)`.
pub fn align_spritesheet_uniform_grid(
    source: &RgbaImage,
    rows: u32,
    cols: u32,
    baseline_ratio: f32,
) -> (RgbaImage, Vec<RgbaImage>) {
    align_spritesheet_uniform_grid_with_options(source, rows, cols, baseline_ratio, false)
}

/// Extended version of `align_spritesheet_uniform_grid` that supports `preserve_vertical_arc`:
/// when `preserve_vertical_arc` is `true` (e.g. jump / airborne animations), horizontal body centers are still
/// strictly aligned to `50%` cell width, while vertical airborne elevation differences relative to the grounded
/// take-off/landing baseline are preserved instead of slamming mid-air frames onto the ground!
pub fn align_spritesheet_uniform_grid_with_options(
    source: &RgbaImage,
    rows: u32,
    cols: u32,
    baseline_ratio: f32,
    preserve_vertical_arc: bool,
) -> (RgbaImage, Vec<RgbaImage>) {
    let rows = rows.clamp(1, 8);
    let cols = cols.clamp(1, 12);
    let sheet_w = source.width();
    let sheet_h = source.height();
    let base_cell_w = (sheet_w / cols).max(16);
    let base_cell_h = (sheet_h / rows).max(16);

    let (has_alpha, bg_rgb) = sample_corner_bg(source);
    let bg_fill = if has_alpha {
        Rgba([0, 0, 0, 0])
    } else {
        Rgba([bg_rgb.0 as u8, bg_rgb.1 as u8, bg_rgb.2 as u8, 255])
    };

    let mut working = source.clone();
    strip_grid_border_lines(&mut working, rows, cols, has_alpha, bg_rgb);

    let total_cells = (rows * cols) as usize;
    let total_pixels = (sheet_w * sheet_h) as usize;
    let mut labels = vec![-1i32; total_pixels];

    struct Island {
        pixels: Vec<u32>,
        sum_x: u64,
        sum_y: u64,
        min_x: u32,
        min_y: u32,
        max_x: u32,
        max_y: u32,
        owner_cell: usize,
        is_oversized: bool,
    }

    let mut islands: Vec<Island> = Vec::new();
    let mut queue: Vec<u32> = Vec::with_capacity(4096);

    for p in 0..total_pixels {
        if labels[p] != -1 {
            continue;
        }
        let py = (p as u32) / sheet_w;
        let px = (p as u32) - py * sheet_w;
        if !is_foreground_pixel(*working.get_pixel(px, py), has_alpha, bg_rgb) {
            continue;
        }

        let id = islands.len() as i32;
        labels[p] = id;
        queue.clear();
        queue.push(p as u32);

        let mut head = 0usize;
        let mut min_x = sheet_w;
        let mut min_y = sheet_h;
        let mut max_x = 0u32;
        let mut max_y = 0u32;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut max_alpha = 0u8;
        let mut max_chroma = 0i32;

        while head < queue.len() {
            let curr = queue[head];
            head += 1;
            let cy = curr / sheet_w;
            let cx = curr - cy * sheet_w;
            let cpx = *working.get_pixel(cx, cy);
            if cpx[3] > max_alpha {
                max_alpha = cpx[3];
            }
            let ch = pixel_chroma(cpx);
            if ch > max_chroma {
                max_chroma = ch;
            }

            if cx < min_x {
                min_x = cx;
            }
            if cx > max_x {
                max_x = cx;
            }
            if cy < min_y {
                min_y = cy;
            }
            if cy > max_y {
                max_y = cy;
            }
            sum_x += cx as u64;
            sum_y += cy as u64;

            // Use 2px bridge radius within the same nominal row to keep thin sword blades / spears
            // connected across 1px matting gaps without jumping across row boundaries
            let y0 = cy.saturating_sub(2);
            let y1 = (cy + 2).min(sheet_h - 1);
            let x0 = cx.saturating_sub(2);
            let x1 = (cx + 2).min(sheet_w - 1);
            let cur_row = cy / base_cell_h;
            for ny in y0..=y1 {
                if (ny / base_cell_h) != cur_row && (ny as i32 - cy as i32).abs() > 1 {
                    continue;
                }
                let row_off = ny * sheet_w;
                for nx in x0..=x1 {
                    let np = (row_off + nx) as usize;
                    if labels[np] == -1
                        && is_foreground_pixel(*working.get_pixel(nx, ny), has_alpha, bg_rgb)
                    {
                        labels[np] = id;
                        queue.push(np as u32);
                    }
                }
            }
        }

        let keep_small_vfx = (queue.len() >= 4 && (max_alpha >= 42 || max_chroma >= 30))
            || (queue.len() >= 2 && max_chroma >= 48 && max_alpha >= 14);
        if queue.len() < 8 && !keep_small_vfx {
            continue;
        }

        let mean_x = (sum_x as f32) / (queue.len() as f32);
        let mean_y = (sum_y as f32) / (queue.len() as f32);
        let c = ((mean_x / base_cell_w as f32).floor() as i32).clamp(0, cols as i32 - 1) as u32;
        let r = ((mean_y / base_cell_h as f32).floor() as i32).clamp(0, rows as i32 - 1) as u32;
        let bw = max_x - min_x + 1;
        let bh = max_y - min_y + 1;
        let is_oversized = bw as f32 > base_cell_w as f32 * 1.45
            || bh as f32 > base_cell_h as f32 * 1.45;

        islands.push(Island {
            pixels: queue.clone(),
            sum_x,
            sum_y,
            min_x,
            min_y,
            max_x,
            max_y,
            owner_cell: (r * cols + c) as usize,
            is_oversized,
        });
    }

    // Rank-order primary character islands if the model drew `total_cells` distinct figures with uneven margins
    let max_isl_area = islands
        .iter()
        .filter(|isl| !isl.is_oversized)
        .map(|isl| isl.pixels.len())
        .max()
        .unwrap_or(0);
    let primary_thresh = (max_isl_area * 15 / 100).max(32);
    let mut primary_indices: Vec<usize> = islands
        .iter()
        .enumerate()
        .filter(|(_, isl)| !isl.is_oversized && isl.pixels.len() >= primary_thresh)
        .map(|(i, _)| i)
        .collect();

    if primary_indices.len() == total_cells && total_cells > 1 {
        let min_my = primary_indices
            .iter()
            .map(|&i| islands[i].sum_y as f32 / islands[i].pixels.len() as f32)
            .fold(f32::INFINITY, f32::min);
        let max_my = primary_indices
            .iter()
            .map(|&i| islands[i].sum_y as f32 / islands[i].pixels.len() as f32)
            .fold(f32::NEG_INFINITY, f32::max);

        if rows == 1 || (max_my - min_my) < (sheet_h as f32 * 0.22) {
            // Single horizontal row of `total_cells` characters: sort strictly left-to-right by mean_x
            primary_indices.sort_by(|&a, &b| {
                let ax = islands[a].sum_x as f32 / islands[a].pixels.len() as f32;
                let bx = islands[b].sum_x as f32 / islands[b].pixels.len() as f32;
                ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
            });
            for (cell_idx, &isl_idx) in primary_indices.iter().enumerate() {
                islands[isl_idx].owner_cell = cell_idx;
            }
        } else {
            // Multi-row layout: sort by mean_y into `rows` row-bands of `cols` characters, then sort each row by mean_x
            primary_indices.sort_by(|&a, &b| {
                let ay = islands[a].sum_y as f32 / islands[a].pixels.len() as f32;
                let by = islands[b].sum_y as f32 / islands[b].pixels.len() as f32;
                ay.partial_cmp(&by).unwrap_or(std::cmp::Ordering::Equal)
            });
            for r in 0..rows as usize {
                let start = r * (cols as usize);
                let end = ((r + 1) * (cols as usize)).min(primary_indices.len());
                let row_slice = &mut primary_indices[start..end];
                row_slice.sort_by(|&a, &b| {
                    let ax = islands[a].sum_x as f32 / islands[a].pixels.len() as f32;
                    let bx = islands[b].sum_x as f32 / islands[b].pixels.len() as f32;
                    ax.partial_cmp(&bx).unwrap_or(std::cmp::Ordering::Equal)
                });
                for (c, &isl_idx) in row_slice.iter().enumerate() {
                    islands[isl_idx].owner_cell = r * (cols as usize) + c;
                }
            }
        }
    }

    // Attach small detached weapon tips / slash arcs / VFX islands (`pixels.len() < primary_thresh`)
    // by MINIMUM FOREGROUND PIXEL DISTANCE to each primary character island (instead of centroid distance!),
    // preventing an extended sword tip in Frame N from being sucked into Frame N+1's body center.
    if !primary_indices.is_empty() {
        let primary_snapshots: Vec<(usize, usize, u32, u32, u32, u32, Vec<(i32, i32)>)> =
            primary_indices
                .iter()
                .map(|&p_idx| {
                    let p_isl = &islands[p_idx];
                    let step = (p_isl.pixels.len() / 600).max(1);
                    let pts: Vec<(i32, i32)> = p_isl
                        .pixels
                        .iter()
                        .step_by(step)
                        .map(|&p| ((p % sheet_w) as i32, (p / sheet_w) as i32))
                        .collect();
                    (
                        p_idx,
                        p_isl.owner_cell,
                        p_isl.min_x,
                        p_isl.min_y,
                        p_isl.max_x,
                        p_isl.max_y,
                        pts,
                    )
                })
                .collect();

        for i in 0..islands.len() {
            if islands[i].is_oversized || islands[i].pixels.len() >= primary_thresh {
                continue;
            }
            let step_s = (islands[i].pixels.len() / 64).max(1);
            let small_pts: Vec<(i32, i32)> = islands[i]
                .pixels
                .iter()
                .step_by(step_s)
                .map(|&p| ((p % sheet_w) as i32, (p / sheet_w) as i32))
                .collect();
            let orig_cell = islands[i].owner_cell;
            let orig_row = (orig_cell as u32) / cols;

            let mut best_cell = orig_cell;
            let mut best_score = f32::INFINITY;

            for &(_, p_cell, _, _, _, _, ref p_pts) in &primary_snapshots {
                let mut min_d2 = i64::MAX;
                for &(sx, sy) in &small_pts {
                    for &(px, py) in p_pts {
                        let dx = (sx - px) as i64;
                        let dy = (sy - py) as i64;
                        let d2 = dx * dx + dy * dy;
                        if d2 < min_d2 {
                            min_d2 = d2;
                        }
                    }
                }
                let mut dist = (min_d2 as f32).sqrt();
                let p_row = (p_cell as u32) / cols;
                if p_row != orig_row {
                    dist += base_cell_h as f32 * 0.85;
                }
                if p_cell == orig_cell {
                    dist *= 0.85;
                }
                if dist < best_score {
                    best_score = dist;
                    best_cell = p_cell;
                }
            }
            islands[i].owner_cell = best_cell;
        }
    }

    // Collect pixels per cell and extract each cell's full bounding-box canvas without clipping
    let mut cell_pixel_lists: Vec<Vec<u32>> = vec![Vec::new(); total_cells];
    for isl in &islands {
        if !isl.is_oversized {
            cell_pixel_lists[isl.owner_cell].extend_from_slice(&isl.pixels);
        } else {
            for &p in &isl.pixels {
                let py = p / sheet_w;
                let px = p % sheet_w;
                let c = (px / base_cell_w).min(cols - 1);
                let r = (py / base_cell_h).min(rows - 1);
                let cell_idx = (r * cols + c) as usize;
                cell_pixel_lists[cell_idx].push(p);
            }
        }
    }

    let mut cell_canvases: Vec<RgbaImage> = Vec::with_capacity(total_cells);
    let mut cell_local_foot_y: Vec<i32> = Vec::with_capacity(total_cells);
    for cell_idx in 0..total_cells {
        let r = (cell_idx as u32) / cols;
        let plist = &cell_pixel_lists[cell_idx];
        if plist.is_empty() {
            cell_canvases.push(RgbaImage::from_pixel(base_cell_w, base_cell_h, bg_fill));
            cell_local_foot_y.push((base_cell_h as f32 * baseline_ratio).round() as i32);
            continue;
        }
        let mut c_min_x = sheet_w;
        let mut c_min_y = sheet_h;
        let mut c_max_x = 0u32;
        let mut c_max_y = 0u32;
        for &p in plist {
            let py = p / sheet_w;
            let px = p % sheet_w;
            if px < c_min_x {
                c_min_x = px;
            }
            if px > c_max_x {
                c_max_x = px;
            }
            if py < c_min_y {
                c_min_y = py;
            }
            if py > c_max_y {
                c_max_y = py;
            }
        }
        cell_local_foot_y.push((c_max_y as i32) - (r * base_cell_h) as i32);

        let cw = (c_max_x - c_min_x + 1 + 8).max(16);
        let ch = (c_max_y - c_min_y + 1 + 8).max(16);
        let mut cv = RgbaImage::from_pixel(cw, ch, bg_fill);
        for &p in plist {
            let py = p / sheet_w;
            let px = p % sheet_w;
            let dx = px - c_min_x + 4;
            let dy = py - c_min_y + 4;
            cv.put_pixel(dx, dy, *working.get_pixel(px, py));
        }
        cell_canvases.push(cv);
    }

    let pivots: Vec<PivotBox> = cell_canvases
        .iter()
        .map(|cv| compute_character_foot_pivot(cv, has_alpha, bg_rgb))
        .collect();

    // Compute per-frame vertical airborne elevation above ground baseline when `preserve_vertical_arc` is active
    let ground_local_y = cell_local_foot_y
        .iter()
        .copied()
        .max()
        .unwrap_or((base_cell_h as f32 * baseline_ratio).round() as i32);

    let frame_elevations: Vec<i32> = (0..total_cells)
        .map(|idx| {
            if !preserve_vertical_arc || total_cells <= 2 || idx == 0 || idx + 1 == total_cells {
                0
            } else {
                let raw_elev = (ground_local_y - cell_local_foot_y[idx]).max(0);
                let phase = (idx as f32) / ((total_cells - 1) as f32);
                let arc_sin = (phase * std::f32::consts::PI).sin();
                let min_arc_elev = (base_cell_h as f32 * 0.16 * arc_sin).round() as i32;
                raw_elev
                    .max(min_arc_elev)
                    .min((base_cell_h as f32 * 0.42).round() as i32)
            }
        })
        .collect();

    // Compute uniform anti-clip cell dimensions `(out_w, out_h)`
    let safety_margin = 16.0f32;
    let mut max_half_span_x = base_cell_w as f32 / 2.0;
    let mut max_top_span_y = base_cell_h as f32 * baseline_ratio;
    let max_bot_span_y = base_cell_h as f32 * (1.0 - baseline_ratio);

    for (idx, p) in pivots.iter().enumerate() {
        if !p.has_content {
            continue;
        }
        let left_reach = p.center_x - p.min_x as f32 + safety_margin;
        let right_reach = p.max_x as f32 - p.center_x + safety_margin;
        let top_reach =
            (p.max_y - p.min_y + frame_elevations[idx]) as f32 + safety_margin;
        max_half_span_x = max_half_span_x.max(left_reach).max(right_reach);
        max_top_span_y = max_top_span_y.max(top_reach);
    }

    let needed_w = (max_half_span_x * 2.0).ceil() as u32;
    let needed_h_top = (max_top_span_y / baseline_ratio.max(0.5)).ceil() as u32;
    let needed_h_bot = (max_bot_span_y / (1.0 - baseline_ratio).max(0.08)).ceil() as u32;
    let needed_h = needed_h_top.max(needed_h_bot);

    let out_w = base_cell_w.max(needed_w + (needed_w % 2));
    let out_h = base_cell_h.max(needed_h + (needed_h % 2));

    let target_center_x = (out_w as f32 / 2.0).round() as i32;
    let target_baseline_y = (out_h as f32 * baseline_ratio).round() as i32;

    let mut aligned_frames: Vec<RgbaImage> = Vec::with_capacity(total_cells);
    let mut aligned_sheet = RgbaImage::from_pixel(out_w * cols, out_h * rows, bg_fill);

    for (idx, (raw_cell, pivot)) in cell_canvases.into_iter().zip(pivots.into_iter()).enumerate() {
        let mut frame_cv = RgbaImage::from_pixel(out_w, out_h, bg_fill);
        if pivot.has_content {
            let shift_x = target_center_x - (pivot.center_x.round() as i32);
            let shift_y = (target_baseline_y - frame_elevations[idx]) - pivot.max_y;

            for sy in 0..raw_cell.height() {
                let dy = sy as i32 + shift_y;
                if dy < 0 || dy >= out_h as i32 {
                    continue;
                }
                for sx in 0..raw_cell.width() {
                    let dx = sx as i32 + shift_x;
                    if dx < 0 || dx >= out_w as i32 {
                        continue;
                    }
                    let px = *raw_cell.get_pixel(sx, sy);
                    if is_foreground_pixel(px, has_alpha, bg_rgb) {
                        frame_cv.put_pixel(dx as u32, dy as u32, px);
                    }
                }
            }
        }

        let r = (idx as u32) / cols;
        let c = (idx as u32) % cols;
        let sheet_ox = c * out_w;
        let sheet_oy = r * out_h;
        for fy in 0..out_h {
            for fx in 0..out_w {
                aligned_sheet.put_pixel(sheet_ox + fx, sheet_oy + fy, *frame_cv.get_pixel(fx, fy));
            }
        }
        aligned_frames.push(frame_cv);
    }

    (aligned_sheet, aligned_frames)
}

/// Detect the fundamental pixel block size (`block_size` in `3..=16`) of an AI-generated pixel-art image
/// using a Run-Length Edge Span Histogram across horizontal and vertical color/alpha transitions.
pub fn detect_pixel_block_size(img: &RgbaImage) -> u32 {
    let w = img.width();
    let h = img.height();
    if w < 128 || h < 128 {
        return 1;
    }

    let x0 = (w as f32 * 0.06) as u32;
    let x1 = (w as f32 * 0.94) as u32;
    let y0 = (h as f32 * 0.06) as u32;
    let y1 = (h as f32 * 0.94) as u32;

    let mut span_hist = [0.0f64; 48];

    // Horizontal edge spans
    for y in (y0..y1).step_by(2) {
        let mut last_edge_x: Option<u32> = None;
        for x in (x0 + 1)..x1 {
            let p0 = img.get_pixel(x - 1, y);
            let p1 = img.get_pixel(x, y);
            let a_diff = (p1[3] as i32 - p0[3] as i32).abs();
            let diff = if a_diff > 96 {
                a_diff * 2
            } else if p0[3] >= 48 && p1[3] >= 48 {
                (p1[0] as i32 - p0[0] as i32).abs()
                    + (p1[1] as i32 - p0[1] as i32).abs()
                    + (p1[2] as i32 - p0[2] as i32).abs()
            } else {
                0
            };

            if diff > 42 {
                if let Some(prev) = last_edge_x {
                    let span = (x - prev) as usize;
                    if (3..44).contains(&span) {
                        span_hist[span] += diff as f64;
                    }
                }
                last_edge_x = Some(x);
            }
        }
    }

    // Vertical edge spans
    for x in (x0..x1).step_by(2) {
        let mut last_edge_y: Option<u32> = None;
        for y in (y0 + 1)..y1 {
            let p0 = img.get_pixel(x, y - 1);
            let p1 = img.get_pixel(x, y);
            let a_diff = (p1[3] as i32 - p0[3] as i32).abs();
            let diff = if a_diff > 96 {
                a_diff * 2
            } else if p0[3] >= 48 && p1[3] >= 48 {
                (p1[0] as i32 - p0[0] as i32).abs()
                    + (p1[1] as i32 - p0[1] as i32).abs()
                    + (p1[2] as i32 - p0[2] as i32).abs()
            } else {
                0
            };

            if diff > 42 {
                if let Some(prev) = last_edge_y {
                    let span = (y - prev) as usize;
                    if (3..44).contains(&span) {
                        span_hist[span] += diff as f64;
                    }
                }
                last_edge_y = Some(y);
            }
        }
    }

    let mut best_step = 8u32;
    let mut best_score = -1.0f64;
    for s in 4usize..=16usize {
        let wl = span_hist[s - 1];
        let wc = span_hist[s];
        let wr = span_hist[s + 1];
        let mut score = wl * 0.78 + wc + wr * 0.78;
        if s * 2 + 1 < 48 {
            let h2 = span_hist[s * 2 - 1] * 0.6 + span_hist[s * 2] + span_hist[s * 2 + 1] * 0.6;
            score += 0.35 * h2;
        }
        if score > best_score {
            best_score = score;
            best_step = s as u32;
        }
    }

    if best_score <= 0.0 {
        (w.min(h) / 128).clamp(4, 12)
    } else {
        best_step.clamp(3, 16)
    }
}

/// Pick the weighted medoid / mode RGB color from `(r, g, b, weight)` samples.
/// Preserves crisp 1px dark outlines and prevents muddy averaged colors at edges.
fn pick_weighted_medoid_rgb(samples: &[(u8, u8, u8, u32)]) -> (u8, u8, u8) {
    let mut best_rgb = (samples[0].0, samples[0].1, samples[0].2);
    let mut best_cost = i64::MAX;

    for &(r1, g1, b1, _) in samples {
        let mut cost = 0i64;
        for &(r2, g2, b2, w2) in samples {
            let dist = (r1 as i64 - r2 as i64).abs()
                + (g1 as i64 - g2 as i64).abs()
                + (b1 as i64 - b2 as i64).abs();
            // Quantize similar colors (dist <= 18) as 0 cost so dominant flat cluster wins
            let eff_dist = if dist <= 18 { 0 } else { dist };
            cost += eff_dist * (w2 as i64);
        }
        if cost < best_cost {
            best_cost = cost;
            best_rgb = (r1, g1, b1);
        }
    }
    best_rgb
}

/// Convert high-res AI "fake pixel art" (mixels / blurry anti-aliased multi-pixel blocks) into
/// a strict integer pixel grid:
/// 1. Center-weighted medoid block sampling onto a true 1:1 logical pixel canvas `(gw, gh)`
/// 2. Hard alpha thresholding (`alpha >= 115 -> 255`) for neutral fringes, while smoothly preserving
///    vibrant high-chroma translucent glow / slash VFX (`alpha >= 14`, `chroma >= 30`)
/// 3. Local color coalescing so flat pixel blocks share crisp uniform RGB values
/// 4. Optional exact integer Nearest-Neighbor upscale (`upscale_back: true` keeps exact `block_size × block_size` squares)
/// Returns `(quantized_image, native_1x_image, detected_block_size)`.
pub fn quantize_pixel_art_grid(
    img: &RgbaImage,
    explicit_block_size: Option<u32>,
    upscale_back: bool,
) -> (RgbaImage, RgbaImage, u32) {
    let w = img.width();
    let h = img.height();
    let block = explicit_block_size
        .filter(|&b| (2..=32).contains(&b))
        .unwrap_or_else(|| detect_pixel_block_size(img));

    if block <= 1 || w < block * 8 || h < block * 8 {
        return (img.clone(), img.clone(), 1);
    }

    let gw = (w / block).max(8);
    let gh = (h / block).max(8);
    let mut native = RgbaImage::new(gw, gh);

    for gy in 0..gh {
        let sy0 = gy * block;
        let sy1 = ((gy + 1) * block).min(h);
        let bh = sy1 - sy0;

        for gx in 0..gw {
            let sx0 = gx * block;
            let sx1 = ((gx + 1) * block).min(w);
            let bw = sx1 - sx0;

            let mut opaque_weight = 0u32;
            let mut total_weight = 0u32;
            let mut samples: Vec<(u8, u8, u8, u32)> = Vec::with_capacity((bw * bh) as usize);
            let mut glow_count = 0u32;
            let mut glow_alpha_sum = 0u32;
            let mut glow_samples: Vec<(u8, u8, u8, u32)> = Vec::new();

            for dy in 0..bh {
                let is_center_y = bh <= 2 || (dy > 0 && dy + 1 < bh);
                for dx in 0..bw {
                    let is_center_x = bw <= 2 || (dx > 0 && dx + 1 < bw);
                    let w_px = if is_center_x && is_center_y { 3 } else { 1 };
                    total_weight += w_px;

                    let px = *img.get_pixel(sx0 + dx, sy0 + dy);
                    if px[3] >= 115 {
                        opaque_weight += w_px;
                        samples.push((px[0], px[1], px[2], w_px));
                    }
                    if px[3] >= 14 && pixel_chroma(px) >= 30 {
                        glow_count += 1;
                        glow_alpha_sum += (px[3] as u32) * w_px;
                        glow_samples.push((px[0], px[1], px[2], w_px));
                    }
                }
            }

            if opaque_weight * 100 >= total_weight * 42 && !samples.is_empty() {
                let best_rgb = pick_weighted_medoid_rgb(&samples);
                native.put_pixel(gx, gy, Rgba([best_rgb.0, best_rgb.1, best_rgb.2, 255]));
            } else {
                // Preserve thin bright/translucent chromatic slash glow while clipping low-chroma AA fringe
                let avg_block_alpha = glow_alpha_sum / total_weight.max(1);
                if glow_count >= 2 && avg_block_alpha >= 8 && !glow_samples.is_empty() {
                    let glow_rgb = pick_weighted_medoid_rgb(&glow_samples);
                    let out_alpha = avg_block_alpha.min(254) as u8;
                    native.put_pixel(gx, gy, Rgba([glow_rgb.0, glow_rgb.1, glow_rgb.2, out_alpha]));
                } else {
                    native.put_pixel(gx, gy, Rgba([0, 0, 0, 0]));
                }
            }
        }
    }

    // Despeckle single isolated 1px color jitter on solid flat regions (leave translucent glow pixels intact)
    let prev_native = native.clone();
    for y in 1..(gh.saturating_sub(1)) {
        for x in 1..(gw.saturating_sub(1)) {
            let c = *prev_native.get_pixel(x, y);
            if c[3] < 255 {
                continue;
            }
            let n_up = *prev_native.get_pixel(x, y - 1);
            let n_dn = *prev_native.get_pixel(x, y + 1);
            let n_lt = *prev_native.get_pixel(x - 1, y);
            let n_rt = *prev_native.get_pixel(x + 1, y);

            // If a pixel is slightly different (< 24 L1 diff) from a neighbor, snap it to reduce sub-palette noise
            for nb in [n_up, n_dn, n_lt, n_rt] {
                if nb[3] == 255 {
                    let diff = (c[0] as i32 - nb[0] as i32).abs()
                        + (c[1] as i32 - nb[1] as i32).abs()
                        + (c[2] as i32 - nb[2] as i32).abs();
                    if diff > 0 && diff <= 20 {
                        native.put_pixel(x, y, nb);
                        break;
                    }
                }
            }
        }
    }

    if !upscale_back {
        return (native.clone(), native, block);
    }

    // Integer Nearest-Neighbor upscale back to exact `gw * block` x `gh * block`
    let up_w = gw * block;
    let up_h = gh * block;
    let mut upscaled = RgbaImage::new(up_w, up_h);
    for gy in 0..gh {
        for gx in 0..gw {
            let px = *native.get_pixel(gx, gy);
            let ox = gx * block;
            let oy = gy * block;
            for dy in 0..block {
                for dx in 0..block {
                    upscaled.put_pixel(ox + dx, oy + dy, px);
                }
            }
        }
    }

    (upscaled, native, block)
}

/// Quantize each aligned sprite sheet frame using a single unified `block_size` and re-stitch both
/// the integer-upscaled sprite sheet and the native 1x pixel-art sprite sheet.
/// Guarantees that `sheet_w == frame_w * cols`, `sheet_h == frame_h * rows`, and every pixel block is an exact square.
pub fn quantize_spritesheet_frames(
    aligned_sheet: &RgbaImage,
    frames: &[RgbaImage],
    rows: u32,
    cols: u32,
    explicit_block_size: Option<u32>,
) -> (RgbaImage, RgbaImage, Vec<RgbaImage>, u32) {
    let block = explicit_block_size
        .filter(|&b| (2..=32).contains(&b))
        .unwrap_or_else(|| detect_pixel_block_size(aligned_sheet));

    if frames.is_empty() || block <= 1 {
        return (
            aligned_sheet.clone(),
            aligned_sheet.clone(),
            frames.to_vec(),
            1,
        );
    }

    let mut q_frames = Vec::with_capacity(frames.len());
    let mut n_frames = Vec::with_capacity(frames.len());
    for f in frames {
        let (qf, nf, _) = quantize_pixel_art_grid(f, Some(block), true);
        q_frames.push(qf);
        n_frames.push(nf);
    }

    let fw = q_frames[0].width();
    let fh = q_frames[0].height();
    let nw = n_frames[0].width();
    let nh = n_frames[0].height();

    let mut q_sheet = RgbaImage::from_pixel(fw * cols, fh * rows, Rgba([0, 0, 0, 0]));
    let mut n_sheet = RgbaImage::from_pixel(nw * cols, nh * rows, Rgba([0, 0, 0, 0]));

    for (idx, (qf, nf)) in q_frames.iter().zip(n_frames.iter()).enumerate() {
        let r = (idx as u32) / cols;
        let c = (idx as u32) % cols;
        if r >= rows {
            break;
        }
        image::imageops::overlay(&mut q_sheet, qf, (c * fw) as i64, (r * fh) as i64);
        image::imageops::overlay(&mut n_sheet, nf, (c * nw) as i64, (r * nh) as i64);
    }

    (q_sheet, n_sheet, q_frames, block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_spritesheet_and_pixel_quantize() {
        // Create a 2x3 sprite sheet (600x400) where each frame has a character slightly off-center and at different heights
        let mut raw_sheet = RgbaImage::from_pixel(600, 400, Rgba([0, 0, 0, 0]));
        let offsets = [(15, -10), (-12, 8), (20, 5), (-8, -6), (10, 12), (-15, 0)];
        for (idx, (dx, dy)) in offsets.iter().enumerate() {
            let r = (idx as u32) / 3;
            let c = (idx as u32) % 3;
            let cx = (c * 200 + 100) as i32 + dx;
            let foot_y = (r * 200 + 170) as i32 + dy;
            for y in (foot_y - 60)..=foot_y {
                for x in (cx - 20)..=(cx + 20) {
                    raw_sheet.put_pixel(x as u32, y as u32, Rgba([220, 80, 60, 255]));
                }
            }
        }

        let (aligned, frames) = align_spritesheet_uniform_grid(&raw_sheet, 2, 3, 0.86);
        assert_eq!(frames.len(), 6);
        assert_eq!(aligned.width(), frames[0].width() * 3);
        assert_eq!(aligned.height(), frames[0].height() * 2);

        // Verify all 6 aligned frames now have identical foot baseline max_y and center_x!
        let p0 = compute_character_foot_pivot(&frames[0], true, (0, 0, 0));
        for f in &frames[1..] {
            let pf = compute_character_foot_pivot(f, true, (0, 0, 0));
            assert_eq!(pf.max_y, p0.max_y, "Foot baseline must be strictly identical across all frames");
            assert!((pf.center_x - p0.center_x).abs() <= 1.0, "Horizontal pivot must be aligned");
        }

        // Test pixel quantization on 600x400 sheet with block_size = 4
        let (quant, native, blk) = quantize_pixel_art_grid(&aligned, Some(4), true);
        assert_eq!(blk, 4);
        assert_eq!(quant.width(), native.width() * 4);
        assert_eq!(quant.height(), native.height() * 4);
    }

    #[test]
    fn test_detached_sword_tip_and_jump_arc_and_prompt_preservation() {
        // 1. Prompt preservation + death non-looping + 4-frame jump landing closure
        let death_prompt =
            build_keyframe_choreography_prompt("手持银蓝长剑、面向右侧的32-bit像素小人，死亡倒地", 2, 3);
        assert!(death_prompt.contains("手持银蓝长剑、面向右侧的32-bit像素小人，死亡倒地"));
        assert!(death_prompt.contains("FINAL NON-LOOPING"));

        let jump4_prompt = build_keyframe_choreography_prompt("跳跃 2x2", 2, 2);
        assert!(jump4_prompt.contains("Frame #4 (Row 2, Col 2): Landing Impact Cushion"));

        // 2. Detached sword tip in Frame 4 (Row 1, Col 0 in 2x3 on 600x400 sheet):
        // Frame 4 character body is at x=60..90, sword blade extends to x=165, and detached sword tip is at x=170..180
        // (separated by 4px gap, and closer to Frame 5's centroid x=280 than to Frame 4's body centroid x=75 if measured by centroid,
        // or even crossing slightly to x=195..204!).
        let mut slash_sheet = RgbaImage::from_pixel(600, 400, Rgba([0, 0, 0, 0]));
        for idx in 0..6 {
            let r = (idx as u32) / 3;
            let c = (idx as u32) % 3;
            let cx = (c * 200 + 80) as i32;
            let foot_y = (r * 200 + 170) as i32;
            for y in (foot_y - 60)..=foot_y {
                for x in (cx - 15)..=(cx + 15) {
                    slash_sheet.put_pixel(x as u32, y as u32, Rgba([60, 120, 220, 255]));
                }
            }
        }
        // In Frame 4 (idx=3, r=1, c=0), draw extended sword connected to body from x=96..182, y=330..334
        for y in 330..=334 {
            for x in 96..=182 {
                slash_sheet.put_pixel(x, y, Rgba([200, 230, 255, 255]));
            }
            // Detached sword tip separated by 4px gap (x=187..198, y=330..334)
            for x in 187..=198 {
                slash_sheet.put_pixel(x, y, Rgba([255, 50, 50, 255]));
            }
        }

        let (_, slash_frames) = align_spritesheet_uniform_grid(&slash_sheet, 2, 3, 0.86);
        // Count red sword-tip pixels ([255, 50, 50, 255]) in Frame 4 (slash_frames[3]) vs Frame 5 (slash_frames[4])
        let red_in_f4 = slash_frames[3]
            .pixels()
            .filter(|p| p.0 == [255, 50, 50, 255])
            .count();
        let red_in_f5 = slash_frames[4]
            .pixels()
            .filter(|p| p.0 == [255, 50, 50, 255])
            .count();
        assert_eq!(red_in_f4, 5 * 12, "Detached sword tip must stay in Frame 4");
        assert_eq!(red_in_f5, 0, "Detached sword tip must NEVER leak into Frame 5");

        // 3. Jump vertical arc preservation on 2x2 (4 frames)
        let mut jump_sheet = RgbaImage::from_pixel(400, 400, Rgba([0, 0, 0, 0]));
        // Frame 0 (ground crouch), Frame 1 (airborne high), Frame 2 (apex highest), Frame 3 (ground landing)
        let local_foot_ys = [175i32, 130i32, 115i32, 175i32];
        for (idx, &lfy) in local_foot_ys.iter().enumerate() {
            let r = (idx as u32) / 2;
            let c = (idx as u32) % 2;
            let cx = (c * 200 + 100) as i32;
            let foot_y = (r * 200) as i32 + lfy;
            for y in (foot_y - 50)..=foot_y {
                for x in (cx - 16)..=(cx + 16) {
                    jump_sheet.put_pixel(x as u32, y as u32, Rgba([100, 220, 120, 255]));
                }
            }
        }
        let (_, jump_frames) =
            align_spritesheet_uniform_grid_with_options(&jump_sheet, 2, 2, 0.86, true);
        let jp0 = compute_character_foot_pivot(&jump_frames[0], true, (0, 0, 0));
        let jp1 = compute_character_foot_pivot(&jump_frames[1], true, (0, 0, 0));
        let jp2 = compute_character_foot_pivot(&jump_frames[2], true, (0, 0, 0));
        let jp3 = compute_character_foot_pivot(&jump_frames[3], true, (0, 0, 0));
        assert_eq!(jp0.max_y, jp3.max_y, "Take-off and landing frames share ground baseline");
        assert!(
            jp1.max_y < jp0.max_y - 30 && jp2.max_y < jp1.max_y,
            "Airborne jump frames must preserve their vertical elevation above ground baseline"
        );
    }

    #[test]
    fn test_word_boundary_no_false_action_triggers() {
        // "white" must NOT match "hit" -> should classify as walk/run, not hurt
        let white_run = build_keyframe_choreography_prompt("white background running sprite", 2, 3);
        assert!(
            white_run.contains("Full Stride Contact A"),
            "running prompt with 'white' must trigger walk/run choreography"
        );
        assert!(
            !white_run.contains("Initial Hit Impact"),
            "'white' must NEVER falsely trigger hurt ('hit')"
        );

        // "soldier" / "medieval" must NOT match "die", "standard" must NOT match "stand"
        let soldier_prompt = build_keyframe_choreography_prompt(
            "soldier with shield in standard medieval armor",
            2,
            3,
        );
        assert!(
            soldier_prompt.contains("Initial Pose / Preparation (0% phase)"),
            "neutral equipment description should stay generic"
        );
        assert!(
            !soldier_prompt.contains("NON-LOOPING COLLAPSE TO GROUND"),
            "'soldier'/'medieval' must NEVER trigger death ('die')"
        );
        assert!(
            !soldier_prompt.contains("Neutral Upright Stance"),
            "'standard' must NEVER trigger idle ('stand')"
        );

        // "workshop" must NOT match "hop", "rune" must NOT match "run" -> triggers attack/slash only
        assert!(!is_airborne_motion("workshop rune slash"));
        let rune_slash = build_keyframe_choreography_prompt("workshop rune slash", 2, 3);
        assert!(
            rune_slash.contains("Combat Ready Stance"),
            "'slash' must trigger attack choreography"
        );
        assert!(
            !rune_slash.contains("GROUND VS AIRBORNE VERTICAL ARC"),
            "'workshop' must NEVER trigger jump ('hop')"
        );
        assert!(
            !rune_slash.contains("Full Stride Contact A"),
            "'rune' must NEVER trigger walk/run ('run')"
        );

        // Remaining trap words: "stable" (stab), "bishop" (hop), "trunk" (run), "grunge" (run)
        assert!(!is_airborne_motion("stable bishop with trunk in grunge armor"));
        let trap_prompt = build_keyframe_choreography_prompt(
            "stable bishop with trunk in grunge armor",
            2,
            3,
        );
        assert!(trap_prompt.contains("Initial Pose / Preparation (0% phase)"));
        assert!(!trap_prompt.contains("Combat Ready Stance"));
        assert!(!trap_prompt.contains("GROUND VS AIRBORNE VERTICAL ARC"));
        assert!(!trap_prompt.contains("Full Stride Contact A"));

        // Grid inference and pixel-art word boundaries
        assert_eq!(infer_sprite_grid("12x3 sprites", None, None), (2, 4));
        assert_eq!(infer_sprite_grid("16-frame walk", None, None), (2, 4));
        assert_eq!(infer_sprite_grid("跳跃2x2", None, None), (2, 2));
        assert!(!is_pixel_art_requested("", "high pixel resolution", "smooth"));
        assert!(!is_pixel_art_requested("", "116-bit fantasy", ""));
        assert!(is_pixel_art_requested("", "16-bit pixel-art knight", ""));
    }

    #[test]
    fn test_word_boundary_inflections_and_cjk() {
        assert!(build_keyframe_choreography_prompt("knight hitting slime", 2, 3)
            .contains("Initial Hit Impact"));
        assert!(build_keyframe_choreography_prompt("boss dying on floor", 2, 3)
            .contains("NON-LOOPING COLLAPSE TO GROUND"));
        assert!(build_keyframe_choreography_prompt("dead warrior", 2, 3)
            .contains("NON-LOOPING COLLAPSE TO GROUND"));
        assert!(is_airborne_motion("frog hopping forward"));
        assert!(build_keyframe_choreography_prompt("frog hopping forward", 2, 2)
            .contains("GROUND VS AIRBORNE VERTICAL ARC"));
        assert!(build_keyframe_choreography_prompt("assassin stabbing target", 2, 3)
            .contains("Combat Ready Stance"));
        assert!(build_keyframe_choreography_prompt("ranger slashing blade", 2, 3)
            .contains("Combat Ready Stance"));
        assert!(build_keyframe_choreography_prompt("scout dashing and moving", 2, 3)
            .contains("Full Stride Contact A"));
        assert!(build_keyframe_choreography_prompt("mage standing calm", 2, 3)
            .contains("Neutral Upright Stance"));
        // CJK phrases remain intact without spaces
        assert!(build_keyframe_choreography_prompt("角色受击硬直", 2, 3)
            .contains("Initial Hit Impact"));
        assert!(build_keyframe_choreography_prompt("骑士挥砍剑气", 2, 3)
            .contains("Combat Ready Stance"));
    }

    #[test]
    fn test_glowing_cyan_slash_vfx_and_sparks_survive_alignment_and_quantize() {
        // 1. Spritesheet alignment: thin translucent cyan slash arc (alpha=22 and alpha=72)
        // plus small detached sparks (2x2=4px at alpha=24 and 3x1=3px at alpha=96) in Frame 0
        // must survive inside aligned_frames[0] without leaking into aligned_frames[1].
        let mut sheet = RgbaImage::from_pixel(600, 400, Rgba([0, 0, 0, 0]));
        for idx in 0..6 {
            let r = (idx as u32) / 3;
            let c = (idx as u32) % 3;
            let cx = (c * 200 + 80) as i32;
            let foot_y = (r * 200 + 170) as i32;
            for y in (foot_y - 60)..=foot_y {
                for x in (cx - 16)..=(cx + 16) {
                    sheet.put_pixel(x as u32, y as u32, Rgba([50, 80, 140, 255]));
                }
            }
            // Bright white armor chest/eyes inside body
            for y in (foot_y - 48)..=(foot_y - 40) {
                for x in (cx - 4)..=(cx + 4) {
                    sheet.put_pixel(x as u32, y as u32, Rgba([255, 255, 255, 255]));
                }
            }
        }

        // In Frame 0 (r=0, c=0), draw connected sword + thin translucent cyan slash arc near right edge
        for y in 124..=127 {
            for x in 97..=136 {
                sheet.put_pixel(x, y, Rgba([210, 235, 255, 255]));
            }
        }
        // Thin translucent cyan slash arc (4px thick mid-glow alpha=72, 2px faint outer glow alpha=22)
        for x in 140..=175 {
            for y in 118..=121 {
                sheet.put_pixel(x, y, Rgba([0, 225, 255, 72]));
            }
            for y in 116..=117 {
                sheet.put_pixel(x, y, Rgba([20, 230, 255, 22]));
            }
        }
        // Detached 2x2 (4px) faint cyan spark at x=180..=181, y=110..=111 (alpha=24)
        for y in 110..=111 {
            for x in 180..=181 {
                sheet.put_pixel(x, y, Rgba([0, 240, 255, 24]));
            }
        }
        // Detached 3x1 (3px) high-chroma spark at x=184..=186, y=132 (alpha=96)
        for x in 184..=186 {
            sheet.put_pixel(x, 132, Rgba([40, 235, 255, 96]));
        }

        let (aligned_sheet, aligned_frames) = align_spritesheet_uniform_grid(&sheet, 2, 3, 0.86);
        let is_cyan_glow = |p: &Rgba<u8>| p.0[3] > 0 && p.0[2] >= 200 && p.0[0] <= 60;

        let f0_cyan: usize = aligned_frames[0].pixels().filter(|p| is_cyan_glow(p)).count();
        let f1_cyan: usize = aligned_frames[1].pixels().filter(|p| is_cyan_glow(p)).count();
        let f0_faint_cyan: usize = aligned_frames[0]
            .pixels()
            .filter(|p| is_cyan_glow(p) && p.0[3] <= 28)
            .count();

        assert_eq!(
            f0_cyan,
            36 * 6 + 4 + 3,
            "All glowing cyan arc pixels and small sparks must survive alignment in Frame 0"
        );
        assert_eq!(
            f0_faint_cyan,
            36 * 2 + 4,
            "Translucent alpha 22..24 cyan glow and 4px spark must be preserved"
        );
        assert_eq!(f1_cyan, 0, "Cyan VFX from Frame 0 must NEVER leak into Frame 1");

        // Quantize spritesheet frames with block_size=4 and confirm cyan VFX stays in Frame 0 only
        let (_, _, q_frames, q_blk) =
            quantize_spritesheet_frames(&aligned_sheet, &aligned_frames, 2, 3, Some(4));
        assert_eq!(q_blk, 4);
        assert!(
            q_frames[0].pixels().any(|p| is_cyan_glow(p) && p.0[3] < 255),
            "Quantized Frame 0 must retain smooth translucent cyan glow"
        );
        assert_eq!(
            q_frames[1].pixels().filter(|p| is_cyan_glow(p)).count(),
            0,
            "Quantized Frame 1 must remain free of Frame 0 cyan VFX"
        );

        // 2. Direct quantize_pixel_art_grid check (64x64, block=4):
        // preserves opaque white armor & thin cyan glow while clipping neutral white AA fringe
        let mut patch = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 0]));
        // Opaque white armor (8x8)
        for y in 8..16 {
            for x in 8..16 {
                patch.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            }
        }
        // Neutral white anti-aliased fringe (8x8, alpha=60, chroma=0) -> must be clipped
        for y in 8..16 {
            for x in 20..28 {
                patch.put_pixel(x, y, Rgba([245, 245, 245, 60]));
            }
        }
        // Solid cyan blade (4x4 at 16..20, 32..36) next to thin 2px cyan arc (20..44, y=33..=34, alpha=72)
        // and 4x4 soft glow tip (44..48, 32..36, alpha=24)
        for y in 32..36 {
            for x in 16..20 {
                patch.put_pixel(x, y, Rgba([0, 230, 255, 255]));
            }
            for x in 44..48 {
                patch.put_pixel(x, y, Rgba([20, 230, 255, 24]));
            }
        }
        for y in 33..=34 {
            for x in 20..44 {
                patch.put_pixel(x, y, Rgba([0, 230, 255, 72]));
            }
        }

        let (_, native, blk) = quantize_pixel_art_grid(&patch, Some(4), true);
        assert_eq!(blk, 4);
        assert_eq!(*native.get_pixel(2, 2), Rgba([255, 255, 255, 255]));
        assert_eq!(
            native.get_pixel(5, 2).0[3],
            0,
            "Neutral white AA fringe below alpha 115 must stay clipped"
        );
        assert_eq!(
            *native.get_pixel(4, 8),
            Rgba([0, 230, 255, 255]),
            "Solid cyan blade must stay opaque"
        );
        let arc_px = *native.get_pixel(6, 8);
        assert!(
            arc_px.0[3] > 0 && arc_px.0[3] < 255 && arc_px.0[2] >= 220 && arc_px.0[0] == 0,
            "Thin translucent cyan arc must survive quantization smoothly: {:?}",
            arc_px
        );
        let tip_px = *native.get_pixel(11, 8);
        assert_eq!(
            tip_px,
            Rgba([20, 230, 255, 24]),
            "Soft cyan glow tip must keep its color and alpha without being overwritten by solid blade"
        );
    }
}
