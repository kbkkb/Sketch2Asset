import * as THREE from 'three';
import type {
  PoseActionClip,
  PoseCameraConfig,
  PoseKeyframe,
  PoseProportionConfig
} from '$lib/api/types';

export type BoneId =
  | 'hips'
  | 'spine'
  | 'chest'
  | 'head'
  | 'l_upper_arm'
  | 'l_fore_arm'
  | 'r_upper_arm'
  | 'r_fore_arm'
  | 'weapon'
  | 'l_thigh'
  | 'l_shin'
  | 'l_foot'
  | 'r_thigh'
  | 'r_shin'
  | 'r_foot'
  | 'neck';

export interface BoneMeta {
  id: BoneId;
  label: string;
  labelEn: string;
  group: 'torso' | 'left' | 'right' | 'weapon';
  color: string;
}

export const BONE_LIST: BoneMeta[] = [
  { id: 'hips', label: '骨盆重心 (Hips)', labelEn: 'Hips (Root)', group: 'torso', color: '#E2E8F0' },
  { id: 'spine', label: '腰腹脊柱 (Spine)', labelEn: 'Spine', group: 'torso', color: '#E2E8F0' },
  { id: 'chest', label: '胸腔肩部 (Chest)', labelEn: 'Chest', group: 'torso', color: '#E2E8F0' },
  { id: 'neck', label: '颈部 (Neck)', labelEn: 'Neck', group: 'torso', color: '#CBD5E1' },
  { id: 'head', label: '头部朝向 (Head)', labelEn: 'Head', group: 'torso', color: '#F8FAFC' },
  { id: 'l_upper_arm', label: '左大臂 (L Upper Arm)', labelEn: 'L Upper Arm', group: 'left', color: '#00E5FF' },
  { id: 'l_fore_arm', label: '左前臂 (L Forearm)', labelEn: 'L Forearm', group: 'left', color: '#38BDF8' },
  { id: 'r_upper_arm', label: '右大臂 (R Upper Arm)', labelEn: 'R Upper Arm', group: 'right', color: '#FF6B6B' },
  { id: 'r_fore_arm', label: '右前臂 (R Forearm)', labelEn: 'R Forearm', group: 'right', color: '#F59E0B' },
  { id: 'weapon', label: '右手武器 (Weapon)', labelEn: 'Right Hand Weapon', group: 'weapon', color: '#C8FF3D' },
  { id: 'l_thigh', label: '左大腿 (L Thigh)', labelEn: 'L Thigh', group: 'left', color: '#00E5FF' },
  { id: 'l_shin', label: '左小腿 (L Shin)', labelEn: 'L Shin', group: 'left', color: '#38BDF8' },
  { id: 'l_foot', label: '左脚掌 (L Foot)', labelEn: 'L Foot', group: 'left', color: '#0284C7' },
  { id: 'r_thigh', label: '右大腿 (R Thigh)', labelEn: 'R Thigh', group: 'right', color: '#FF6B6B' },
  { id: 'r_shin', label: '右小腿 (R Shin)', labelEn: 'R Shin', group: 'right', color: '#F59E0B' },
  { id: 'r_foot', label: '右脚掌 (R Foot)', labelEn: 'R Foot', group: 'right', color: '#EA580C' }
];

export interface CameraPresetOption {
  id: string;
  label: string;
  labelEn: string;
  yaw: number;
  pitch: number;
  ortho: boolean;
  zoom: number;
}

export const CAMERA_PRESETS: CameraPresetOption[] = [
  {
    id: 'side_2d',
    label: '2D横版 (朝右)',
    labelEn: '2D Side (Right)',
    yaw: -90,
    pitch: 0,
    ortho: true,
    zoom: 1.0
  },
  {
    id: 'side_2d_left',
    label: '2D横版 (朝左)',
    labelEn: '2D Side (Left)',
    yaw: 90,
    pitch: 0,
    ortho: true,
    zoom: 1.0
  },
  {
    id: 'hd2d_45',
    label: 'HD-2D 斜45°',
    labelEn: 'HD-2D 45°',
    yaw: -55,
    pitch: 25,
    ortho: true,
    zoom: 1.0
  },
  {
    id: 'topdown_34',
    label: '3/4 俯视',
    labelEn: '3/4 Top-Down',
    yaw: -35,
    pitch: 45,
    ortho: true,
    zoom: 1.0
  },
  {
    id: 'front_view',
    label: '正视图',
    labelEn: 'Front View',
    yaw: 0,
    pitch: 5,
    ortho: true,
    zoom: 1.0
  },
  {
    id: 'top_pure',
    label: '纯顶视',
    labelEn: 'Top View',
    yaw: 0,
    pitch: 82,
    ortho: true,
    zoom: 1.0
  }
];

export interface ProportionPresetOption {
  id: string;
  label: string;
  labelEn: string;
  headScale: number;
  torsoScale: number;
  limbScale: number;
}

export const PROPORTION_PRESETS: ProportionPresetOption[] = [
  {
    id: 'chibi_2',
    label: '2头身 Q版',
    labelEn: '2-Head Chibi',
    headScale: 1.8,
    torsoScale: 0.78,
    limbScale: 0.65
  },
  {
    id: 'stylized_3',
    label: '3头身',
    labelEn: '3-Head Stylized',
    headScale: 1.38,
    torsoScale: 0.9,
    limbScale: 0.82
  },
  {
    id: 'standard_6',
    label: '常规6头身',
    labelEn: '6-Head Standard',
    headScale: 1.0,
    torsoScale: 1.0,
    limbScale: 1.0
  }
];

export const WEAPON_TYPES = [
  { id: 'none', label: '无武器', labelEn: 'Unarmed' },
  { id: 'sword', label: '长剑 / 太刀', labelEn: 'Sword / Katana' },
  { id: 'spear', label: '长枪 / 战戟', labelEn: 'Spear / Polearm' },
  { id: 'staff', label: '法杖 / 魔杖', labelEn: 'Magic Staff' }
];

export function defaultCameraConfig(presetId = 'side_2d'): PoseCameraConfig {
  const found = CAMERA_PRESETS.find((c) => c.id === presetId) ?? CAMERA_PRESETS[0]!;
  return {
    preset: found.id,
    yaw: found.yaw,
    pitch: found.pitch,
    ortho: found.ortho,
    zoom: found.zoom
  };
}

export function defaultProportionConfig(
  presetId = 'stylized_3',
  weaponType = 'sword'
): PoseProportionConfig {
  const found = PROPORTION_PRESETS.find((p) => p.id === presetId) ?? PROPORTION_PRESETS[1]!;
  return {
    preset: found.id,
    headScale: found.headScale,
    torsoScale: found.torsoScale,
    limbScale: found.limbScale,
    weaponType,
    weaponScale: 1.0
  };
}

function kf(
  t: number,
  rootPos: [number, number, number],
  bones: Record<string, [number, number, number]>
): PoseKeyframe {
  return { t, rootPos, bones };
}

export const BUILTIN_POSE_CLIPS: PoseActionClip[] = [
  {
    id: 'idle',
    name: '待机呼吸 (Idle)',
    nameEn: 'Idle Breathing',
    category: 'locomotion',
    builtin: true,
    loopAnim: true,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'sword'),
    keyframes: [
      kf(0.0, [0, 0, 0], {
        hips: [0, -12, 0],
        spine: [4, 0, 0],
        chest: [4, 10, 0],
        head: [-4, 4, 0],
        l_upper_arm: [16, 0, 12],
        l_fore_arm: [-28, 0, 0],
        r_upper_arm: [-14, 0, -12],
        r_fore_arm: [-35, 0, 0],
        weapon: [12, 0, 0],
        l_thigh: [-10, 0, 3],
        l_shin: [18, 0, 0],
        l_foot: [-8, 0, 0],
        r_thigh: [12, 0, -3],
        r_shin: [14, 0, 0],
        r_foot: [-14, 0, 0]
      }),
      kf(0.5, [0, -0.05, 0], {
        hips: [0, -12, 0],
        spine: [8, 0, 0],
        chest: [7, 10, 0],
        head: [-2, 4, 0],
        l_upper_arm: [22, 0, 15],
        l_fore_arm: [-36, 0, 0],
        r_upper_arm: [-8, 0, -15],
        r_fore_arm: [-42, 0, 0],
        weapon: [18, 0, 0],
        l_thigh: [-18, 0, 3],
        l_shin: [30, 0, 0],
        l_foot: [-12, 0, 0],
        r_thigh: [6, 0, -3],
        r_shin: [24, 0, 0],
        r_foot: [-18, 0, 0]
      }),
      kf(1.0, [0, 0, 0], {
        hips: [0, -12, 0],
        spine: [4, 0, 0],
        chest: [4, 10, 0],
        head: [-4, 4, 0],
        l_upper_arm: [16, 0, 12],
        l_fore_arm: [-28, 0, 0],
        r_upper_arm: [-14, 0, -12],
        r_fore_arm: [-35, 0, 0],
        weapon: [12, 0, 0],
        l_thigh: [-10, 0, 3],
        l_shin: [18, 0, 0],
        l_foot: [-8, 0, 0],
        r_thigh: [12, 0, -3],
        r_shin: [14, 0, 0],
        r_foot: [-14, 0, 0]
      })
    ]
  },
  {
    id: 'walk',
    name: '标准行走 (Walk)',
    nameEn: 'Walk Cycle',
    category: 'locomotion',
    builtin: true,
    loopAnim: true,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'none'),
    keyframes: [
      kf(0.0, [0, 0.0, 0], {
        hips: [4, -6, 0],
        spine: [6, 4, 0],
        chest: [6, 8, 0],
        head: [-6, 0, 0],
        l_upper_arm: [-36, 0, 8],
        l_fore_arm: [-28, 0, 0],
        r_upper_arm: [34, 0, -8],
        r_fore_arm: [-24, 0, 0],
        l_thigh: [34, 0, 0],
        l_shin: [12, 0, 0],
        l_foot: [-18, 0, 0],
        r_thigh: [-32, 0, 0],
        r_shin: [8, 0, 0],
        r_foot: [16, 0, 0]
      }),
      kf(0.25, [0, -0.05, 0], {
        hips: [5, 0, 0],
        spine: [7, 0, 0],
        chest: [7, 0, 0],
        head: [-6, 0, 0],
        l_upper_arm: [-10, 0, 8],
        l_fore_arm: [-35, 0, 0],
        r_upper_arm: [10, 0, -8],
        r_fore_arm: [-32, 0, 0],
        l_thigh: [8, 0, 0],
        l_shin: [42, 0, 0],
        l_foot: [-10, 0, 0],
        r_thigh: [-12, 0, 0],
        r_shin: [24, 0, 0],
        r_foot: [-6, 0, 0]
      }),
      kf(0.5, [0, 0.0, 0], {
        hips: [4, 6, 0],
        spine: [6, -4, 0],
        chest: [6, -8, 0],
        head: [-6, 0, 0],
        l_upper_arm: [34, 0, 8],
        l_fore_arm: [-24, 0, 0],
        r_upper_arm: [-36, 0, -8],
        r_fore_arm: [-28, 0, 0],
        l_thigh: [-32, 0, 0],
        l_shin: [8, 0, 0],
        l_foot: [16, 0, 0],
        r_thigh: [34, 0, 0],
        r_shin: [12, 0, 0],
        r_foot: [-18, 0, 0]
      }),
      kf(0.75, [0, -0.05, 0], {
        hips: [5, 0, 0],
        spine: [7, 0, 0],
        chest: [7, 0, 0],
        head: [-6, 0, 0],
        l_upper_arm: [10, 0, 8],
        l_fore_arm: [-32, 0, 0],
        r_upper_arm: [-10, 0, -8],
        r_fore_arm: [-35, 0, 0],
        l_thigh: [-12, 0, 0],
        l_shin: [24, 0, 0],
        l_foot: [-6, 0, 0],
        r_thigh: [8, 0, 0],
        r_shin: [42, 0, 0],
        r_foot: [-10, 0, 0]
      }),
      kf(1.0, [0, 0.0, 0], {
        hips: [4, -6, 0],
        spine: [6, 4, 0],
        chest: [6, 8, 0],
        head: [-6, 0, 0],
        l_upper_arm: [-36, 0, 8],
        l_fore_arm: [-28, 0, 0],
        r_upper_arm: [34, 0, -8],
        r_fore_arm: [-24, 0, 0],
        l_thigh: [34, 0, 0],
        l_shin: [12, 0, 0],
        l_foot: [-18, 0, 0],
        r_thigh: [-32, 0, 0],
        r_shin: [8, 0, 0],
        r_foot: [16, 0, 0]
      })
    ]
  },
  {
    id: 'run',
    name: '高速奔跑 (Run)',
    nameEn: 'Dynamic Run Cycle',
    category: 'locomotion',
    builtin: true,
    loopAnim: true,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'sword'),
    keyframes: [
      kf(0.0, [0, 0.08, 0], {
        hips: [18, -10, 0],
        spine: [14, 6, 0],
        chest: [12, 10, 0],
        head: [-18, 0, 0],
        l_upper_arm: [-62, 0, 12],
        l_fore_arm: [-68, 0, 0],
        r_upper_arm: [56, 0, -12],
        r_fore_arm: [-60, 0, 0],
        weapon: [25, 0, 0],
        l_thigh: [58, 0, 0],
        l_shin: [24, 0, 0],
        l_foot: [-24, 0, 0],
        r_thigh: [-56, 0, 0],
        r_shin: [16, 0, 0],
        r_foot: [22, 0, 0]
      }),
      kf(0.25, [0, -0.08, 0], {
        hips: [20, 0, 0],
        spine: [16, 0, 0],
        chest: [14, 0, 0],
        head: [-20, 0, 0],
        l_upper_arm: [-12, 0, 12],
        l_fore_arm: [-78, 0, 0],
        r_upper_arm: [12, 0, -12],
        r_fore_arm: [-72, 0, 0],
        weapon: [10, 0, 0],
        l_thigh: [14, 0, 0],
        l_shin: [82, 0, 0],
        l_foot: [-12, 0, 0],
        r_thigh: [-20, 0, 0],
        r_shin: [42, 0, 0],
        r_foot: [-15, 0, 0]
      }),
      kf(0.5, [0, 0.08, 0], {
        hips: [18, 10, 0],
        spine: [14, -6, 0],
        chest: [12, -10, 0],
        head: [-18, 0, 0],
        l_upper_arm: [56, 0, 12],
        l_fore_arm: [-60, 0, 0],
        r_upper_arm: [-62, 0, -12],
        r_fore_arm: [-68, 0, 0],
        weapon: [35, 0, 0],
        l_thigh: [-56, 0, 0],
        l_shin: [16, 0, 0],
        l_foot: [22, 0, 0],
        r_thigh: [58, 0, 0],
        r_shin: [24, 0, 0],
        r_foot: [-24, 0, 0]
      }),
      kf(0.75, [0, -0.08, 0], {
        hips: [20, 0, 0],
        spine: [16, 0, 0],
        chest: [14, 0, 0],
        head: [-20, 0, 0],
        l_upper_arm: [12, 0, 12],
        l_fore_arm: [-72, 0, 0],
        r_upper_arm: [-12, 0, -12],
        r_fore_arm: [-78, 0, 0],
        weapon: [15, 0, 0],
        l_thigh: [-20, 0, 0],
        l_shin: [42, 0, 0],
        l_foot: [-15, 0, 0],
        r_thigh: [14, 0, 0],
        r_shin: [82, 0, 0],
        r_foot: [-12, 0, 0]
      }),
      kf(1.0, [0, 0.08, 0], {
        hips: [18, -10, 0],
        spine: [14, 6, 0],
        chest: [12, 10, 0],
        head: [-18, 0, 0],
        l_upper_arm: [-62, 0, 12],
        l_fore_arm: [-68, 0, 0],
        r_upper_arm: [56, 0, -12],
        r_fore_arm: [-60, 0, 0],
        weapon: [25, 0, 0],
        l_thigh: [58, 0, 0],
        l_shin: [24, 0, 0],
        l_foot: [-24, 0, 0],
        r_thigh: [-56, 0, 0],
        r_shin: [16, 0, 0],
        r_foot: [22, 0, 0]
      })
    ]
  },
  {
    id: 'jump',
    name: '跳跃起落 (引擎原地)',
    nameEn: 'Jump Velocity States (In-Place)',
    category: 'locomotion',
    builtin: true,
    loopAnim: false,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'none'),
    keyframes: [
      kf(0.0, [0, -0.18, 0], {
        hips: [24, 0, 0],
        spine: [18, 0, 0],
        chest: [12, 0, 0],
        head: [-24, 0, 0],
        l_upper_arm: [42, 0, 14],
        l_fore_arm: [-45, 0, 0],
        r_upper_arm: [42, 0, -14],
        r_fore_arm: [-45, 0, 0],
        l_thigh: [-52, 0, 0],
        l_shin: [88, 0, 0],
        l_foot: [-32, 0, 0],
        r_thigh: [-46, 0, 0],
        r_shin: [82, 0, 0],
        r_foot: [-30, 0, 0]
      }),
      kf(0.28, [0, 0.06, 0], {
        hips: [-8, 0, 0],
        spine: [-6, 0, 0],
        chest: [-6, 0, 0],
        head: [10, 0, 0],
        l_upper_arm: [-110, 0, 18],
        l_fore_arm: [-25, 0, 0],
        r_upper_arm: [-100, 0, -18],
        r_fore_arm: [-25, 0, 0],
        l_thigh: [-38, 0, 0],
        l_shin: [64, 0, 0],
        l_foot: [18, 0, 0],
        r_thigh: [24, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [28, 0, 0]
      }),
      kf(0.55, [0, 0.08, 0], {
        hips: [6, 0, 0],
        spine: [8, 0, 0],
        chest: [6, 0, 0],
        head: [-8, 0, 0],
        l_upper_arm: [-65, 0, 26],
        l_fore_arm: [-48, 0, 0],
        r_upper_arm: [-55, 0, -26],
        r_fore_arm: [-48, 0, 0],
        l_thigh: [-58, 0, 0],
        l_shin: [82, 0, 0],
        l_foot: [-12, 0, 0],
        r_thigh: [-32, 0, 0],
        r_shin: [66, 0, 0],
        r_foot: [10, 0, 0]
      }),
      kf(0.8, [0, 0.04, 0], {
        hips: [12, 0, 0],
        spine: [10, 0, 0],
        chest: [8, 0, 0],
        head: [-10, 0, 0],
        l_upper_arm: [-20, 0, 22],
        l_fore_arm: [-30, 0, 0],
        r_upper_arm: [-15, 0, -22],
        r_fore_arm: [-30, 0, 0],
        l_thigh: [-18, 0, 0],
        l_shin: [24, 0, 0],
        l_foot: [20, 0, 0],
        r_thigh: [-8, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [24, 0, 0]
      }),
      kf(1.0, [0, -0.16, 0], {
        hips: [20, 0, 0],
        spine: [14, 0, 0],
        chest: [10, 0, 0],
        head: [-18, 0, 0],
        l_upper_arm: [26, 0, 16],
        l_fore_arm: [-42, 0, 0],
        r_upper_arm: [26, 0, -16],
        r_fore_arm: [-42, 0, 0],
        l_thigh: [-46, 0, 0],
        l_shin: [76, 0, 0],
        l_foot: [-26, 0, 0],
        r_thigh: [-38, 0, 0],
        r_shin: [68, 0, 0],
        r_foot: [-24, 0, 0]
      })
    ]
  },
  {
    id: 'attack',
    name: '近战挥砍 (Attack)',
    nameEn: 'Melee Slash Combo',
    category: 'combat',
    builtin: true,
    loopAnim: false,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'sword'),
    keyframes: [
      kf(0.0, [-0.1, -0.06, 0], {
        hips: [-8, -28, 0],
        spine: [-10, -22, 0],
        chest: [-12, -26, 0],
        head: [12, 38, 0],
        l_upper_arm: [28, 0, 22],
        l_fore_arm: [-42, 0, 0],
        r_upper_arm: [-125, -20, -24],
        r_fore_arm: [-48, 0, 0],
        weapon: [-28, 0, 0],
        l_thigh: [-34, 0, 0],
        l_shin: [48, 0, 0],
        l_foot: [-14, 0, 0],
        r_thigh: [28, 0, 0],
        r_shin: [24, 0, 0],
        r_foot: [-22, 0, 0]
      }),
      kf(0.35, [-0.04, -0.02, 0], {
        hips: [10, -10, 0],
        spine: [12, 10, 0],
        chest: [14, 18, 0],
        head: [-10, -8, 0],
        l_upper_arm: [36, 0, 25],
        l_fore_arm: [-35, 0, 0],
        r_upper_arm: [-85, 10, -18],
        r_fore_arm: [-22, 0, 0],
        weapon: [15, 0, 0],
        l_thigh: [-42, 0, 0],
        l_shin: [56, 0, 0],
        l_foot: [-14, 0, 0],
        r_thigh: [36, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [-20, 0, 0]
      }),
      kf(0.65, [0.16, -0.1, 0], {
        hips: [24, 26, 0],
        spine: [22, 28, 0],
        chest: [20, 32, 0],
        head: [-22, -32, 0],
        l_upper_arm: [48, 0, 28],
        l_fore_arm: [-38, 0, 0],
        r_upper_arm: [32, 24, -18],
        r_fore_arm: [-12, 0, 0],
        weapon: [62, 0, 0],
        l_thigh: [-52, 0, 0],
        l_shin: [68, 0, 0],
        l_foot: [-18, 0, 0],
        r_thigh: [48, 0, 0],
        r_shin: [14, 0, 0],
        r_foot: [-14, 0, 0]
      }),
      kf(1.0, [0.04, -0.02, 0], {
        hips: [8, 6, 0],
        spine: [6, 4, 0],
        chest: [6, 6, 0],
        head: [-6, -6, 0],
        l_upper_arm: [20, 0, 16],
        l_fore_arm: [-32, 0, 0],
        r_upper_arm: [-10, 0, -16],
        r_fore_arm: [-34, 0, 0],
        weapon: [20, 0, 0],
        l_thigh: [-20, 0, 0],
        l_shin: [30, 0, 0],
        l_foot: [-10, 0, 0],
        r_thigh: [18, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [-14, 0, 0]
      })
    ]
  },
  {
    id: 'cast',
    name: '技能施法 (Cast)',
    nameEn: 'Spellcasting',
    category: 'combat',
    builtin: true,
    loopAnim: false,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'staff'),
    keyframes: [
      kf(0.0, [0, -0.04, 0], {
        hips: [6, -12, 0],
        spine: [8, -8, 0],
        chest: [10, -8, 0],
        head: [-8, 10, 0],
        l_upper_arm: [-35, 20, 18],
        l_fore_arm: [-65, 0, 0],
        r_upper_arm: [-45, -15, -18],
        r_fore_arm: [-55, 0, 0],
        weapon: [-15, 0, 0],
        l_thigh: [-18, 0, 0],
        l_shin: [28, 0, 0],
        l_foot: [-10, 0, 0],
        r_thigh: [16, 0, 0],
        r_shin: [20, 0, 0],
        r_foot: [-12, 0, 0]
      }),
      kf(0.45, [0, 0.14, 0], {
        hips: [-10, 0, 0],
        spine: [-12, 0, 0],
        chest: [-14, 0, 0],
        head: [-16, 0, 0],
        l_upper_arm: [-115, 0, 24],
        l_fore_arm: [-20, 0, 0],
        r_upper_arm: [-120, 0, -24],
        r_fore_arm: [-18, 0, 0],
        weapon: [-8, 0, 0],
        l_thigh: [12, 0, 0],
        l_shin: [22, 0, 0],
        l_foot: [18, 0, 0],
        r_thigh: [-8, 0, 0],
        r_shin: [36, 0, 0],
        r_foot: [20, 0, 0]
      }),
      kf(0.75, [0.1, -0.04, 0], {
        hips: [16, 14, 0],
        spine: [18, 16, 0],
        chest: [16, 20, 0],
        head: [-18, -18, 0],
        l_upper_arm: [-68, 0, 16],
        l_fore_arm: [-12, 0, 0],
        r_upper_arm: [-76, 0, -14],
        r_fore_arm: [-8, 0, 0],
        weapon: [48, 0, 0],
        l_thigh: [-38, 0, 0],
        l_shin: [52, 0, 0],
        l_foot: [-14, 0, 0],
        r_thigh: [32, 0, 0],
        r_shin: [16, 0, 0],
        r_foot: [-16, 0, 0]
      }),
      kf(1.0, [0, 0, 0], {
        hips: [4, 0, 0],
        spine: [4, 0, 0],
        chest: [4, 0, 0],
        head: [-4, 0, 0],
        l_upper_arm: [-18, 0, 14],
        l_fore_arm: [-36, 0, 0],
        r_upper_arm: [-24, 0, -14],
        r_fore_arm: [-38, 0, 0],
        weapon: [8, 0, 0],
        l_thigh: [-12, 0, 0],
        l_shin: [20, 0, 0],
        l_foot: [-8, 0, 0],
        r_thigh: [12, 0, 0],
        r_shin: [16, 0, 0],
        r_foot: [-12, 0, 0]
      })
    ]
  },
  {
    id: 'hurt',
    name: '受击硬直 (Hurt)',
    nameEn: 'Hit Flinch',
    category: 'reaction',
    builtin: true,
    loopAnim: false,
    defaultRows: 1,
    defaultCols: 4,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'sword'),
    keyframes: [
      kf(0.0, [0, 0, 0], {
        hips: [0, 0, 0],
        spine: [4, 0, 0],
        chest: [4, 0, 0],
        head: [-4, 0, 0],
        l_upper_arm: [12, 0, 12],
        l_fore_arm: [-30, 0, 0],
        r_upper_arm: [-12, 0, -12],
        r_fore_arm: [-32, 0, 0],
        weapon: [10, 0, 0],
        l_thigh: [-10, 0, 0],
        l_shin: [18, 0, 0],
        l_foot: [-8, 0, 0],
        r_thigh: [10, 0, 0],
        r_shin: [16, 0, 0],
        r_foot: [-10, 0, 0]
      }),
      kf(0.35, [-0.18, 0.04, 0], {
        hips: [-22, -16, 0],
        spine: [-24, -12, 0],
        chest: [-20, -14, 0],
        head: [-26, 12, 0],
        l_upper_arm: [-68, 0, 38],
        l_fore_arm: [-35, 0, 0],
        r_upper_arm: [-52, 0, -38],
        r_fore_arm: [-28, 0, 0],
        weapon: [-25, 0, 0],
        l_thigh: [-32, 0, 0],
        l_shin: [48, 0, 0],
        l_foot: [14, 0, 0],
        r_thigh: [26, 0, 0],
        r_shin: [36, 0, 0],
        r_foot: [-18, 0, 0]
      }),
      kf(0.7, [-0.24, -0.1, 0], {
        hips: [16, -10, 0],
        spine: [18, -8, 0],
        chest: [14, -8, 0],
        head: [12, 8, 0],
        l_upper_arm: [-28, 0, 24],
        l_fore_arm: [-62, 0, 0],
        r_upper_arm: [18, 0, -24],
        r_fore_arm: [-52, 0, 0],
        weapon: [12, 0, 0],
        l_thigh: [-42, 0, 0],
        l_shin: [64, 0, 0],
        l_foot: [-22, 0, 0],
        r_thigh: [22, 0, 0],
        r_shin: [42, 0, 0],
        r_foot: [-24, 0, 0]
      }),
      kf(1.0, [-0.08, 0, 0], {
        hips: [6, -6, 0],
        spine: [6, -4, 0],
        chest: [6, -4, 0],
        head: [-6, 4, 0],
        l_upper_arm: [14, 0, 14],
        l_fore_arm: [-34, 0, 0],
        r_upper_arm: [-10, 0, -14],
        r_fore_arm: [-36, 0, 0],
        weapon: [12, 0, 0],
        l_thigh: [-14, 0, 0],
        l_shin: [24, 0, 0],
        l_foot: [-10, 0, 0],
        r_thigh: [14, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [-12, 0, 0]
      })
    ]
  },
  {
    id: 'ko',
    name: '击飞倒地 (KO)',
    nameEn: 'Knockout Fall',
    category: 'reaction',
    builtin: true,
    loopAnim: false,
    defaultRows: 2,
    defaultCols: 3,
    camera: defaultCameraConfig('side_2d'),
    proportion: defaultProportionConfig('stylized_3', 'none'),
    keyframes: [
      kf(0.0, [0, 0, 0], {
        hips: [-16, 0, 0],
        spine: [-18, 0, 0],
        chest: [-16, 0, 0],
        head: [-22, 0, 0],
        l_upper_arm: [-55, 0, 32],
        l_fore_arm: [-30, 0, 0],
        r_upper_arm: [-48, 0, -32],
        r_fore_arm: [-28, 0, 0],
        l_thigh: [-26, 0, 0],
        l_shin: [38, 0, 0],
        l_foot: [12, 0, 0],
        r_thigh: [18, 0, 0],
        r_shin: [28, 0, 0],
        r_foot: [-12, 0, 0]
      }),
      kf(0.4, [-0.22, 0.24, 0], {
        hips: [-48, 0, 0],
        spine: [-22, 0, 0],
        chest: [-18, 0, 0],
        head: [-24, 0, 0],
        l_upper_arm: [-85, 0, 36],
        l_fore_arm: [-20, 0, 0],
        r_upper_arm: [-75, 0, -36],
        r_fore_arm: [-20, 0, 0],
        l_thigh: [-32, 0, 0],
        l_shin: [45, 0, 0],
        l_foot: [20, 0, 0],
        r_thigh: [-12, 0, 0],
        r_shin: [35, 0, 0],
        r_foot: [18, 0, 0]
      }),
      kf(0.75, [-0.42, -0.52, 0], {
        hips: [-78, 0, 0],
        spine: [-10, 0, 0],
        chest: [-8, 0, 0],
        head: [14, 0, 0],
        l_upper_arm: [-110, 0, 28],
        l_fore_arm: [-15, 0, 0],
        r_upper_arm: [-95, 0, -28],
        r_fore_arm: [-18, 0, 0],
        l_thigh: [16, 0, 0],
        l_shin: [28, 0, 0],
        l_foot: [15, 0, 0],
        r_thigh: [24, 0, 0],
        r_shin: [18, 0, 0],
        r_foot: [12, 0, 0]
      }),
      kf(1.0, [-0.48, -0.68, 0], {
        hips: [-88, 0, 0],
        spine: [4, 0, 0],
        chest: [6, 0, 0],
        head: [18, 0, 0],
        l_upper_arm: [-135, 0, 22],
        l_fore_arm: [-10, 0, 0],
        r_upper_arm: [24, 0, -18],
        r_fore_arm: [-22, 0, 0],
        l_thigh: [8, 0, 0],
        l_shin: [12, 0, 0],
        l_foot: [20, 0, 0],
        r_thigh: [14, 0, 0],
        r_shin: [8, 0, 0],
        r_foot: [18, 0, 0]
      })
    ]
  }
];

export function cloneClip(clip: PoseActionClip): PoseActionClip {
  return JSON.parse(JSON.stringify(clip)) as PoseActionClip;
}

function lerp(a: number, b: number, alpha: number): number {
  return a + (b - a) * alpha;
}

function lerpAngleDeg(a: number, b: number, alpha: number): number {
  let diff = ((b - a + 540) % 360) - 180;
  return a + diff * alpha;
}

export function interpolatePoseAtTime(
  clip: PoseActionClip,
  rawT: number
): { rootPos: [number, number, number]; bones: Record<string, [number, number, number]> } {
  const kfs = [...(clip.keyframes || [])].sort((a, b) => a.t - b.t);
  if (kfs.length === 0) {
    return { rootPos: [0, 0, 0], bones: {} };
  }
  if (kfs.length === 1) {
    const only = kfs[0]!;
    return {
      rootPos: [...only.rootPos] as [number, number, number],
      bones: { ...only.bones }
    };
  }

  const t = Math.max(0, Math.min(1, rawT));
  let prev = kfs[0]!;
  let next = kfs[kfs.length - 1]!;

  for (let i = 0; i < kfs.length - 1; i++) {
    const a = kfs[i]!;
    const b = kfs[i + 1]!;
    if (t >= a.t && t <= b.t) {
      prev = a;
      next = b;
      break;
    }
  }

  if (t <= kfs[0]!.t) {
    prev = kfs[0]!;
    next = kfs[0]!;
  } else if (t >= kfs[kfs.length - 1]!.t) {
    prev = kfs[kfs.length - 1]!;
    next = kfs[kfs.length - 1]!;
  }

  const span = Math.max(1e-5, next.t - prev.t);
  const alpha = prev === next ? 0 : Math.max(0, Math.min(1, (t - prev.t) / span));

  const rootPos: [number, number, number] = [
    lerp(prev.rootPos[0] ?? 0, next.rootPos[0] ?? 0, alpha),
    lerp(prev.rootPos[1] ?? 0, next.rootPos[1] ?? 0, alpha),
    lerp(prev.rootPos[2] ?? 0, next.rootPos[2] ?? 0, alpha)
  ];

  const allKeys = new Set<string>([
    ...BONE_LIST.map((b) => b.id),
    ...Object.keys(prev.bones || {}),
    ...Object.keys(next.bones || {})
  ]);

  const bones: Record<string, [number, number, number]> = {};
  for (const key of allKeys) {
    const r0 = prev.bones?.[key] ?? [0, 0, 0];
    const r1 = next.bones?.[key] ?? [0, 0, 0];
    bones[key] = [
      lerpAngleDeg(r0[0] ?? 0, r1[0] ?? 0, alpha),
      lerpAngleDeg(r0[1] ?? 0, r1[1] ?? 0, alpha),
      lerpAngleDeg(r0[2] ?? 0, r1[2] ?? 0, alpha)
    ];
  }

  return { rootPos, bones };
}

export type GizmoAxis = 'x' | 'y' | 'z' | 'view' | 'tx' | 'ty' | 'tz';
export type GizmoMode = 'rotate' | 'translate';

/**
 * Unlit 3D Hierarchical Stickman Rig Engine
 * Uses ONLY THREE.MeshBasicMaterial with high-contrast flat colors:
 * - Torso / Head: Slate White (#E2E8F0 / #F8FAFC) + Bright Visor & Facing Nose
 * - Left Arm / Left Leg: Cyan / Sky (#00E5FF / #38BDF8)
 * - Right Arm / Right Leg: Coral / Amber (#FF6B6B / #F59E0B)
 * - Weapon: Lime (#C8FF3D)
 */
export class Pose3DRigScene {
  scene: THREE.Scene;
  orthoCamera: THREE.OrthographicCamera;
  perspCamera: THREE.PerspectiveCamera;
  activeCamera: THREE.Camera;
  renderer: THREE.WebGLRenderer;
  raycaster: THREE.Raycaster;

  bonePivots: Record<BoneId, THREE.Group>;
  pickMeshes: THREE.Mesh[] = [];
  meshMaterials: Map<THREE.Mesh, { baseColor: number; boneId: BoneId }> = new Map();

  // Dynamic geometry groups for proportion scaling
  private torsoScaleGroup!: THREE.Group;
  private headScaleGroup!: THREE.Group;
  private lArmGroup!: THREE.Group;
  private rArmGroup!: THREE.Group;
  private lLegGroup!: THREE.Group;
  private rLegGroup!: THREE.Group;
  private weaponVisualGroup!: THREE.Group;
  private groundGrid!: THREE.Group;

  // Unity-style 3D Rotation & Translation Gizmo
  gizmoGroup!: THREE.Group;
  private rotateGizmoGroup!: THREE.Group;
  private rotateLocalRingsGroup!: THREE.Group;
  private rotateViewRingGroup!: THREE.Group;
  private translateGizmoGroup!: THREE.Group;
  private gizmoPickMeshes: THREE.Mesh[] = [];
  private gizmoVisualMeshes: Map<GizmoAxis, { meshes: THREE.Mesh[]; baseColor: number }> = new Map();
  gizmoMode: GizmoMode = 'rotate';
  hoveredGizmoAxis: GizmoAxis | null = null;
  lastGizmoHitPoint: THREE.Vector3 | null = null;

  // Smooth camera target pan offset
  panX = 0;
  panY = 0;

  selectedBoneId: BoneId | null = 'r_upper_arm';
  private baseHipHeight = 1.05;

  constructor(canvas?: HTMLCanvasElement, width = 640, height = 640, transparentBg = false) {
    this.scene = new THREE.Scene();
    if (!transparentBg) {
      this.scene.background = new THREE.Color('#0E1117');
    }

    const aspect = width / Math.max(1, height);
    const frustumHeight = 2.9;
    this.orthoCamera = new THREE.OrthographicCamera(
      (-frustumHeight * aspect) / 2,
      (frustumHeight * aspect) / 2,
      frustumHeight / 2,
      -frustumHeight / 2,
      0.1,
      50
    );
    this.perspCamera = new THREE.PerspectiveCamera(35, aspect, 0.1, 50);
    this.activeCamera = this.orthoCamera;

    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: true,
      alpha: transparentBg,
      preserveDrawingBuffer: true
    });
    this.renderer.setSize(width, height, false);
    this.renderer.setPixelRatio(Math.min(2, typeof window !== 'undefined' ? window.devicePixelRatio || 1 : 1));

    this.raycaster = new THREE.Raycaster();
    this.bonePivots = {} as Record<BoneId, THREE.Group>;

    this.groundGrid = this.buildGroundFloor();
    this.scene.add(this.groundGrid);

    this.buildStickmanHierarchy();
    this.buildGizmoHierarchy();
    this.updateGizmoTransform();
  }

  private buildGroundFloor(): THREE.Group {
    const g = new THREE.Group();
    const gridHelper = new THREE.GridHelper(3.2, 8, 0x00e5ff, 0x222938);
    gridHelper.position.y = 0;
    g.add(gridHelper);

    // Facing arrow (+Z is character forward, which points to the right when yaw=90 side view!)
    const arrowMat = new THREE.MeshBasicMaterial({ color: 0x00e5ff, side: THREE.DoubleSide });
    const shaftGeo = new THREE.PlaneGeometry(0.06, 0.55);
    shaftGeo.rotateX(-Math.PI / 2);
    const shaft = new THREE.Mesh(shaftGeo, arrowMat);
    shaft.position.set(0, 0.004, 0.65);
    g.add(shaft);

    const tipGeo = new THREE.ConeGeometry(0.11, 0.22, 3);
    tipGeo.rotateX(Math.PI / 2);
    const tip = new THREE.Mesh(tipGeo, arrowMat);
    tip.position.set(0, 0.006, 0.98);
    g.add(tip);

    return g;
  }

  public setShowGroundGrid(visible: boolean) {
    this.groundGrid.visible = visible;
  }

  private createUnlitPart(
    geo: THREE.BufferGeometry,
    hexColor: number,
    boneId: BoneId,
    offsetY = 0,
    offsetZ = 0
  ): THREE.Mesh {
    const mat = new THREE.MeshBasicMaterial({ color: hexColor });
    const mesh = new THREE.Mesh(geo, mat);
    mesh.position.set(0, offsetY, offsetZ);
    mesh.userData.boneId = boneId;
    this.pickMeshes.push(mesh);
    this.meshMaterials.set(mesh, { baseColor: hexColor, boneId });
    return mesh;
  }

  private buildStickmanHierarchy() {
    const makePivot = (id: BoneId, parent: THREE.Object3D, x: number, y: number, z: number) => {
      const grp = new THREE.Group();
      grp.position.set(x, y, z);
      grp.name = id;
      parent.add(grp);
      this.bonePivots[id] = grp;
      return grp;
    };

    // 1. HIPS (Root at y = baseHipHeight)
    const hips = makePivot('hips', this.scene, 0, this.baseHipHeight, 0);
    const pelvisMesh = this.createUnlitPart(
      new THREE.BoxGeometry(0.32, 0.14, 0.2),
      0xcbd5e1,
      'hips',
      0,
      0
    );
    hips.add(pelvisMesh);

    this.torsoScaleGroup = new THREE.Group();
    hips.add(this.torsoScaleGroup);

    // 2. SPINE
    const spine = makePivot('spine', this.torsoScaleGroup, 0, 0.08, 0);
    spine.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.12, 0.13, 0.24, 12),
        0xe2e8f0,
        'spine',
        0.12,
        0
      )
    );

    // 3. CHEST
    const chest = makePivot('chest', spine, 0, 0.25, 0);
    chest.add(
      this.createUnlitPart(
        new THREE.BoxGeometry(0.38, 0.3, 0.22),
        0xf1f5f9,
        'chest',
        0.15,
        0
      )
    );
    // Chest front emblem so torso front/back is unmistakable
    const chestFront = new THREE.Mesh(
      new THREE.BoxGeometry(0.2, 0.14, 0.04),
      new THREE.MeshBasicMaterial({ color: 0x38bdf8 })
    );
    chestFront.position.set(0, 0.17, 0.115);
    chest.add(chestFront);

    // 4. NECK & HEAD
    const neck = makePivot('neck', chest, 0, 0.32, 0);
    neck.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.06, 0.07, 0.1, 10),
        0x94a3b8,
        'neck',
        0.04,
        0
      )
    );

    const head = makePivot('head', neck, 0, 0.09, 0);
    this.headScaleGroup = new THREE.Group();
    head.add(this.headScaleGroup);

    const cranium = this.createUnlitPart(
      new THREE.SphereGeometry(0.2, 18, 16),
      0xf8fafc,
      'head',
      0.18,
      0
    );
    this.headScaleGroup.add(cranium);

    // High-contrast visor & nose cone facing +Z so AI immediately sees head yaw/pitch
    const visor = new THREE.Mesh(
      new THREE.BoxGeometry(0.28, 0.08, 0.12),
      new THREE.MeshBasicMaterial({ color: 0x0f172a })
    );
    visor.position.set(0, 0.19, 0.14);
    this.headScaleGroup.add(visor);

    const nose = new THREE.Mesh(
      new THREE.ConeGeometry(0.055, 0.16, 8),
      new THREE.MeshBasicMaterial({ color: 0xfacc15 })
    );
    nose.rotation.x = Math.PI / 2;
    nose.position.set(0, 0.16, 0.25);
    this.headScaleGroup.add(nose);

    // 5. LEFT ARM (Cyan / Sky — #00E5FF / #38BDF8)
    this.lArmGroup = new THREE.Group();
    this.lArmGroup.position.set(0.25, 0.26, 0);
    chest.add(this.lArmGroup);

    const lUpperArm = makePivot('l_upper_arm', this.lArmGroup, 0, 0, 0);
    lUpperArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.075, 10, 10), 0x00e5ff, 'l_upper_arm', 0, 0)
    );
    lUpperArm.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.06, 0.052, 0.3, 10),
        0x00e5ff,
        'l_upper_arm',
        -0.15,
        0
      )
    );

    const lForeArm = makePivot('l_fore_arm', lUpperArm, 0, -0.3, 0);
    lForeArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.062, 10, 10), 0x38bdf8, 'l_fore_arm', 0, 0)
    );
    lForeArm.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.052, 0.045, 0.28, 10),
        0x38bdf8,
        'l_fore_arm',
        -0.14,
        0
      )
    );
    lForeArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.068, 10, 10), 0x7dd3fc, 'l_fore_arm', -0.29, 0)
    );

    // 6. RIGHT ARM (Coral / Amber — #FF6B6B / #F59E0B)
    this.rArmGroup = new THREE.Group();
    this.rArmGroup.position.set(-0.25, 0.26, 0);
    chest.add(this.rArmGroup);

    const rUpperArm = makePivot('r_upper_arm', this.rArmGroup, 0, 0, 0);
    rUpperArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.075, 10, 10), 0xff6b6b, 'r_upper_arm', 0, 0)
    );
    rUpperArm.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.06, 0.052, 0.3, 10),
        0xff6b6b,
        'r_upper_arm',
        -0.15,
        0
      )
    );

    const rForeArm = makePivot('r_fore_arm', rUpperArm, 0, -0.3, 0);
    rForeArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.062, 10, 10), 0xf59e0b, 'r_fore_arm', 0, 0)
    );
    rForeArm.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.052, 0.045, 0.28, 10),
        0xf59e0b,
        'r_fore_arm',
        -0.14,
        0
      )
    );
    rForeArm.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.068, 10, 10), 0xfbbf24, 'r_fore_arm', -0.29, 0)
    );

    // 7. WEAPON attached to right hand
    const weaponPivot = makePivot('weapon', rForeArm, 0, -0.29, 0.02);
    this.weaponVisualGroup = new THREE.Group();
    weaponPivot.add(this.weaponVisualGroup);
    this.rebuildWeaponGeometry('sword', 1.0);

    // 8. LEFT LEG (Cyan / Sky — #00E5FF / #38BDF8)
    this.lLegGroup = new THREE.Group();
    this.lLegGroup.position.set(0.11, -0.05, 0);
    hips.add(this.lLegGroup);

    const lThigh = makePivot('l_thigh', this.lLegGroup, 0, 0, 0);
    lThigh.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.08, 10, 10), 0x00e5ff, 'l_thigh', 0, 0)
    );
    lThigh.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.075, 0.062, 0.44, 10),
        0x00e5ff,
        'l_thigh',
        -0.22,
        0
      )
    );

    const lShin = makePivot('l_shin', lThigh, 0, -0.44, 0);
    lShin.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.068, 10, 10), 0x38bdf8, 'l_shin', 0, 0)
    );
    lShin.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.06, 0.05, 0.44, 10),
        0x38bdf8,
        'l_shin',
        -0.22,
        0
      )
    );

    const lFoot = makePivot('l_foot', lShin, 0, -0.44, 0);
    lFoot.add(
      this.createUnlitPart(new THREE.BoxGeometry(0.11, 0.08, 0.24), 0x0284c7, 'l_foot', -0.04, 0.06)
    );

    // 9. RIGHT LEG (Coral / Amber — #FF6B6B / #F59E0B)
    this.rLegGroup = new THREE.Group();
    this.rLegGroup.position.set(-0.11, -0.05, 0);
    hips.add(this.rLegGroup);

    const rThigh = makePivot('r_thigh', this.rLegGroup, 0, 0, 0);
    rThigh.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.08, 10, 10), 0xff6b6b, 'r_thigh', 0, 0)
    );
    rThigh.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.075, 0.062, 0.44, 10),
        0xff6b6b,
        'r_thigh',
        -0.22,
        0
      )
    );

    const rShin = makePivot('r_shin', rThigh, 0, -0.44, 0);
    rShin.add(
      this.createUnlitPart(new THREE.SphereGeometry(0.068, 10, 10), 0xf59e0b, 'r_shin', 0, 0)
    );
    rShin.add(
      this.createUnlitPart(
        new THREE.CylinderGeometry(0.06, 0.05, 0.44, 10),
        0xf59e0b,
        'r_shin',
        -0.22,
        0
      )
    );

    const rFoot = makePivot('r_foot', rShin, 0, -0.44, 0);
    rFoot.add(
      this.createUnlitPart(new THREE.BoxGeometry(0.11, 0.08, 0.24), 0xea580c, 'r_foot', -0.04, 0.06)
    );
  }

  private rebuildWeaponGeometry(weaponType: string, weaponScale: number) {
    while (this.weaponVisualGroup.children.length > 0) {
      const child = this.weaponVisualGroup.children[0]!;
      this.weaponVisualGroup.remove(child);
    }
    // Remove old weapon meshes from pickMeshes
    this.pickMeshes = this.pickMeshes.filter((m) => m.userData.boneId !== 'weapon');

    if (!weaponType || weaponType === 'none') {
      return;
    }

    const s = Math.max(0.4, Math.min(2.5, weaponScale || 1.0));
    this.weaponVisualGroup.scale.set(s, s, s);

    if (weaponType === 'sword') {
      const guard = this.createUnlitPart(
        new THREE.BoxGeometry(0.18, 0.04, 0.06),
        0xfacc15,
        'weapon',
        0,
        0.08
      );
      const blade = this.createUnlitPart(
        new THREE.BoxGeometry(0.035, 0.07, 0.85),
        0xc8ff3d,
        'weapon',
        0,
        0.52
      );
      this.weaponVisualGroup.add(guard, blade);
    } else if (weaponType === 'spear') {
      const shaft = this.createUnlitPart(
        new THREE.CylinderGeometry(0.025, 0.025, 1.45, 8),
        0xfacc15,
        'weapon',
        0,
        0.35
      );
      shaft.rotation.x = Math.PI / 2;
      const tip = this.createUnlitPart(
        new THREE.ConeGeometry(0.065, 0.28, 6),
        0xc8ff3d,
        'weapon',
        0,
        1.18
      );
      tip.rotation.x = Math.PI / 2;
      this.weaponVisualGroup.add(shaft, tip);
    } else if (weaponType === 'staff') {
      const shaft = this.createUnlitPart(
        new THREE.CylinderGeometry(0.028, 0.024, 1.3, 8),
        0xf59e0b,
        'weapon',
        0,
        0.3
      );
      shaft.rotation.x = Math.PI / 2;
      const orb = this.createUnlitPart(
        new THREE.OctahedronGeometry(0.12, 1),
        0xc8ff3d,
        'weapon',
        0,
        1.02
      );
      this.weaponVisualGroup.add(shaft, orb);
    }
  }

  public applyProportion(prop: PoseProportionConfig) {
    const hs = Math.max(0.5, Math.min(2.6, prop.headScale || 1.0));
    const ts = Math.max(0.5, Math.min(1.8, prop.torsoScale || 1.0));
    const ls = Math.max(0.45, Math.min(1.8, prop.limbScale || 1.0));

    this.headScaleGroup.scale.set(hs, hs, hs);
    this.torsoScaleGroup.scale.set(ts, ts, ts);
    this.lArmGroup.scale.set(ls / ts, ls / ts, ls / ts);
    this.rArmGroup.scale.set(ls / ts, ls / ts, ls / ts);
    this.lLegGroup.scale.set(ls, ls, ls);
    this.rLegGroup.scale.set(ls, ls, ls);

    this.baseHipHeight = 0.96 * ls + 0.08;
    this.rebuildWeaponGeometry(prop.weaponType || 'none', prop.weaponScale ?? 1.0);
    this.refreshSelectionHighlight();
  }

  public resetPan() {
    this.panX = 0;
    this.panY = 0;
  }

  private buildGizmoHierarchy() {
    const R = 0.28;
    this.gizmoGroup = new THREE.Group();
    this.gizmoGroup.name = 'unity_gizmo_root';
    this.gizmoGroup.renderOrder = 999;
    this.scene.add(this.gizmoGroup);

    this.rotateGizmoGroup = new THREE.Group();
    this.gizmoGroup.add(this.rotateGizmoGroup);

    // 1. Inner Translucent Sphere
    const sphereMat = new THREE.MeshBasicMaterial({
      color: 0x0f172a,
      transparent: true,
      opacity: 0.22,
      depthTest: false,
      depthWrite: false
    });
    const innerSphere = new THREE.Mesh(new THREE.SphereGeometry(R * 0.96, 24, 24), sphereMat);
    innerSphere.renderOrder = 998;
    this.rotateGizmoGroup.add(innerSphere);

    // 2. Outer White View-Aligned Ring
    this.rotateViewRingGroup = new THREE.Group();
    this.rotateGizmoGroup.add(this.rotateViewRingGroup);

    const viewMat = new THREE.MeshBasicMaterial({
      color: 0xf8fafc,
      transparent: true,
      opacity: 0.92,
      depthTest: false,
      depthWrite: false
    });
    const viewRingMesh = new THREE.Mesh(new THREE.TorusGeometry(R * 1.08, 0.007, 8, 64), viewMat);
    viewRingMesh.renderOrder = 999;
    this.rotateViewRingGroup.add(viewRingMesh);

    const hitMat = new THREE.MeshBasicMaterial({
      color: 0xffffff,
      transparent: true,
      opacity: 0.001,
      depthTest: false,
      depthWrite: false
    });
    const viewHitMesh = new THREE.Mesh(new THREE.TorusGeometry(R * 1.08, 0.042, 8, 48), hitMat);
    viewHitMesh.renderOrder = 999;
    viewHitMesh.userData.gizmoAxis = 'view';
    this.rotateViewRingGroup.add(viewHitMesh);
    this.gizmoPickMeshes.push(viewHitMesh);
    this.gizmoVisualMeshes.set('view', { meshes: [viewRingMesh], baseColor: 0xf8fafc });

    // 3/4/5. Local X (Red), Y (Green), Z (Blue) Rotation Rings
    this.rotateLocalRingsGroup = new THREE.Group();
    this.rotateGizmoGroup.add(this.rotateLocalRingsGroup);

    const makeRotRing = (axis: 'x' | 'y' | 'z', hexColor: number) => {
      const mat = new THREE.MeshBasicMaterial({
        color: hexColor,
        depthTest: false,
        depthWrite: false
      });
      const visual = new THREE.Mesh(new THREE.TorusGeometry(R, 0.011, 10, 64), mat);
      visual.renderOrder = 999;

      const hit = new THREE.Mesh(new THREE.TorusGeometry(R, 0.045, 8, 48), hitMat);
      hit.renderOrder = 999;
      hit.userData.gizmoAxis = axis;

      if (axis === 'x') {
        visual.rotation.y = Math.PI / 2;
        hit.rotation.y = Math.PI / 2;
      } else if (axis === 'y') {
        visual.rotation.x = Math.PI / 2;
        hit.rotation.x = Math.PI / 2;
      }

      this.rotateLocalRingsGroup.add(visual, hit);
      this.gizmoPickMeshes.push(hit);
      this.gizmoVisualMeshes.set(axis, { meshes: [visual], baseColor: hexColor });
    };

    makeRotRing('x', 0xef4444);
    makeRotRing('y', 0x22c55e);
    makeRotRing('z', 0x3b82f6);

    // Translate Gizmo (3-Axis Move Arrows for Root hips X/Y/Z)
    this.translateGizmoGroup = new THREE.Group();
    this.gizmoGroup.add(this.translateGizmoGroup);

    const makeMoveArrow = (axis: 'tx' | 'ty' | 'tz', hexColor: number) => {
      const holder = new THREE.Group();
      const mat = new THREE.MeshBasicMaterial({
        color: hexColor,
        depthTest: false,
        depthWrite: false
      });
      const shaft = new THREE.Mesh(new THREE.CylinderGeometry(0.012, 0.012, 0.34, 8), mat);
      shaft.position.y = 0.17;
      shaft.renderOrder = 999;

      const tip = new THREE.Mesh(new THREE.ConeGeometry(0.038, 0.11, 10), mat);
      tip.position.y = 0.38;
      tip.renderOrder = 999;

      const hit = new THREE.Mesh(new THREE.CylinderGeometry(0.055, 0.055, 0.46, 8), hitMat);
      hit.position.y = 0.23;
      hit.renderOrder = 999;
      hit.userData.gizmoAxis = axis;

      holder.add(shaft, tip, hit);
      if (axis === 'tx') {
        holder.rotation.z = -Math.PI / 2;
      } else if (axis === 'tz') {
        holder.rotation.x = Math.PI / 2;
      }

      this.translateGizmoGroup.add(holder);
      this.gizmoPickMeshes.push(hit);
      this.gizmoVisualMeshes.set(axis, { meshes: [shaft, tip], baseColor: hexColor });
    };

    makeMoveArrow('tx', 0xef4444);
    makeMoveArrow('ty', 0x22c55e);
    makeMoveArrow('tz', 0x3b82f6);
  }

  public setGizmoMode(mode: GizmoMode) {
    this.gizmoMode = mode;
    this.updateGizmoTransform();
  }

  public setHoveredGizmoAxis(axis: GizmoAxis | null) {
    this.hoveredGizmoAxis = axis;
    for (const [key, entry] of this.gizmoVisualMeshes.entries()) {
      const targetHex = key === axis ? 0xfacc15 : entry.baseColor;
      for (const m of entry.meshes) {
        (m.material as THREE.MeshBasicMaterial).color.setHex(targetHex);
      }
    }
  }

  private updateGizmoTransform() {
    if (!this.gizmoGroup) return;
    if (!this.selectedBoneId) {
      this.gizmoGroup.visible = false;
      return;
    }

    this.scene.updateMatrixWorld(true);
    this.activeCamera.updateMatrixWorld(true);

    this.gizmoGroup.visible = true;
    this.rotateGizmoGroup.visible = this.gizmoMode === 'rotate';
    this.translateGizmoGroup.visible = this.gizmoMode === 'translate';

    if (this.gizmoMode === 'translate') {
      const hips = this.bonePivots.hips;
      if (hips) {
        const wp = new THREE.Vector3();
        hips.getWorldPosition(wp);
        this.gizmoGroup.position.copy(wp);
        this.gizmoGroup.quaternion.identity();
      }
      return;
    }

    const piv = this.bonePivots[this.selectedBoneId];
    if (!piv) {
      this.gizmoGroup.visible = false;
      return;
    }

    const wp = new THREE.Vector3();
    piv.getWorldPosition(wp);
    this.gizmoGroup.position.copy(wp);

    const parentQuat = new THREE.Quaternion();
    if (piv.parent) {
      piv.parent.getWorldQuaternion(parentQuat);
    }
    this.gizmoGroup.quaternion.copy(parentQuat);
    this.rotateLocalRingsGroup.quaternion.copy(piv.quaternion);

    const camQuat = new THREE.Quaternion();
    this.activeCamera.getWorldQuaternion(camQuat);
    this.rotateViewRingGroup.quaternion
      .copy(parentQuat)
      .invert()
      .multiply(camQuat);
  }

  public pickGizmoAxis(ndcX: number, ndcY: number): GizmoAxis | null {
    if (!this.selectedBoneId || !this.gizmoGroup || !this.gizmoGroup.visible) {
      return null;
    }
    this.updateGizmoTransform();
    this.scene.updateMatrixWorld(true);
    this.raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.activeCamera);

    const activeTargets = this.gizmoPickMeshes.filter((m) => {
      const ax = m.userData.gizmoAxis as GizmoAxis;
      const isTrans = ax === 'tx' || ax === 'ty' || ax === 'tz';
      return this.gizmoMode === 'translate' ? isTrans : !isTrans;
    });

    const hits = this.raycaster.intersectObjects(activeTargets, false);
    if (hits.length > 0) {
      const top = hits[0]!;
      this.lastGizmoHitPoint = top.point.clone();
      return (top.object.userData.gizmoAxis as GizmoAxis) ?? null;
    }
    return null;
  }

  public getAxisScreenTangent(
    axis: GizmoAxis,
    hitWorldPoint?: THREE.Vector3,
    width?: number,
    height?: number
  ): { tx: number; ty: number; centerPx: [number, number]; axisSign: number } {
    const w = Math.max(1, width ?? (this.renderer.domElement?.clientWidth || 640));
    const h = Math.max(1, height ?? (this.renderer.domElement?.clientHeight || 480));

    this.updateGizmoTransform();
    this.scene.updateMatrixWorld(true);
    this.activeCamera.updateMatrixWorld(true);

    const toScreen = (pt: THREE.Vector3): [number, number] => {
      const p = pt.clone().project(this.activeCamera);
      return [((p.x + 1) * 0.5) * w, ((-p.y + 1) * 0.5) * h];
    };

    const centerWorld = this.gizmoGroup.position.clone();
    const centerPx = toScreen(centerWorld);

    if (axis === 'tx' || axis === 'ty' || axis === 'tz') {
      const dir =
        axis === 'tx'
          ? new THREE.Vector3(1, 0, 0)
          : axis === 'ty'
            ? new THREE.Vector3(0, 1, 0)
            : new THREE.Vector3(0, 0, 1);
      const tipPx = toScreen(centerWorld.clone().addScaledVector(dir, 0.35));
      const vx = tipPx[0] - centerPx[0];
      const vy = tipPx[1] - centerPx[1];
      const len = Math.hypot(vx, vy) || 1;
      return {
        tx: vx / len,
        ty: vy / len,
        centerPx,
        axisSign: 1
      };
    }

    const ringWorldQuat = new THREE.Quaternion();
    this.rotateLocalRingsGroup.getWorldQuaternion(ringWorldQuat);

    let worldAxis: THREE.Vector3;
    if (axis === 'x') {
      worldAxis = new THREE.Vector3(1, 0, 0).applyQuaternion(ringWorldQuat).normalize();
    } else if (axis === 'y') {
      worldAxis = new THREE.Vector3(0, 1, 0).applyQuaternion(ringWorldQuat).normalize();
    } else if (axis === 'z') {
      worldAxis = new THREE.Vector3(0, 0, 1).applyQuaternion(ringWorldQuat).normalize();
    } else {
      const camQuat = new THREE.Quaternion();
      this.activeCamera.getWorldQuaternion(camQuat);
      worldAxis = new THREE.Vector3(0, 0, -1).applyQuaternion(camQuat).normalize();
    }

    const camForward = new THREE.Vector3();
    this.activeCamera.getWorldDirection(camForward);
    // In screen coordinates (+X right, +Y down), camera look direction is +Z into screen
    const axisSign = worldAxis.dot(camForward) >= 0 ? 1 : -1;

    const hitPt = hitWorldPoint ?? this.lastGizmoHitPoint;
    if (hitPt) {
      const rel = hitPt.clone().sub(centerWorld);
      const tangent3d = new THREE.Vector3().crossVectors(worldAxis, rel);
      if (tangent3d.lengthSq() > 1e-5) {
        tangent3d.normalize();
        const p0 = toScreen(hitPt);
        const p1 = toScreen(hitPt.clone().addScaledVector(tangent3d, 0.15));
        const vx = p1[0] - p0[0];
        const vy = p1[1] - p0[1];
        const len = Math.hypot(vx, vy);
        if (len > 0.5) {
          return {
            tx: vx / len,
            ty: vy / len,
            centerPx,
            axisSign
          };
        }
      }
    }

    // Fallback screen tangent perpendicular to projected rotation axis
    const axisTipPx = toScreen(centerWorld.clone().addScaledVector(worldAxis, 0.3));
    const ax = axisTipPx[0] - centerPx[0];
    const ay = axisTipPx[1] - centerPx[1];
    const alen = Math.hypot(ax, ay);
    if (alen > 1.0) {
      return {
        tx: -ay / alen,
        ty: ax / alen,
        centerPx,
        axisSign
      };
    }

    return {
      tx: 1,
      ty: 0,
      centerPx,
      axisSign
    };
  }

  public applyCamera(cam: PoseCameraConfig, width: number, height: number) {
    const aspect = width / Math.max(1, height);
    const zoom = Math.max(0.35, Math.min(3.5, cam.zoom || 1.0));
    const frustumHeight = 2.95 / zoom;

    this.orthoCamera.left = (-frustumHeight * aspect) / 2;
    this.orthoCamera.right = (frustumHeight * aspect) / 2;
    this.orthoCamera.top = frustumHeight / 2;
    this.orthoCamera.bottom = -frustumHeight / 2;
    this.orthoCamera.updateProjectionMatrix();

    this.perspCamera.aspect = aspect;
    this.perspCamera.fov = 36 / Math.sqrt(zoom);
    this.perspCamera.updateProjectionMatrix();

    this.activeCamera = cam.ortho ? this.orthoCamera : this.perspCamera;

    const yawRad = THREE.MathUtils.degToRad(cam.yaw ?? -90);
    const pitchRad = THREE.MathUtils.degToRad(Math.max(-85, Math.min(88, cam.pitch ?? 0)));
    const dist = 6.5;
    const targetY = 1.02;

    const cosP = Math.cos(pitchRad);
    const sinP = Math.sin(pitchRad);
    const sinY = Math.sin(yawRad);
    const cosY = Math.cos(yawRad);

    // Camera right & up vectors for smooth target panning
    const rightX = cosY;
    const rightZ = -sinY;
    const upX = -sinP * sinY;
    const upY = cosP;
    const upZ = -sinP * cosY;

    const offsetX = rightX * this.panX + upX * this.panY;
    const offsetY = upY * this.panY;
    const offsetZ = rightZ * this.panX + upZ * this.panY;

    const x = offsetX + dist * cosP * sinY;
    const y = targetY + offsetY + dist * sinP;
    const z = offsetZ + dist * cosP * cosY;

    this.activeCamera.position.set(x, y, z);
    this.activeCamera.lookAt(offsetX, targetY + offsetY, offsetZ);
    this.updateGizmoTransform();
  }

  public applyPose(pose: {
    rootPos: [number, number, number];
    bones: Record<string, [number, number, number]>;
  }) {
    const hips = this.bonePivots.hips;
    if (hips) {
      hips.position.set(
        pose.rootPos[0] ?? 0,
        this.baseHipHeight + (pose.rootPos[1] ?? 0),
        pose.rootPos[2] ?? 0
      );
    }

    for (const meta of BONE_LIST) {
      const piv = this.bonePivots[meta.id];
      if (!piv) continue;
      const rot = pose.bones[meta.id] ?? [0, 0, 0];
      piv.rotation.set(
        THREE.MathUtils.degToRad(rot[0] ?? 0),
        THREE.MathUtils.degToRad(rot[1] ?? 0),
        THREE.MathUtils.degToRad(rot[2] ?? 0),
        'XYZ'
      );
    }
    this.updateGizmoTransform();
  }

  public setSelectedBone(boneId: BoneId | null) {
    this.selectedBoneId = boneId;
    this.refreshSelectionHighlight();
    this.updateGizmoTransform();
  }

  private refreshSelectionHighlight() {
    for (const [mesh, info] of this.meshMaterials.entries()) {
      const mat = mesh.material as THREE.MeshBasicMaterial;
      if (this.selectedBoneId && info.boneId === this.selectedBoneId) {
        mat.color.setHex(0xffffff);
      } else {
        mat.color.setHex(info.baseColor);
      }
    }
  }

  public pickBone(ndcX: number, ndcY: number): BoneId | null {
    this.raycaster.setFromCamera(new THREE.Vector2(ndcX, ndcY), this.activeCamera);
    const hits = this.raycaster.intersectObjects(this.pickMeshes, false);
    if (hits.length > 0) {
      const bid = hits[0]!.object.userData.boneId as BoneId | undefined;
      if (bid) return bid;
    }
    return null;
  }

  public resize(width: number, height: number, cam: PoseCameraConfig) {
    this.renderer.setSize(width, height, false);
    this.applyCamera(cam, width, height);
  }

  public render() {
    this.renderer.render(this.scene, this.activeCamera);
  }

  public dispose() {
    this.renderer.dispose();
  }
}

/**
 * Renders a `PoseActionClip` into a clean `Rows × Cols` Sprite Reference Sheet PNG DataURL.
 * Each cell renders the unlit color-coded 3D stickman from the exact same camera & ground baseline,
 * with a dashed safe-zone box, ground baseline, and frame index badge.
 */
export function renderClipToSpriteSheetDataUrl(
  clip: PoseActionClip,
  rows: number,
  cols: number,
  width = 1536,
  height = 1024
): string {
  const safeRows = Math.max(1, Math.min(6, Math.round(rows || clip.defaultRows || 2)));
  const safeCols = Math.max(1, Math.min(8, Math.round(cols || clip.defaultCols || 3)));
  const totalFrames = safeRows * safeCols;

  const cellW = Math.floor(width / safeCols);
  const cellH = Math.floor(height / safeRows);

  const outCv = document.createElement('canvas');
  outCv.width = width;
  outCv.height = height;
  const ctx = outCv.getContext('2d')!;

  // Pure studio white backdrop for clean AI conditioning
  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, width, height);

  const rig = new Pose3DRigScene(undefined, cellW, cellH, true);
  rig.setShowGroundGrid(false);
  rig. setSelectedBone(null);
  rig.applyProportion(clip.proportion);
  rig.applyCamera(clip.camera, cellW, cellH);

  for (let i = 0; i < totalFrames; i++) {
    const r = Math.floor(i / safeCols);
    const c = i % safeCols;
    const x0 = c * cellW;
    const y0 = r * cellH;

    const sampleT =
      totalFrames <= 1
        ? 0
        : clip.loopAnim
          ? i / totalFrames
          : i / (totalFrames - 1);

    const marginX = cellW * 0.12;
    const marginY = cellH * 0.1;
    const sx = x0 + marginX;
    const sy = y0 + marginY;
    const sw = cellW - marginX * 2;
    const sh = cellH - marginY * 2;
    const footY = y0 + cellH * 0.85;

    // 1. Safe-zone dashed frame & ground baseline
    ctx.save();
    ctx.strokeStyle = 'rgba(148, 163, 184, 0.42)';
    ctx.lineWidth = 2;
    ctx.setLineDash([8, 8]);
    ctx.strokeRect(sx, sy, sw, sh);

    ctx.strokeStyle = 'rgba(56, 189, 248, 0.55)';
    ctx.setLineDash([4, 6]);
    ctx.beginPath();
    ctx.moveTo(sx + sw * 0.15, footY);
    ctx.lineTo(sx + sw * 0.85, footY);
    ctx.stroke();

    // Frame index tag
    ctx.fillStyle = 'rgba(15, 23, 42, 0.55)';
    ctx.font = 'bold 14px monospace';
    ctx.fillText(`#${i + 1}`, sx + 6, sy + 18);
    ctx.restore();

    // 2. Render 3D Stickman at sampleT
    const pose = interpolatePoseAtTime(clip, sampleT);
    rig.applyPose(pose);
    rig.render();

    ctx.drawImage(rig.renderer.domElement, x0, y0, cellW, cellH);
  }

  rig.dispose();
  return outCv.toDataURL('image/png');
}
