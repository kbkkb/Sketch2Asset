export type CharacterModuleId = 'turnaround_skin' | 'spritesheet' | 'expression';

export interface CharacterModuleMeta {
  id: CharacterModuleId;
  code: string;
  title: string;
  titleEn: string;
  subtitle: string;
  subtitleEn: string;
  badge: string;
  badgeEn: string;
}

/**
 * 角色工坊可扩展功能模块注册表
 * 若后续新增角色相关功能（如八方向行走、骨骼拆件图层、武器派生等），只需在此注册表追加模块定义并挂载对应视图即可。
 */
export const CHARACTER_MODULES: CharacterModuleMeta[] = [
  {
    id: 'turnaround_skin',
    code: 'MOD-01',
    title: '三视图与角色换肤',
    titleEn: 'Turnaround & Skinning',
    subtitle: '标准正/侧/背三视图生成切分 · 锁定体型同模换肤派生',
    subtitleEn: 'Front/Side/Back turnaround slicer · Identity-locked outfit skinning',
    badge: '设定 & 换肤',
    badgeEn: 'Design & Skin'
  },
  {
    id: 'spritesheet',
    code: 'MOD-02',
    title: '序列帧动画工坊',
    titleEn: 'Sprite Sheet Studio',
    subtitle: '防出界序列帧生成 · 连通域智能切帧 · 脚底重心对齐预览',
    subtitleEn: 'Anti-clip sprite sheet gen · Smart island slicer · Foot pivot player',
    badge: '动画 & 切帧',
    badgeEn: 'Anim & Slicer'
  },
  {
    id: 'expression',
    code: 'MOD-03',
    title: '角色表情差分集',
    titleEn: 'Expression Sheet',
    subtitle: '喜怒哀乐多表情网格生成 · 自动切分对话立绘差分',
    subtitleEn: 'Multi-expression portrait grid · Auto-sliced dialogue portraits',
    badge: '立绘差分',
    badgeEn: 'Expressions'
  }
];

export interface SpriteActionPreset {
  id: string;
  label: string;
  labelEn: string;
  placeholder: string;
  placeholderEn: string;
  rule: string;
}

/**
 * 序列帧动作预设模板
 * 第 1 项为「默认自由模板 (Default)」，不注入任何特定动作套路，完全由用户输入的提示词决定动作，避免内置动作词污染。
 */
export const SPRITE_ACTION_PRESETS: SpriteActionPreset[] = [
  {
    id: 'default',
    label: '默认模板 (自由描述动作，无内置动作词干预)',
    labelEn: 'Default Template (Free Action Prompt, No Built-in Bias)',
    placeholder: '输入角色与连续动作描述（例如：一个拿着武士刀的香蕉武士，拔刀横斩并收刀的连续分解动作）...',
    placeholderEn: 'Describe character & sequential motion (e.g., A banana samurai drawing katana, slashing horizontally, and sheathing)...',
    rule: '2D game sprite sheet of sequential character animation frames, uniform phase progression with large distinct limb silhouette changes between every consecutive frame (strictly NO duplicate or near-identical adjacent poses)'
  },
  {
    id: 'idle',
    label: '待机呼吸 (Idle Cycle)',
    labelEn: 'Idle Breathing Cycle',
    placeholder: '补充角色特征或待机细节（如已选定角色参考图可留空，直接按待机呼吸预设生成）...',
    placeholderEn: 'Optional character or idle details (leave empty to use reference character with Idle Breathing preset)...',
    rule: '2D game sprite sheet of idle breathing animation cycle: Frame 1 neutral upright -> Frame 2 inhaling chest/shoulders rising -> Frame 3 peak expansion -> Frame 4 exhaling settling -> Frame 5 lowest relaxed posture -> Frame 6 rebounding to neutral, feet planted at identical ground baseline'
  },
  {
    id: 'walk',
    label: '横向行走 (Walk Cycle)',
    labelEn: 'Side Walk Cycle',
    placeholder: '补充角色特征或行走细节（如已选定角色参考图可留空，直接按横向行走预设生成）...',
    placeholderEn: 'Optional character or walk details (leave empty to use reference character with Walk Cycle preset)...',
    rule: '2D side-view game sprite sheet of a full walking cycle facing right with strict biomechanic keyframe breakdown: (1) Full Stride Contact A (front heel landing, back leg extended), (2) Down/Recoil A (front knee bent low), (3) Mid-Stride Passing A (legs crossing torso midline), (4) High-Point Push-Off A, (5) Full Stride Contact B (opposite leg forward), (6) Down/Passing B — every consecutive frame MUST show a distinct leg/arm angle with zero duplicate adjacent poses'
  },
  {
    id: 'run',
    label: '高速奔跑 (Run Cycle)',
    labelEn: 'Dynamic Run Cycle',
    placeholder: '补充角色特征或奔跑细节（如已选定角色参考图可留空，直接按高速奔跑预设生成）...',
    placeholderEn: 'Optional character or run details (leave empty to use reference character with Run Cycle preset)...',
    rule: '2D side-view game sprite sheet of a dynamic running cycle facing right with strict keyframe progression: (1) Wide Airborne Stride A, (2) Front Foot Impact Compression A, (3) Rapid Mid-Stride Drive A, (4) Explosive Rear Toe Push-Off A, (5) Opposite Wide Stride B, (6) Opposite Drive/Recovery B — uniform motion delta between every frame, no repeated adjacent poses'
  },
  {
    id: 'attack',
    label: '近战挥砍 (Attack Combo)',
    labelEn: 'Melee Attack Combo',
    placeholder: '补充角色或挥砍招式细节（如：双手高举太刀蓄力并向前劈砍，已选角色参考图可留空）...',
    placeholderEn: 'Optional character or slash details (e.g., Raising katana overhead and slashing forward)...',
    rule: '2D game sprite sheet of a melee attack animation sequence: anticipation wind-up, forward strike, slash follow-through, and recovery pose'
  },
  {
    id: 'cast',
    label: '技能施法 (Cast Spell)',
    labelEn: 'Magic Spellcasting',
    placeholder: '补充角色或施法特效细节（如：举起法杖凝聚冰霜能量并向前释放，已选参考图可留空）...',
    placeholderEn: 'Optional character or spell details (e.g., Raising staff to gather frost energy and casting forward)...',
    rule: '2D game sprite sheet of a magic spellcasting animation sequence: gathering energy, chanting pose, releasing compact energy burst, and recovery'
  },
  {
    id: 'jump',
    label: '跳跃起落 (Jump Arc)',
    labelEn: 'Jump & Landing Arc',
    placeholder: '补充角色或跳跃动作细节（如已选定角色参考图可留空，直接按起跳滞空落地预设生成）...',
    placeholderEn: 'Optional character or jump details (leave empty to use reference character with Jump Arc preset)...',
    rule: '2D game sprite sheet of a jump animation sequence: crouch preparation, upward launch, mid-air apex pose, downward fall, and landing buffer'
  },
  {
    id: 'hit_ko',
    label: '受击与硬直 (Hit / Knockback)',
    labelEn: 'Hit Flinch & Knockback',
    placeholder: '补充角色或受击反馈细节（如已选定角色参考图可留空，直接按受击后仰恢复预设生成）...',
    placeholderEn: 'Optional character or hit reaction details (leave empty to use reference character with Hit Flinch preset)...',
    rule: '2D game sprite sheet of a hit reaction animation sequence: initial impact flinch, knockback recoil posture, bracing stance, and recovery'
  }
];

/**
 * 生成一张用于生图防出界的「虚拟安全区网格参考图 (Safe-Zone Grid Layout Guide)」
 * 帮助多模态大模型直观感知 Rows × Cols 的格子边界与每格中心 70% 安全区，防止挥刀/跳跃画到格子外面
 */
export function buildSafeGridGuideDataUrl(
  rows: number,
  cols: number,
  width = 1536,
  height = 1024
): string {
  const cv = document.createElement('canvas');
  cv.width = width;
  cv.height = height;
  const ctx = cv.getContext('2d')!;

  ctx.fillStyle = '#FFFFFF';
  ctx.fillRect(0, 0, width, height);

  const cellW = width / cols;
  const cellH = height / rows;

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      const x0 = c * cellW;
      const y0 = r * cellH;

      const marginX = cellW * 0.14;
      const marginY = cellH * 0.12;
      const sx = x0 + marginX;
      const sy = y0 + marginY;
      const sw = cellW - marginX * 2;
      const sh = cellH - marginY * 2;

      ctx.save();
      ctx.strokeStyle = 'rgba(180, 200, 220, 0.35)';
      ctx.lineWidth = 2;
      ctx.setLineDash([8, 8]);
      ctx.strokeRect(sx, sy, sw, sh);

      const footY = y0 + cellH * 0.86;
      const centerX = x0 + cellW * 0.5;
      ctx.strokeStyle = 'rgba(160, 190, 215, 0.4)';
      ctx.setLineDash([4, 6]);
      ctx.beginPath();
      ctx.moveTo(sx + sw * 0.2, footY);
      ctx.lineTo(sx + sw * 0.8, footY);
      ctx.moveTo(centerX, sy + sh * 0.15);
      ctx.lineTo(centerX, footY + 6);
      ctx.stroke();
      ctx.restore();
    }
  }

  return cv.toDataURL('image/png');
}
