use crate::error::{AppError, AppErrorCode, AppResult};
use base64::Engine;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const BONE_KEYS: [&str; 15] = [
    "hips",
    "spine",
    "chest",
    "head",
    "l_upper_arm",
    "l_fore_arm",
    "r_upper_arm",
    "r_fore_arm",
    "weapon",
    "l_thigh",
    "l_shin",
    "l_foot",
    "r_thigh",
    "r_shin",
    "r_foot",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseCameraConfig {
    pub preset: String,
    pub yaw: f32,
    pub pitch: f32,
    pub ortho: bool,
    pub zoom: f32,
}

impl Default for PoseCameraConfig {
    fn default() -> Self {
        Self {
            preset: "side_2d".to_string(),
            yaw: -90.0,
            pitch: 0.0,
            ortho: true,
            zoom: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseProportionConfig {
    pub preset: String,
    pub head_scale: f32,
    pub torso_scale: f32,
    pub limb_scale: f32,
    pub weapon_type: String,
    pub weapon_scale: f32,
}

impl Default for PoseProportionConfig {
    fn default() -> Self {
        Self {
            preset: "chibi_3".to_string(),
            head_scale: 1.32,
            torso_scale: 0.92,
            limb_scale: 0.85,
            weapon_type: "sword".to_string(),
            weapon_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseKeyframe {
    pub t: f32,
    pub root_pos: [f32; 3],
    pub bones: BTreeMap<String, [f32; 3]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PoseActionClip {
    pub id: String,
    pub name: String,
    pub name_en: String,
    pub category: String,
    pub builtin: bool,
    pub loop_anim: bool,
    pub default_rows: u32,
    pub default_cols: u32,
    pub camera: PoseCameraConfig,
    pub proportion: PoseProportionConfig,
    pub keyframes: Vec<PoseKeyframe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpPoseBridgeRequest {
    pub req_id: String,
    pub action: PoseActionClip,
    pub rows: u32,
    pub cols: u32,
    pub width: u32,
    pub height: u32,
    pub created_at_ms: u64,
}

pub fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn apply_camera_preset(cam: &mut PoseCameraConfig, preset: &str) {
    let p = preset.trim().to_lowercase();
    if p.contains("left") || p.contains("朝左") {
        cam.preset = "side_2d_left".to_string();
        cam.yaw = 90.0;
        cam.pitch = 0.0;
        cam.ortho = true;
    } else if p.contains("hd2d") || p.contains("45") {
        cam.preset = "hd2d_45".to_string();
        cam.yaw = -55.0;
        cam.pitch = 25.0;
        cam.ortho = true;
    } else if p.contains("34") || p.contains("3/4") || p.contains("俯视") {
        cam.preset = "topdown_34".to_string();
        cam.yaw = -35.0;
        cam.pitch = 45.0;
        cam.ortho = true;
    } else if p.contains("front") || p.contains("正视") {
        cam.preset = "front_view".to_string();
        cam.yaw = 0.0;
        cam.pitch = 5.0;
        cam.ortho = true;
    } else if p.contains("top") || p.contains("顶视") || p.contains("90") {
        cam.preset = "top_pure".to_string();
        cam.yaw = 0.0;
        cam.pitch = 82.0;
        cam.ortho = true;
    } else {
        cam.preset = "side_2d".to_string();
        cam.yaw = -90.0;
        cam.pitch = 0.0;
        cam.ortho = true;
    }
}

pub fn apply_proportion_preset(prop: &mut PoseProportionConfig, preset: &str) {
    let p = preset.trim().to_lowercase();
    if p.contains("2") || p.contains("q版") || p.contains("chibi_2") {
        prop.preset = "chibi_2".to_string();
        prop.head_scale = 1.65;
        prop.torso_scale = 0.82;
        prop.limb_scale = 0.68;
    } else if p.contains("6") || p.contains("standard") || p.contains("常规") {
        prop.preset = "standard_6".to_string();
        prop.head_scale = 1.0;
        prop.torso_scale = 1.0;
        prop.limb_scale = 1.0;
    } else {
        prop.preset = "chibi_3".to_string();
        prop.head_scale = 1.32;
        prop.torso_scale = 0.92;
        prop.limb_scale = 0.85;
    }
}

fn empty_bones() -> BTreeMap<String, [f32; 3]> {
    let mut map = BTreeMap::new();
    for k in BONE_KEYS {
        map.insert(k.to_string(), [0.0, 0.0, 0.0]);
    }
    map
}

fn make_kf(t: f32, root_pos: [f32; 3], overrides: &[(&str, [f32; 3])]) -> PoseKeyframe {
    let mut bones = empty_bones();
    for (k, v) in overrides {
        bones.insert((*k).to_string(), *v);
    }
    PoseKeyframe { t, root_pos, bones }
}

pub fn builtin_pose_clips() -> Vec<PoseActionClip> {
    let cam = PoseCameraConfig::default();
    let prop_sword = PoseProportionConfig::default();
    let mut prop_unarmed = PoseProportionConfig::default();
    prop_unarmed.weapon_type = "none".to_string();
    let mut prop_staff = PoseProportionConfig::default();
    prop_staff.weapon_type = "staff".to_string();

    vec![
        // 1. idle
        PoseActionClip {
            id: "idle".to_string(),
            name: "待机呼吸".to_string(),
            name_en: "Idle Breathing".to_string(),
            category: "locomotion".to_string(),
            builtin: true,
            loop_anim: true,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_sword.clone(),
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [3.0, -6.0, 0.0]),
                        ("chest", [2.0, -4.0, 0.0]),
                        ("l_upper_arm", [12.0, 0.0, 14.0]),
                        ("l_fore_arm", [-28.0, 0.0, 0.0]),
                        ("r_upper_arm", [-10.0, 0.0, -14.0]),
                        ("r_fore_arm", [-24.0, 0.0, 0.0]),
                        ("l_thigh", [-8.0, 0.0, 4.0]),
                        ("l_shin", [14.0, 0.0, 0.0]),
                        ("r_thigh", [10.0, 0.0, -4.0]),
                        ("r_shin", [12.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.5,
                    [0.0, -0.05, 0.0],
                    &[
                        ("spine", [6.0, -4.0, 0.0]),
                        ("chest", [5.0, -2.0, 0.0]),
                        ("head", [-4.0, 0.0, 0.0]),
                        ("l_upper_arm", [18.0, 0.0, 18.0]),
                        ("l_fore_arm", [-35.0, 0.0, 0.0]),
                        ("r_upper_arm", [-16.0, 0.0, -18.0]),
                        ("r_fore_arm", [-32.0, 0.0, 0.0]),
                        ("l_thigh", [-14.0, 0.0, 4.0]),
                        ("l_shin", [24.0, 0.0, 0.0]),
                        ("r_thigh", [6.0, 0.0, -4.0]),
                        ("r_shin", [22.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [3.0, -6.0, 0.0]),
                        ("chest", [2.0, -4.0, 0.0]),
                        ("l_upper_arm", [12.0, 0.0, 14.0]),
                        ("l_fore_arm", [-28.0, 0.0, 0.0]),
                        ("r_upper_arm", [-10.0, 0.0, -14.0]),
                        ("r_fore_arm", [-24.0, 0.0, 0.0]),
                        ("l_thigh", [-8.0, 0.0, 4.0]),
                        ("l_shin", [14.0, 0.0, 0.0]),
                        ("r_thigh", [10.0, 0.0, -4.0]),
                        ("r_shin", [12.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
        // 2. walk
        PoseActionClip {
            id: "walk".to_string(),
            name: "标准行走".to_string(),
            name_en: "Walk Cycle".to_string(),
            category: "locomotion".to_string(),
            builtin: true,
            loop_anim: true,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_unarmed.clone(),
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, -0.03, 0.0],
                    &[
                        ("spine", [6.0, 8.0, 0.0]),
                        ("l_upper_arm", [-32.0, 0.0, 10.0]),
                        ("l_fore_arm", [-25.0, 0.0, 0.0]),
                        ("r_upper_arm", [28.0, 0.0, -10.0]),
                        ("r_fore_arm", [-20.0, 0.0, 0.0]),
                        ("l_thigh", [30.0, 0.0, 2.0]),
                        ("l_shin", [18.0, 0.0, 0.0]),
                        ("r_thigh", [-28.0, 0.0, -2.0]),
                        ("r_shin", [10.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.25,
                    [0.0, 0.04, 0.0],
                    &[
                        ("spine", [5.0, 0.0, 0.0]),
                        ("l_upper_arm", [-10.0, 0.0, 10.0]),
                        ("l_fore_arm", [-22.0, 0.0, 0.0]),
                        ("r_upper_arm", [10.0, 0.0, -10.0]),
                        ("r_fore_arm", [-22.0, 0.0, 0.0]),
                        ("l_thigh", [6.0, 0.0, 2.0]),
                        ("l_shin", [42.0, 0.0, 0.0]),
                        ("r_thigh", [-4.0, 0.0, -2.0]),
                        ("r_shin", [6.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.5,
                    [0.0, -0.03, 0.0],
                    &[
                        ("spine", [6.0, -8.0, 0.0]),
                        ("l_upper_arm", [28.0, 0.0, 10.0]),
                        ("l_fore_arm", [-20.0, 0.0, 0.0]),
                        ("r_upper_arm", [-32.0, 0.0, -10.0]),
                        ("r_fore_arm", [-25.0, 0.0, 0.0]),
                        ("l_thigh", [-28.0, 0.0, 2.0]),
                        ("l_shin", [10.0, 0.0, 0.0]),
                        ("r_thigh", [30.0, 0.0, -2.0]),
                        ("r_shin", [18.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.75,
                    [0.0, 0.04, 0.0],
                    &[
                        ("spine", [5.0, 0.0, 0.0]),
                        ("l_upper_arm", [10.0, 0.0, 10.0]),
                        ("l_fore_arm", [-22.0, 0.0, 0.0]),
                        ("r_upper_arm", [-10.0, 0.0, -10.0]),
                        ("r_fore_arm", [-22.0, 0.0, 0.0]),
                        ("l_thigh", [-4.0, 0.0, 2.0]),
                        ("l_shin", [6.0, 0.0, 0.0]),
                        ("r_thigh", [6.0, 0.0, -2.0]),
                        ("r_shin", [42.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, -0.03, 0.0],
                    &[
                        ("spine", [6.0, 8.0, 0.0]),
                        ("l_upper_arm", [-32.0, 0.0, 10.0]),
                        ("l_fore_arm", [-25.0, 0.0, 0.0]),
                        ("r_upper_arm", [28.0, 0.0, -10.0]),
                        ("r_fore_arm", [-20.0, 0.0, 0.0]),
                        ("l_thigh", [30.0, 0.0, 2.0]),
                        ("l_shin", [18.0, 0.0, 0.0]),
                        ("r_thigh", [-28.0, 0.0, -2.0]),
                        ("r_shin", [10.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
        // 3. run
        PoseActionClip {
            id: "run".to_string(),
            name: "疾跑冲刺".to_string(),
            name_en: "Run Sprint".to_string(),
            category: "locomotion".to_string(),
            builtin: true,
            loop_anim: true,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_sword.clone(),
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, -0.06, 0.0],
                    &[
                        ("spine", [18.0, 12.0, 0.0]),
                        ("chest", [10.0, 8.0, 0.0]),
                        ("head", [-14.0, 0.0, 0.0]),
                        ("l_upper_arm", [-58.0, 0.0, 12.0]),
                        ("l_fore_arm", [-65.0, 0.0, 0.0]),
                        ("r_upper_arm", [48.0, 0.0, -12.0]),
                        ("r_fore_arm", [-55.0, 0.0, 0.0]),
                        ("l_thigh", [48.0, 0.0, 2.0]),
                        ("l_shin", [55.0, 0.0, 0.0]),
                        ("r_thigh", [-52.0, 0.0, -2.0]),
                        ("r_shin", [22.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.25,
                    [0.0, 0.09, 0.0],
                    &[
                        ("spine", [20.0, 0.0, 0.0]),
                        ("chest", [10.0, 0.0, 0.0]),
                        ("head", [-16.0, 0.0, 0.0]),
                        ("l_upper_arm", [-15.0, 0.0, 14.0]),
                        ("l_fore_arm", [-70.0, 0.0, 0.0]),
                        ("r_upper_arm", [15.0, 0.0, -14.0]),
                        ("r_fore_arm", [-65.0, 0.0, 0.0]),
                        ("l_thigh", [15.0, 0.0, 2.0]),
                        ("l_shin", [78.0, 0.0, 0.0]),
                        ("r_thigh", [-18.0, 0.0, -2.0]),
                        ("r_shin", [25.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.5,
                    [0.0, -0.06, 0.0],
                    &[
                        ("spine", [18.0, -12.0, 0.0]),
                        ("chest", [10.0, -8.0, 0.0]),
                        ("head", [-14.0, 0.0, 0.0]),
                        ("l_upper_arm", [48.0, 0.0, 12.0]),
                        ("l_fore_arm", [-55.0, 0.0, 0.0]),
                        ("r_upper_arm", [-58.0, 0.0, -12.0]),
                        ("r_fore_arm", [-65.0, 0.0, 0.0]),
                        ("l_thigh", [-52.0, 0.0, 2.0]),
                        ("l_shin", [22.0, 0.0, 0.0]),
                        ("r_thigh", [48.0, 0.0, -2.0]),
                        ("r_shin", [55.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.75,
                    [0.0, 0.09, 0.0],
                    &[
                        ("spine", [20.0, 0.0, 0.0]),
                        ("chest", [10.0, 0.0, 0.0]),
                        ("head", [-16.0, 0.0, 0.0]),
                        ("l_upper_arm", [15.0, 0.0, 14.0]),
                        ("l_fore_arm", [-65.0, 0.0, 0.0]),
                        ("r_upper_arm", [-15.0, 0.0, -14.0]),
                        ("r_fore_arm", [-70.0, 0.0, 0.0]),
                        ("l_thigh", [-18.0, 0.0, 2.0]),
                        ("l_shin", [25.0, 0.0, 0.0]),
                        ("r_thigh", [15.0, 0.0, -2.0]),
                        ("r_shin", [78.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, -0.06, 0.0],
                    &[
                        ("spine", [18.0, 12.0, 0.0]),
                        ("chest", [10.0, 8.0, 0.0]),
                        ("head", [-14.0, 0.0, 0.0]),
                        ("l_upper_arm", [-58.0, 0.0, 12.0]),
                        ("l_fore_arm", [-65.0, 0.0, 0.0]),
                        ("r_upper_arm", [48.0, 0.0, -12.0]),
                        ("r_fore_arm", [-55.0, 0.0, 0.0]),
                        ("l_thigh", [48.0, 0.0, 2.0]),
                        ("l_shin", [55.0, 0.0, 0.0]),
                        ("r_thigh", [-52.0, 0.0, -2.0]),
                        ("r_shin", [22.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
        // 4. jump (Engine In-Place Velocity States: collider center stays anchored near origin while limbs express squat -> launch stretch -> knee tuck -> fall extension -> land)
        PoseActionClip {
            id: "jump".to_string(),
            name: "跳跃起落 (引擎原地)".to_string(),
            name_en: "Jump Velocity States (In-Place)".to_string(),
            category: "locomotion".to_string(),
            builtin: true,
            loop_anim: false,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_unarmed.clone(),
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, -0.18, 0.0],
                    &[
                        ("spine", [24.0, 0.0, 0.0]),
                        ("chest", [12.0, 0.0, 0.0]),
                        ("head", [-18.0, 0.0, 0.0]),
                        ("l_upper_arm", [42.0, 0.0, 16.0]),
                        ("l_fore_arm", [-35.0, 0.0, 0.0]),
                        ("r_upper_arm", [42.0, 0.0, -16.0]),
                        ("r_fore_arm", [-35.0, 0.0, 0.0]),
                        ("l_thigh", [-48.0, 0.0, 4.0]),
                        ("l_shin", [76.0, 0.0, 0.0]),
                        ("r_thigh", [-42.0, 0.0, -4.0]),
                        ("r_shin", [72.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.25,
                    [0.0, 0.06, 0.0],
                    &[
                        ("spine", [-8.0, 0.0, 0.0]),
                        ("chest", [-6.0, 0.0, 0.0]),
                        ("l_upper_arm", [-68.0, 0.0, 18.0]),
                        ("l_fore_arm", [-20.0, 0.0, 0.0]),
                        ("r_upper_arm", [-68.0, 0.0, -18.0]),
                        ("r_fore_arm", [-20.0, 0.0, 0.0]),
                        ("l_thigh", [-22.0, 0.0, 4.0]),
                        ("l_shin", [40.0, 0.0, 0.0]),
                        ("r_thigh", [18.0, 0.0, -4.0]),
                        ("r_shin", [12.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.5,
                    [0.0, 0.08, 0.0],
                    &[
                        ("spine", [6.0, 0.0, 0.0]),
                        ("chest", [4.0, 0.0, 0.0]),
                        ("l_upper_arm", [-95.0, 0.0, 26.0]),
                        ("l_fore_arm", [-28.0, 0.0, 0.0]),
                        ("r_upper_arm", [-85.0, 0.0, -26.0]),
                        ("r_fore_arm", [-28.0, 0.0, 0.0]),
                        ("l_thigh", [-58.0, 0.0, 5.0]),
                        ("l_shin", [86.0, 0.0, 0.0]),
                        ("r_thigh", [-34.0, 0.0, -5.0]),
                        ("r_shin", [68.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.75,
                    [0.0, 0.04, 0.0],
                    &[
                        ("spine", [10.0, 0.0, 0.0]),
                        ("l_upper_arm", [-35.0, 0.0, 24.0]),
                        ("l_fore_arm", [-25.0, 0.0, 0.0]),
                        ("r_upper_arm", [-30.0, 0.0, -24.0]),
                        ("r_fore_arm", [-25.0, 0.0, 0.0]),
                        ("l_thigh", [-18.0, 0.0, 4.0]),
                        ("l_shin", [16.0, 0.0, 0.0]),
                        ("r_thigh", [14.0, 0.0, -4.0]),
                        ("r_shin", [36.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, -0.16, 0.0],
                    &[
                        ("spine", [22.0, 0.0, 0.0]),
                        ("chest", [10.0, 0.0, 0.0]),
                        ("l_upper_arm", [28.0, 0.0, 18.0]),
                        ("l_fore_arm", [-32.0, 0.0, 0.0]),
                        ("r_upper_arm", [28.0, 0.0, -18.0]),
                        ("r_fore_arm", [-32.0, 0.0, 0.0]),
                        ("l_thigh", [-44.0, 0.0, 4.0]),
                        ("l_shin", [70.0, 0.0, 0.0]),
                        ("r_thigh", [-38.0, 0.0, -4.0]),
                        ("r_shin", [66.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
        // 5. attack (windup -> lunge slash -> follow-through -> recovery)
        PoseActionClip {
            id: "attack".to_string(),
            name: "挥砍攻击".to_string(),
            name_en: "Melee Slash".to_string(),
            category: "combat".to_string(),
            builtin: true,
            loop_anim: false,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_sword.clone(),
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [6.0, -12.0, 0.0]),
                        ("r_upper_arm", [-25.0, 0.0, -18.0]),
                        ("r_fore_arm", [-40.0, 0.0, 0.0]),
                        ("weapon", [15.0, 0.0, 0.0]),
                        ("l_thigh", [-16.0, 0.0, 4.0]),
                        ("l_shin", [22.0, 0.0, 0.0]),
                        ("r_thigh", [18.0, 0.0, -4.0]),
                        ("r_shin", [18.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.25,
                    [0.0, -0.06, -0.14],
                    &[
                        ("spine", [-8.0, -35.0, 0.0]),
                        ("chest", [-6.0, -20.0, 0.0]),
                        ("l_upper_arm", [28.0, 0.0, 22.0]),
                        ("r_upper_arm", [-115.0, -20.0, -28.0]),
                        ("r_fore_arm", [-35.0, 0.0, 0.0]),
                        ("weapon", [-25.0, 0.0, 0.0]),
                        ("l_thigh", [-12.0, 0.0, 4.0]),
                        ("l_shin", [18.0, 0.0, 0.0]),
                        ("r_thigh", [-28.0, 0.0, -4.0]),
                        ("r_shin", [48.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.55,
                    [0.0, -0.10, 0.24],
                    &[
                        ("spine", [22.0, 32.0, 0.0]),
                        ("chest", [14.0, 24.0, 0.0]),
                        ("l_upper_arm", [38.0, 0.0, 25.0]),
                        ("r_upper_arm", [-32.0, 35.0, -15.0]),
                        ("r_fore_arm", [-10.0, 0.0, 0.0]),
                        ("weapon", [65.0, 0.0, 0.0]),
                        ("l_thigh", [-45.0, 0.0, 4.0]),
                        ("l_shin", [58.0, 0.0, 0.0]),
                        ("r_thigh", [38.0, 0.0, -4.0]),
                        ("r_shin", [14.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.8,
                    [0.0, -0.08, 0.18],
                    &[
                        ("spine", [16.0, 24.0, 0.0]),
                        ("chest", [10.0, 16.0, 0.0]),
                        ("l_upper_arm", [30.0, 0.0, 20.0]),
                        ("r_upper_arm", [25.0, 18.0, -22.0]),
                        ("r_fore_arm", [-18.0, 0.0, 0.0]),
                        ("weapon", [82.0, 0.0, 0.0]),
                        ("l_thigh", [-34.0, 0.0, 4.0]),
                        ("l_shin", [44.0, 0.0, 0.0]),
                        ("r_thigh", [28.0, 0.0, -4.0]),
                        ("r_shin", [16.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [6.0, -10.0, 0.0]),
                        ("r_upper_arm", [-22.0, 0.0, -16.0]),
                        ("r_fore_arm", [-36.0, 0.0, 0.0]),
                        ("weapon", [15.0, 0.0, 0.0]),
                        ("l_thigh", [-14.0, 0.0, 4.0]),
                        ("l_shin", [20.0, 0.0, 0.0]),
                        ("r_thigh", [16.0, 0.0, -4.0]),
                        ("r_shin", [18.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
        // 6. cast
        PoseActionClip {
            id: "cast".to_string(),
            name: "蓄力施法".to_string(),
            name_en: "Spell Cast".to_string(),
            category: "combat".to_string(),
            builtin: true,
            loop_anim: false,
            default_rows: 2,
            default_cols: 3,
            camera: cam.clone(),
            proportion: prop_staff,
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, -0.05, 0.0],
                    &[
                        ("spine", [10.0, 0.0, 0.0]),
                        ("l_upper_arm", [-25.0, 20.0, 12.0]),
                        ("l_fore_arm", [-55.0, 0.0, 0.0]),
                        ("r_upper_arm", [-30.0, -20.0, -12.0]),
                        ("r_fore_arm", [-50.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.45,
                    [0.0, 0.24, 0.0],
                    &[
                        ("spine", [-12.0, 0.0, 0.0]),
                        ("chest", [-10.0, 0.0, 0.0]),
                        ("head", [-15.0, 0.0, 0.0]),
                        ("l_upper_arm", [-110.0, 0.0, 32.0]),
                        ("l_fore_arm", [-18.0, 0.0, 0.0]),
                        ("r_upper_arm", [-115.0, 0.0, -32.0]),
                        ("r_fore_arm", [-15.0, 0.0, 0.0]),
                        ("weapon", [-20.0, 0.0, 0.0]),
                        ("l_thigh", [-20.0, 0.0, 4.0]),
                        ("l_shin", [45.0, 0.0, 0.0]),
                        ("r_thigh", [-10.0, 0.0, -4.0]),
                        ("r_shin", [35.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.75,
                    [0.0, 0.08, 0.12],
                    &[
                        ("spine", [14.0, 10.0, 0.0]),
                        ("l_upper_arm", [-75.0, 0.0, 18.0]),
                        ("r_upper_arm", [-82.0, 0.0, -18.0]),
                        ("weapon", [35.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [4.0, 0.0, 0.0]),
                        ("l_upper_arm", [-15.0, 0.0, 14.0]),
                        ("r_upper_arm", [-20.0, 0.0, -14.0]),
                    ],
                ),
            ],
        },
        // 7. hurt
        PoseActionClip {
            id: "hurt".to_string(),
            name: "受击硬直".to_string(),
            name_en: "Hit Flinch".to_string(),
            category: "combat".to_string(),
            builtin: true,
            loop_anim: false,
            default_rows: 1,
            default_cols: 4,
            camera: cam.clone(),
            proportion: prop_sword.clone(),
            keyframes: vec![
                make_kf(0.0, [0.0, 0.0, 0.0], &[("spine", [4.0, 0.0, 0.0])]),
                make_kf(
                    0.35,
                    [0.0, -0.08, -0.22],
                    &[
                        ("hips", [-14.0, 0.0, 0.0]),
                        ("spine", [-24.0, -18.0, 0.0]),
                        ("chest", [-18.0, -12.0, 0.0]),
                        ("head", [-26.0, 0.0, 0.0]),
                        ("l_upper_arm", [-48.0, 0.0, 42.0]),
                        ("l_fore_arm", [-20.0, 0.0, 0.0]),
                        ("r_upper_arm", [-42.0, 0.0, -45.0]),
                        ("r_fore_arm", [-20.0, 0.0, 0.0]),
                        ("l_thigh", [22.0, 0.0, 5.0]),
                        ("l_shin", [36.0, 0.0, 0.0]),
                        ("r_thigh", [-18.0, 0.0, -5.0]),
                        ("r_shin", [42.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.7,
                    [0.0, -0.12, -0.26],
                    &[
                        ("spine", [16.0, -10.0, 0.0]),
                        ("head", [18.0, 0.0, 0.0]),
                        ("l_upper_arm", [18.0, 0.0, 26.0]),
                        ("r_upper_arm", [15.0, 0.0, -26.0]),
                        ("l_thigh", [-28.0, 0.0, 4.0]),
                        ("l_shin", [48.0, 0.0, 0.0]),
                        ("r_thigh", [-14.0, 0.0, -4.0]),
                        ("r_shin", [32.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(1.0, [0.0, 0.0, 0.0], &[("spine", [5.0, 0.0, 0.0])]),
            ],
        },
        // 8. ko (standing -> knocked back -> flat on ground root_pos[1] = -0.75, hipsX = 85 deg)
        PoseActionClip {
            id: "ko".to_string(),
            name: "击倒阵亡".to_string(),
            name_en: "Knockout / Death".to_string(),
            category: "combat".to_string(),
            builtin: true,
            loop_anim: false,
            default_rows: 2,
            default_cols: 3,
            camera: cam,
            proportion: prop_sword,
            keyframes: vec![
                make_kf(
                    0.0,
                    [0.0, 0.0, 0.0],
                    &[
                        ("spine", [-10.0, 0.0, 0.0]),
                        ("l_upper_arm", [-30.0, 0.0, 25.0]),
                        ("r_upper_arm", [-30.0, 0.0, -25.0]),
                    ],
                ),
                make_kf(
                    0.35,
                    [0.0, 0.12, -0.25],
                    &[
                        ("hips", [38.0, 0.0, 0.0]),
                        ("spine", [-18.0, 0.0, 0.0]),
                        ("head", [-22.0, 0.0, 0.0]),
                        ("l_upper_arm", [-65.0, 0.0, 38.0]),
                        ("r_upper_arm", [-60.0, 0.0, -38.0]),
                        ("l_thigh", [-24.0, 0.0, 6.0]),
                        ("l_shin", [32.0, 0.0, 0.0]),
                        ("r_thigh", [-12.0, 0.0, -6.0]),
                        ("r_shin", [24.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    0.70,
                    [0.0, -0.62, -0.45],
                    &[
                        ("hips", [78.0, 0.0, 0.0]),
                        ("spine", [8.0, 0.0, 0.0]),
                        ("head", [12.0, 0.0, 0.0]),
                        ("l_upper_arm", [-35.0, 0.0, 48.0]),
                        ("r_upper_arm", [-28.0, 0.0, -48.0]),
                        ("l_thigh", [-10.0, 0.0, 6.0]),
                        ("l_shin", [18.0, 0.0, 0.0]),
                        ("r_thigh", [6.0, 0.0, -6.0]),
                        ("r_shin", [12.0, 0.0, 0.0]),
                    ],
                ),
                make_kf(
                    1.0,
                    [0.0, -0.75, -0.52],
                    &[
                        ("hips", [85.0, 0.0, 0.0]),
                        ("spine", [5.0, 0.0, 0.0]),
                        ("chest", [4.0, 0.0, 0.0]),
                        ("head", [8.0, 15.0, 0.0]),
                        ("l_upper_arm", [-12.0, 0.0, 55.0]),
                        ("l_fore_arm", [-10.0, 0.0, 0.0]),
                        ("r_upper_arm", [-8.0, 0.0, -52.0]),
                        ("r_fore_arm", [-12.0, 0.0, 0.0]),
                        ("l_thigh", [4.0, 0.0, 8.0]),
                        ("l_shin", [8.0, 0.0, 0.0]),
                        ("r_thigh", [6.0, 0.0, -8.0]),
                        ("r_shin", [6.0, 0.0, 0.0]),
                    ],
                ),
            ],
        },
    ]
}

pub fn actions_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("actions")
}

pub fn ensure_seeded_actions(app_data_dir: &Path) -> AppResult<Vec<PoseActionClip>> {
    let dir = actions_dir(app_data_dir);
    fs::create_dir_all(&dir)?;

    let builtins = builtin_pose_clips();
    for clip in &builtins {
        let path = dir.join(format!("{}.json", clip.id));
        if !path.exists() {
            if let Ok(json_str) = serde_json::to_string_pretty(clip) {
                let _ = fs::write(&path, json_str);
            }
        }
    }

    let mut out = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();
    for b in &builtins {
        let path = dir.join(format!("{}.json", b.id));
        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(mut parsed) = serde_json::from_str::<PoseActionClip>(&raw) {
                let mut migrated = false;
                if parsed.builtin
                    && parsed.camera.preset == "side_2d"
                    && (parsed.camera.yaw - 90.0).abs() < 1.0
                {
                    parsed.camera.yaw = -90.0;
                    migrated = true;
                }
                if parsed.builtin
                    && parsed.id == "jump"
                    && parsed.keyframes.iter().any(|k| k.root_pos[1] > 0.20)
                {
                    parsed.name = b.name.clone();
                    parsed.name_en = b.name_en.clone();
                    parsed.keyframes = b.keyframes.clone();
                    migrated = true;
                }
                if migrated {
                    if let Ok(json_str) = serde_json::to_string_pretty(&parsed) {
                        let _ = fs::write(&path, json_str);
                    }
                }
                seen_ids.insert(parsed.id.clone());
                out.push(parsed);
                continue;
            }
        }
        seen_ids.insert(b.id.clone());
        out.push(b.clone());
    }

    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            if let Ok(raw) = fs::read_to_string(&p) {
                if let Ok(parsed) = serde_json::from_str::<PoseActionClip>(&raw) {
                    if !seen_ids.contains(&parsed.id) {
                        seen_ids.insert(parsed.id.clone());
                        out.push(parsed);
                    }
                }
            }
        }
    }

    Ok(out)
}

pub fn get_pose_action(app_data_dir: &Path, action_id: &str) -> AppResult<PoseActionClip> {
    let list = ensure_seeded_actions(app_data_dir)?;
    let q = action_id.trim().to_lowercase();
    list.into_iter()
        .find(|c| {
            c.id.eq_ignore_ascii_case(&q)
                || c.name.to_lowercase() == q
                || c.name_en.to_lowercase() == q
        })
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::NotFound,
                format!("3D pose action '{}' not found", action_id),
            )
        })
}

fn contains_ascii_word(haystack: &str, word: &str) -> bool {
    if word.is_empty() {
        return false;
    }
    let hb = haystack.as_bytes();
    let wb = word.as_bytes();
    if wb.len() > hb.len() {
        return false;
    }
    for i in 0..=(hb.len() - wb.len()) {
        if hb[i..i + wb.len()].eq_ignore_ascii_case(wb) {
            let left_ok = i == 0 || !hb[i - 1].is_ascii_alphanumeric();
            let right_ok =
                i + wb.len() == hb.len() || !hb[i + wb.len()].is_ascii_alphanumeric();
            if left_ok && right_ok {
                return true;
            }
        }
    }
    false
}

fn contains_any_ascii_word(haystack: &str, words: &[&str]) -> bool {
    words.iter().any(|w| contains_ascii_word(haystack, w))
}

pub fn infer_action_id_from_prompt(prompt: &str) -> Option<&'static str> {
    let lower = prompt.to_lowercase();
    if lower.contains("击倒")
        || lower.contains("阵亡")
        || lower.contains("死亡")
        || lower.contains("倒地")
        || contains_any_ascii_word(
            &lower,
            &["ko", "death", "die", "dies", "dying", "dead", "defeat", "collapse"],
        )
    {
        return Some("ko");
    }
    if lower.contains("受击")
        || lower.contains("受伤")
        || lower.contains("挨打")
        || contains_any_ascii_word(
            &lower,
            &["hurt", "hit", "hits", "flinch", "damage", "damaged", "stagger"],
        )
    {
        return Some("hurt");
    }
    if lower.contains("跳跃")
        || lower.contains("起跳")
        || lower.contains("腾空")
        || contains_any_ascii_word(
            &lower,
            &["jump", "jumping", "jumps", "leap", "leaping", "hop", "hopping", "airborne"],
        )
    {
        return Some("jump");
    }
    if lower.contains("施法")
        || lower.contains("魔法")
        || lower.contains("吟唱")
        || contains_any_ascii_word(
            &lower,
            &["cast", "casting", "spell", "magic", "summon", "channel"],
        )
    {
        return Some("cast");
    }
    if lower.contains("攻击")
        || lower.contains("挥砍")
        || lower.contains("劈砍")
        || lower.contains("斩击")
        || contains_any_ascii_word(
            &lower,
            &["attack", "attacking", "slash", "slashing", "strike", "swing", "combo", "stab"],
        )
    {
        return Some("attack");
    }
    if lower.contains("奔跑")
        || lower.contains("跑步")
        || lower.contains("疾跑")
        || lower.contains("冲刺")
        || contains_any_ascii_word(
            &lower,
            &["run", "running", "runs", "sprint", "sprinting", "dash", "dashing"],
        )
    {
        return Some("run");
    }
    if lower.contains("行走")
        || lower.contains("走路")
        || lower.contains("步行")
        || contains_any_ascii_word(&lower, &["walk", "walking", "walks", "stroll"])
    {
        return Some("walk");
    }
    if lower.contains("待机")
        || lower.contains("呼吸")
        || lower.contains("站立")
        || contains_any_ascii_word(&lower, &["idle", "stand", "standing", "breathe"])
    {
        return Some("idle");
    }
    None
}

pub fn save_pose_action_clip(app_data_dir: &Path, mut clip: PoseActionClip) -> AppResult<PoseActionClip> {
    let clean_id: String = clip
        .id
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if clean_id.is_empty() {
        return Err(AppError::new(AppErrorCode::InvalidInput, "Action id cannot be empty"));
    }
    clip.id = clean_id;
    clip.keyframes
        .sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
    let dir = actions_dir(app_data_dir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", clip.id));
    let json_str = serde_json::to_string_pretty(&clip)
        .map_err(|e| AppError::new(AppErrorCode::Internal, e.to_string()))?;
    fs::write(&path, json_str)?;
    Ok(clip)
}

pub fn delete_pose_action_clip(app_data_dir: &Path, id: &str) -> AppResult<bool> {
    let clean_id = id.trim();
    if clean_id.is_empty() {
        return Ok(false);
    }
    let dir = actions_dir(app_data_dir);
    let path = dir.join(format!("{}.json", clean_id));
    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    // If it was a builtin, re-seed default
    let builtins = builtin_pose_clips();
    if let Some(b) = builtins.into_iter().find(|c| c.id == clean_id) {
        if let Ok(json_str) = serde_json::to_string_pretty(&b) {
            let _ = fs::write(&path, json_str);
        }
    }
    Ok(true)
}

// ============================================================================
// Desktop Three.js Live Bridge + Deterministic Software 3D Fallback
// ============================================================================

fn bridge_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("pose3d_bridge")
}

pub fn record_desktop_heartbeat(app_data_dir: &Path) {
    let dir = bridge_dir(app_data_dir);
    let _ = fs::create_dir_all(&dir);
    let hb_path = dir.join("heartbeat.json");
    let _ = fs::write(&hb_path, format!("{{\"ts\":{}}}", now_unix_ms()));
}

fn is_desktop_heartbeat_fresh(app_data_dir: &Path, max_age_ms: u64) -> bool {
    let hb_path = bridge_dir(app_data_dir).join("heartbeat.json");
    let Ok(raw) = fs::read_to_string(&hb_path) else {
        return false;
    };
    let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return false;
    };
    let Some(ts) = val.get("ts").and_then(|v| v.as_u64()) else {
        return false;
    };
    now_unix_ms().saturating_sub(ts) <= max_age_ms
}

pub fn poll_bridge_request(app_data_dir: &Path) -> AppResult<Option<McpPoseBridgeRequest>> {
    record_desktop_heartbeat(app_data_dir);
    let req_path = bridge_dir(app_data_dir).join("active_request.json");
    if !req_path.exists() {
        return Ok(None);
    }
    let Ok(raw) = fs::read_to_string(&req_path) else {
        return Ok(None);
    };
    let _ = fs::remove_file(&req_path);
    let parsed = serde_json::from_str::<McpPoseBridgeRequest>(&raw).ok();
    Ok(parsed)
}

pub fn complete_bridge_request(app_data_dir: &Path, req_id: &str, data_url: &str) -> AppResult<bool> {
    let dir = bridge_dir(app_data_dir);
    fs::create_dir_all(&dir)?;
    let b64 = data_url
        .split_once(",")
        .map(|(_, b)| b.trim())
        .unwrap_or_else(|| data_url.trim());
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| AppError::new(AppErrorCode::InvalidInput, e.to_string()))?;
    let resp_path = dir.join(format!("response_{}.png", req_id.trim()));
    fs::write(&resp_path, bytes)?;
    Ok(true)
}

pub fn push_live_action_to_bridge(
    app_data_dir: &Path,
    clip: &PoseActionClip,
    rows: u32,
    cols: u32,
    width: u32,
    height: u32,
) -> String {
    let dir = bridge_dir(app_data_dir);
    let _ = fs::create_dir_all(&dir);
    let req_id = uuid::Uuid::new_v4().simple().to_string();
    let req = McpPoseBridgeRequest {
        req_id: req_id.clone(),
        action: clip.clone(),
        rows: rows.max(1),
        cols: cols.max(1),
        width: width.max(256),
        height: height.max(256),
        created_at_ms: now_unix_ms(),
    };
    if let Ok(json_str) = serde_json::to_string_pretty(&req) {
        let _ = fs::write(dir.join("active_request.json"), json_str);
    }
    req_id
}

#[cfg(not(test))]
fn is_desktop_process_running() -> bool {
    if let Ok(out) = std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq Sketch2Asset.exe", "/NH"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&out.stdout).to_ascii_lowercase();
        if stdout.contains("sketch2asset.exe") {
            return true;
        }
    }
    false
}

#[cfg(not(test))]
fn try_launch_desktop_app() {
    if is_desktop_process_running() {
        return;
    }
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            candidates.push(parent.join("Sketch2Asset.exe"));
        }
    }
    candidates.push(PathBuf::from("D:/Sketch2Asset/bin/Sketch2Asset.exe"));
    for cand in candidates {
        if cand.exists() {
            let _ = std::process::Command::new(&cand).spawn();
            break;
        }
    }
}

#[cfg(test)]
fn try_launch_desktop_app() {}

pub fn render_pose_via_bridge_or_fallback(
    app_data_dir: &Path,
    clip: &PoseActionClip,
    rows: u32,
    cols: u32,
    width: u32,
    height: u32,
) -> AppResult<(Vec<u8>, &'static str)> {
    let req_id = push_live_action_to_bridge(app_data_dir, clip, rows, cols, width, height);
    let resp_path = bridge_dir(app_data_dir).join(format!("response_{}.png", req_id));

    let had_heartbeat = is_desktop_heartbeat_fresh(app_data_dir, 3000);
    if !had_heartbeat {
        try_launch_desktop_app();
    }

    #[cfg(not(test))]
    let wait_iters = if had_heartbeat { 30 } else { 15 };
    #[cfg(test)]
    let wait_iters = 0;

    for _ in 0..wait_iters {
        if resp_path.exists() {
            if let Ok(bytes) = fs::read(&resp_path) {
                let _ = fs::remove_file(&resp_path);
                if !bytes.is_empty() {
                    return Ok((bytes, "desktop_threejs_bridge"));
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(80));
    }

    let png_bytes = render_pose_clip_grid_png(clip, rows, cols, width, height)?;
    Ok((png_bytes, "software_3d_renderer"))
}

// ============================================================================
// Software 3D Hierarchical Forward Kinematics & Unlit Capsule Renderer
// ============================================================================

#[derive(Debug, Clone, Copy)]
struct Vec3 {
    x: f32,
    y: f32,
    z: f32,
}

impl Vec3 {
    fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }
    fn add(self, o: Self) -> Self {
        Self::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}

#[derive(Debug, Clone, Copy)]
struct Mat3 {
    m: [[f32; 3]; 3],
}

impl Mat3 {
    fn mul(self, rhs: Self) -> Self {
        let mut out = [[0.0_f32; 3]; 3];
        for r in 0..3 {
            for c in 0..3 {
                out[r][c] =
                    self.m[r][0] * rhs.m[0][c] + self.m[r][1] * rhs.m[1][c] + self.m[r][2] * rhs.m[2][c];
            }
        }
        Self { m: out }
    }

    fn mul_vec(self, v: Vec3) -> Vec3 {
        Vec3::new(
            self.m[0][0] * v.x + self.m[0][1] * v.y + self.m[0][2] * v.z,
            self.m[1][0] * v.x + self.m[1][1] * v.y + self.m[1][2] * v.z,
            self.m[2][0] * v.x + self.m[2][1] * v.y + self.m[2][2] * v.z,
        )
    }

    fn from_euler_xyz_deg(deg: [f32; 3]) -> Self {
        let rx = deg[0].to_radians();
        let ry = deg[1].to_radians();
        let rz = deg[2].to_radians();
        let (sx, cx) = rx.sin_cos();
        let (sy, cy) = ry.sin_cos();
        let (sz, cz) = rz.sin_cos();

        let mx = Self {
            m: [[1.0, 0.0, 0.0], [0.0, cx, -sx], [0.0, sx, cx]],
        };
        let my = Self {
            m: [[cy, 0.0, sy], [0.0, 1.0, 0.0], [-sy, 0.0, cy]],
        };
        let mz = Self {
            m: [[cz, -sz, 0.0], [sz, cz, 0.0], [0.0, 0.0, 1.0]],
        };
        mx.mul(my).mul(mz)
    }
}

pub fn sample_clip_at(clip: &PoseActionClip, t_raw: f32) -> PoseKeyframe {
    let t = t_raw.clamp(0.0, 1.0);
    if clip.keyframes.is_empty() {
        return make_kf(t, [0.0, 0.0, 0.0], &[]);
    }
    if clip.keyframes.len() == 1 || t <= clip.keyframes[0].t {
        let mut kf = clip.keyframes[0].clone();
        kf.t = t;
        return kf;
    }
    let last = clip.keyframes.last().unwrap();
    if t >= last.t {
        let mut kf = last.clone();
        kf.t = t;
        return kf;
    }

    let mut idx = 0;
    for i in 0..(clip.keyframes.len() - 1) {
        if t >= clip.keyframes[i].t && t <= clip.keyframes[i + 1].t {
            idx = i;
            break;
        }
    }
    let a = &clip.keyframes[idx];
    let b = &clip.keyframes[idx + 1];
    let span = (b.t - a.t).max(1e-5);
    let alpha = ((t - a.t) / span).clamp(0.0, 1.0);

    let root_pos = [
        a.root_pos[0] + (b.root_pos[0] - a.root_pos[0]) * alpha,
        a.root_pos[1] + (b.root_pos[1] - a.root_pos[1]) * alpha,
        a.root_pos[2] + (b.root_pos[2] - a.root_pos[2]) * alpha,
    ];

    let mut bones = empty_bones();
    for key in BONE_KEYS {
        let va = a.bones.get(key).copied().unwrap_or([0.0, 0.0, 0.0]);
        let vb = b.bones.get(key).copied().unwrap_or([0.0, 0.0, 0.0]);
        bones.insert(
            key.to_string(),
            [
                va[0] + (vb[0] - va[0]) * alpha,
                va[1] + (vb[1] - va[1]) * alpha,
                va[2] + (vb[2] - va[2]) * alpha,
            ],
        );
    }

    PoseKeyframe { t, root_pos, bones }
}

#[derive(Debug, Clone)]
struct CapsuleSeg {
    p0: Vec3,
    p1: Vec3,
    radius: f32,
    color: [u8; 4],
}

fn build_stickman_capsules(kf: &PoseKeyframe, prop: &PoseProportionConfig) -> Vec<CapsuleSeg> {
    let rot = |name: &str| -> Mat3 {
        Mat3::from_euler_xyz_deg(kf.bones.get(name).copied().unwrap_or([0.0, 0.0, 0.0]))
    };

    let ts = prop.torso_scale.clamp(0.5, 1.8);
    let ls = prop.limb_scale.clamp(0.5, 1.8);
    let hs = prop.head_scale.clamp(0.6, 2.2);
    let ws = prop.weapon_scale.clamp(0.4, 2.2);

    // Base standing hip height is 0.90 * ls so feet touch y = 0.0 when root_pos[1] == 0.0
    let hip_base_y = 0.90 * ls;
    let p_hips = Vec3::new(
        kf.root_pos[0],
        hip_base_y + kf.root_pos[1],
        kf.root_pos[2],
    );
    let r_hips = rot("hips");

    let r_spine = r_hips.mul(rot("spine"));
    let p_spine = p_hips.add(r_hips.mul_vec(Vec3::new(0.0, 0.24 * ts, 0.0)));
    let r_chest = r_spine.mul(rot("chest"));
    let p_chest = p_spine.add(r_spine.mul_vec(Vec3::new(0.0, 0.28 * ts, 0.0)));
    let p_neck = p_chest.add(r_chest.mul_vec(Vec3::new(0.0, 0.14 * ts, 0.0)));

    let r_head = r_chest.mul(rot("head"));
    let head_center = p_neck.add(r_head.mul_vec(Vec3::new(0.0, 0.19 * hs, 0.0)));
    let head_nose = head_center.add(r_head.mul_vec(Vec3::new(0.0, 0.02 * hs, 0.24 * hs)));

    // Shoulders
    let shoulder_w = 0.22 * ts;
    let p_l_shoulder = p_chest.add(r_chest.mul_vec(Vec3::new(shoulder_w, 0.08 * ts, 0.0)));
    let p_r_shoulder = p_chest.add(r_chest.mul_vec(Vec3::new(-shoulder_w, 0.08 * ts, 0.0)));

    let r_l_uarm = r_chest.mul(rot("l_upper_arm"));
    let p_l_elbow = p_l_shoulder.add(r_l_uarm.mul_vec(Vec3::new(0.0, -0.30 * ls, 0.0)));
    let r_l_farm = r_l_uarm.mul(rot("l_fore_arm"));
    let p_l_hand = p_l_elbow.add(r_l_farm.mul_vec(Vec3::new(0.0, -0.28 * ls, 0.0)));

    let r_r_uarm = r_chest.mul(rot("r_upper_arm"));
    let p_r_elbow = p_r_shoulder.add(r_r_uarm.mul_vec(Vec3::new(0.0, -0.30 * ls, 0.0)));
    let r_r_farm = r_r_uarm.mul(rot("r_fore_arm"));
    let p_r_hand = p_r_elbow.add(r_r_farm.mul_vec(Vec3::new(0.0, -0.28 * ls, 0.0)));

    // Hips -> Legs
    let hip_w = 0.13 * ts;
    let p_l_hip = p_hips.add(r_hips.mul_vec(Vec3::new(hip_w, 0.0, 0.0)));
    let p_r_hip = p_hips.add(r_hips.mul_vec(Vec3::new(-hip_w, 0.0, 0.0)));

    let r_l_thigh = r_hips.mul(rot("l_thigh"));
    let p_l_knee = p_l_hip.add(r_l_thigh.mul_vec(Vec3::new(0.0, -0.44 * ls, 0.0)));
    let r_l_shin = r_l_thigh.mul(rot("l_shin"));
    let p_l_ankle = p_l_knee.add(r_l_shin.mul_vec(Vec3::new(0.0, -0.44 * ls, 0.0)));
    let r_l_foot = r_l_shin.mul(rot("l_foot"));
    let p_l_toe = p_l_ankle.add(r_l_foot.mul_vec(Vec3::new(0.0, -0.02 * ls, 0.16 * ls)));

    let r_r_thigh = r_hips.mul(rot("r_thigh"));
    let p_r_knee = p_r_hip.add(r_r_thigh.mul_vec(Vec3::new(0.0, -0.44 * ls, 0.0)));
    let r_r_shin = r_r_thigh.mul(rot("r_shin"));
    let p_r_ankle = p_r_knee.add(r_r_shin.mul_vec(Vec3::new(0.0, -0.44 * ls, 0.0)));
    let r_r_foot = r_r_shin.mul(rot("r_foot"));
    let p_r_toe = p_r_ankle.add(r_r_foot.mul_vec(Vec3::new(0.0, -0.02 * ls, 0.16 * ls)));

    let c_torso = [71, 85, 105, 255];
    let c_head = [51, 65, 85, 255];
    let c_visor = [14, 165, 233, 255];
    let c_left_upper = [0, 184, 217, 255];
    let c_left_lower = [2, 132, 199, 255];
    let c_right_upper = [244, 63, 94, 255];
    let c_right_lower = [234, 88, 12, 255];
    let c_weapon = [16, 185, 129, 255];

    let mut segs = vec![
        // Left limbs
        CapsuleSeg { p0: p_l_hip, p1: p_l_knee, radius: 0.075, color: c_left_upper },
        CapsuleSeg { p0: p_l_knee, p1: p_l_ankle, radius: 0.065, color: c_left_lower },
        CapsuleSeg { p0: p_l_ankle, p1: p_l_toe, radius: 0.060, color: c_left_lower },
        CapsuleSeg { p0: p_l_shoulder, p1: p_l_elbow, radius: 0.065, color: c_left_upper },
        CapsuleSeg { p0: p_l_elbow, p1: p_l_hand, radius: 0.058, color: c_left_lower },
        // Torso & head
        CapsuleSeg { p0: p_l_hip, p1: p_r_hip, radius: 0.075, color: c_torso },
        CapsuleSeg { p0: p_hips, p1: p_spine, radius: 0.115 * ts, color: c_torso },
        CapsuleSeg { p0: p_spine, p1: p_chest, radius: 0.125 * ts, color: c_torso },
        CapsuleSeg { p0: p_l_shoulder, p1: p_r_shoulder, radius: 0.075, color: c_torso },
        CapsuleSeg { p0: head_center, p1: head_center, radius: 0.185 * hs, color: c_head },
        CapsuleSeg { p0: head_center, p1: head_nose, radius: 0.048 * hs, color: c_visor },
        // Right limbs
        CapsuleSeg { p0: p_r_hip, p1: p_r_knee, radius: 0.075, color: c_right_upper },
        CapsuleSeg { p0: p_r_knee, p1: p_r_ankle, radius: 0.065, color: c_right_lower },
        CapsuleSeg { p0: p_r_ankle, p1: p_r_toe, radius: 0.060, color: c_right_lower },
        CapsuleSeg { p0: p_r_shoulder, p1: p_r_elbow, radius: 0.065, color: c_right_upper },
        CapsuleSeg { p0: p_r_elbow, p1: p_r_hand, radius: 0.058, color: c_right_lower },
    ];

    let wt = prop.weapon_type.trim().to_lowercase();
    if wt != "none" && !wt.is_empty() {
        let r_weap = r_r_farm.mul(rot("weapon"));
        let len = if wt == "spear" {
            1.15 * ws
        } else if wt == "staff" {
            1.02 * ws
        } else {
            0.78 * ws
        };
        let p_w_tip = p_r_hand.add(r_weap.mul_vec(Vec3::new(0.0, 0.12 * ws, len)));
        let p_w_pommel = p_r_hand.add(r_weap.mul_vec(Vec3::new(0.0, -0.04 * ws, -0.14 * ws)));
        segs.push(CapsuleSeg {
            p0: p_w_pommel,
            p1: p_w_tip,
            radius: 0.042 * ws,
            color: c_weapon,
        });
    }

    segs
}

fn project_point(p: Vec3, cam: &PoseCameraConfig, cell_w: f32, cell_h: f32) -> (f32, f32, f32, f32) {
    // Rotate world around center (0, 0.85, 0) matching Three.js camera at (sin(yaw), sin(pitch), cos(yaw))
    let pivot_y = 0.85_f32;
    let vx = p.x;
    let vy = p.y - pivot_y;
    let vz = p.z;

    let yaw_rad = cam.yaw.to_radians();
    let pitch_rad = cam.pitch.to_radians();
    let (sy, cy) = yaw_rad.sin_cos();
    let (sp, cp) = pitch_rad.sin_cos();

    // Yaw around Y: when yaw=-90 deg (side_2d facing right), world +Z (forward) projects to screen +X (right)
    let x1 = vx * cy - vz * sy;
    let z1 = vx * sy + vz * cy;
    let y1 = vy;

    // Pitch around X
    let x2 = x1;
    let y2 = y1 * cp - z1 * sp;
    let z2 = y1 * sp + z1 * cp;

    let zoom = cam.zoom.clamp(0.4, 2.5);
    let base_scale = cell_w.min(cell_h) * 0.36 * zoom;
    let persp = if cam.ortho {
        1.0
    } else {
        (3.5 / (3.5 - z2).max(1.2)).clamp(0.5, 1.8)
    };
    let scale = base_scale * persp;

    let cx = cell_w * 0.5;
    let cy_screen = cell_h * 0.54;
    let sx = cx + x2 * scale;
    let sy_screen = cy_screen - y2 * scale;
    (sx, sy_screen, z2, scale)
}

fn draw_capsule_2d(
    img: &mut RgbaImage,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    r: f32,
    fill: [u8; 4],
    clip_rect: (u32, u32, u32, u32),
) {
    let outline = [15_u8, 23, 42, 255];
    let r_outer = (r + 1.5).max(2.0);
    let min_x = ((x0.min(x1) - r_outer - 1.0).floor() as i32).max(clip_rect.0 as i32) as u32;
    let max_x = ((x0.max(x1) + r_outer + 1.0).ceil() as i32)
        .min((clip_rect.0 + clip_rect.2).saturating_sub(1) as i32)
        .max(0) as u32;
    let min_y = ((y0.min(y1) - r_outer - 1.0).floor() as i32).max(clip_rect.1 as i32) as u32;
    let max_y = ((y0.max(y1) + r_outer + 1.0).ceil() as i32)
        .min((clip_rect.1 + clip_rect.3).saturating_sub(1) as i32)
        .max(0) as u32;

    let dx = x1 - x0;
    let dy = y1 - y0;
    let len_sq = dx * dx + dy * dy;

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let fx = px as f32 + 0.5;
            let fy = py as f32 + 0.5;
            let t = if len_sq > 1e-5 {
                (((fx - x0) * dx + (fy - y0) * dy) / len_sq).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let cx = x0 + t * dx;
            let cy = y0 + t * dy;
            let dist = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            if dist <= r {
                img.put_pixel(px, py, Rgba(fill));
            } else if dist <= r_outer {
                img.put_pixel(px, py, Rgba(outline));
            }
        }
    }
}

pub fn render_pose_clip_grid_png(
    clip: &PoseActionClip,
    rows: u32,
    cols: u32,
    width: u32,
    height: u32,
) -> AppResult<Vec<u8>> {
    let rows = rows.clamp(1, 8);
    let cols = cols.clamp(1, 12);
    let width = width.clamp(256, 4096);
    let height = height.clamp(256, 4096);

    let mut img = RgbaImage::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let cell_w = width / cols;
    let cell_h = height / rows;
    let total = (rows * cols) as usize;

    for idx in 0..total {
        let r = (idx as u32) / cols;
        let c = (idx as u32) % cols;
        let ox = c * cell_w;
        let oy = r * cell_h;

        // Draw subtle dashed safe-zone box & ground baseline
        let pad_x = (cell_w as f32 * 0.08).round() as u32;
        let pad_y = (cell_h as f32 * 0.08).round() as u32;
        let bx0 = ox + pad_x;
        let by0 = oy + pad_y;
        let bx1 = (ox + cell_w).saturating_sub(pad_x);
        let by1 = (oy + cell_h).saturating_sub(pad_y);

        for x in bx0..bx1 {
            if ((x - bx0) / 6) % 2 == 0 {
                img.put_pixel(x, by0.min(height - 1), Rgba([203, 213, 225, 255]));
                img.put_pixel(x, by1.min(height - 1), Rgba([203, 213, 225, 255]));
            }
        }
        for y in by0..by1 {
            if ((y - by0) / 6) % 2 == 0 {
                img.put_pixel(bx0.min(width - 1), y, Rgba([203, 213, 225, 255]));
                img.put_pixel(bx1.min(width - 1), y, Rgba([203, 213, 225, 255]));
            }
        }

        // Ground baseline projected at y = 0.0
        let (_, gy_rel, _, _) =
            project_point(Vec3::new(0.0, 0.0, 0.0), &clip.camera, cell_w as f32, cell_h as f32);
        let gy = (oy as f32 + gy_rel).round().clamp(oy as f32, (oy + cell_h - 1) as f32) as u32;
        for x in (ox + pad_x / 2)..((ox + cell_w).saturating_sub(pad_x / 2)) {
            if ((x - ox) / 4) % 2 == 0 {
                img.put_pixel(x.min(width - 1), gy.min(height - 1), Rgba([148, 163, 184, 255]));
            }
        }

        let t = if total <= 1 {
            0.0
        } else if clip.loop_anim {
            (idx as f32) / (total as f32)
        } else {
            (idx as f32) / ((total - 1) as f32)
        };
        let kf = sample_clip_at(clip, t);
        let segs = build_stickman_capsules(&kf, &clip.proportion);

        let mut projected: Vec<(f32, f32, f32, f32, f32, f32, [u8; 4])> = segs
            .into_iter()
            .map(|s| {
                let (x0, y0, z0, sc0) =
                    project_point(s.p0, &clip.camera, cell_w as f32, cell_h as f32);
                let (x1, y1, z1, sc1) =
                    project_point(s.p1, &clip.camera, cell_w as f32, cell_h as f32);
                let avg_z = 0.5 * (z0 + z1);
                let r_px = (s.radius * 0.5 * (sc0 + sc1)).max(2.5);
                (
                    avg_z,
                    ox as f32 + x0,
                    oy as f32 + y0,
                    ox as f32 + x1,
                    oy as f32 + y1,
                    r_px,
                    s.color,
                )
            })
            .collect();

        // Sort back-to-front by avg_z
        projected.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        for (_, x0, y0, x1, y1, r_px, color) in projected {
            draw_capsule_2d(&mut img, x0, y0, x1, y1, r_px, color, (ox, oy, cell_w, cell_h));
        }
    }

    let mut out = Vec::new();
    let mut cursor = Cursor::new(&mut out);
    DynamicImage::ImageRgba8(img)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| AppError::new(AppErrorCode::Io, e.to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_pose_clips_and_jump_ko_elevation() {
        let clips = builtin_pose_clips();
        assert_eq!(clips.len(), 8);
        let jump = clips.iter().find(|c| c.id == "jump").unwrap();
        let apex = sample_clip_at(jump, 0.5);
        assert!(
            apex.root_pos[1] > 0.04 && apex.root_pos[1] <= 0.15,
            "In-place engine jump apex should stay near collider origin (0.04..=0.15), got {}",
            apex.root_pos[1]
        );
        assert!(
            apex.bones.get("l_thigh").unwrap()[0] <= -50.0,
            "Jump apex should have tucked knees"
        );
        let start = sample_clip_at(jump, 0.0);
        assert!(start.root_pos[1] < -0.1, "jump start should crouch < -0.1");

        let ko = clips.iter().find(|c| c.id == "ko").unwrap();
        let end = sample_clip_at(ko, 1.0);
        assert!(
            (end.root_pos[1] - (-0.75)).abs() < 0.05,
            "ko end should be flat on ground"
        );
        assert!(
            end.bones.get("hips").unwrap()[0] >= 80.0,
            "ko hipsX should be ~85 deg"
        );
    }

    #[test]
    fn test_render_pose_clip_grid_png_outputs_valid_image() {
        let clips = builtin_pose_clips();
        let attack = clips.iter().find(|c| c.id == "attack").unwrap();
        let png = render_pose_clip_grid_png(attack, 2, 3, 600, 400).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(decoded.dimensions(), (600, 400));
        // Ensure stickman non-white pixels were rendered inside cell (0,0)
        let mut non_white = 0;
        for y in 20..180 {
            for x in 20..180 {
                let p = decoded.get_pixel(x, y);
                if p[0] < 240 || p[1] < 240 || p[2] < 240 {
                    non_white += 1;
                }
            }
        }
        assert!(non_white > 100, "expected stickman pixels in cell 0, got {}", non_white);
    }

    #[test]
    fn test_side_2d_default_yaw_minus_90_faces_right() {
        let cam = PoseCameraConfig::default();
        assert_eq!(cam.preset, "side_2d");
        assert!((cam.yaw - (-90.0)).abs() < 1e-4);

        let (sx_origin, _, _, _) = project_point(Vec3::new(0.0, 0.85, 0.0), &cam, 200.0, 200.0);
        let (sx_forward, _, _, _) = project_point(Vec3::new(0.0, 0.85, 1.0), &cam, 200.0, 200.0);
        assert!(
            sx_forward > sx_origin + 20.0,
            "At yaw=-90 (side_2d), +Z forward should project to +screenX (right): origin={}, forward={}",
            sx_origin,
            sx_forward
        );

        let mut cam_left = PoseCameraConfig::default();
        apply_camera_preset(&mut cam_left, "side_2d_left");
        assert!((cam_left.yaw - 90.0).abs() < 1e-4);
        let (sx_left_fwd, _, _, _) =
            project_point(Vec3::new(0.0, 0.85, 1.0), &cam_left, 200.0, 200.0);
        assert!(
            sx_left_fwd < sx_origin - 20.0,
            "At yaw=+90 (side_2d_left), +Z forward should project to -screenX (left)"
        );
    }
}
