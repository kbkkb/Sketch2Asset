import type { MessageKey } from '$lib/i18n/index.svelte';

export interface PromptModifierTag {
  id: string;
  zh: string;
  en: string;
  promptSnippet: string;
}

export interface ArchetypePreset {
  id: string;
  labelKey: MessageKey;
  promptRule: string;
  recommendedSize: string;
  modifiers: PromptModifierTag[];
}

export const ARCHETYPE_PRESETS: ArchetypePreset[] = [
  {
    id: 'item_icon',
    labelKey: 'arch_item_icon',
    promptRule:
      'Game item icon asset, single isolated object centered in frame, clean silhouette, generous margin around edges, no cropped parts, no text or UI frames.',
    recommendedSize: '1024x1024',
    modifiers: [
      { id: 'iso45', zh: '45° 等轴测视角', en: '45° Isometric', promptSnippet: '45-degree isometric game icon view' },
      { id: 'front', zh: '正视对称构图', en: 'Front Symmetrical', promptSnippet: 'front-facing symmetrical icon composition' },
      { id: 'bold_rim', zh: '高对比轮廓光', en: 'Crisp Rim Light', promptSnippet: 'strong readable silhouette with crisp rim lighting' },
      { id: 'metal_gem', zh: '金属与宝石高光', en: 'Metallic & Gem Sheen', promptSnippet: 'rich metallic specular highlights and translucent gemstone glow' },
      { id: 'worn', zh: '战损古旧质感', en: 'Battle-worn', promptSnippet: 'weathered battle-worn surface details and scratches' }
    ]
  },
  {
    id: 'pixel_sprite',
    labelKey: 'arch_pixel_sprite',
    promptRule:
      'Authentic 2D game pixel art sprite snapped to a strictly uniform 1:1 square pixel grid, consistent pixel block size across the entire sprite, ZERO mixels (never mix smaller sub-pixel resolutions for eyes or fine details), flat solid colors per pixel block, no gradient blur, no anti-aliasing fuzz, single isolated sprite.',
    recommendedSize: '1024x1024',
    modifiers: [
      { id: 'no_mixels', zh: '严格等大网格 (禁伪像素)', en: 'Strict Grid (No Mixels)', promptSnippet: 'strictly uniform pixel grid size everywhere, zero mixels, no sub-pixel rendering, every facial feature and detail locked to the same coarse pixel grid' },
      { id: 'px32', zh: '32×32 经典颗粒度', en: '32x32 Grid Density', promptSnippet: '32x32 retro pixel art resolution density, distinct chunky square pixels' },
      { id: 'px64', zh: '64×64 精细像素', en: '64x64 HD Pixel', promptSnippet: '64x64 detailed pixel art sprite, clean uniform pixel clusters' },
      { id: 'outline1px', zh: '1px 深色外描边', en: '1px Dark Outline', promptSnippet: 'clean 1-pixel dark outer contour around sprite' },
      { id: 'side_view', zh: '横版平台视角', en: 'Side-scroller View', promptSnippet: '2D side-scrolling platformer game view' },
      { id: 'topdown_rpg', zh: '俯视 RPG 视角', en: 'Top-down RPG View', promptSnippet: 'top-down 3/4 JRPG sprite perspective' }
    ]
  },
  {
    id: 'character_card',
    labelKey: 'arch_character_card',
    promptRule:
      'Full-body game character concept illustration, clear readable silhouette, standing pose, complete head-to-toe view, isolated character.',
    recommendedSize: '1024x1536',
    modifiers: [
      { id: 'apose', zh: '标准 A-Pose 设定站姿', en: 'Standard A-Pose', promptSnippet: 'symmetrical A-pose character concept standing posture, full body head to toe' },
      { id: 'dynamic', zh: '动态战斗姿态', en: 'Dynamic Action Pose', promptSnippet: 'dynamic combat action pose with clear silhouette' },
      { id: 'turnaround', zh: '正侧背三视图设定', en: '3-View Turnaround', promptSnippet: 'character turnaround sheet showing front view, side view, and back view on same canvas' },
      { id: 'layered_costume', zh: '层次感服饰材质', en: 'Layered Costume Detail', promptSnippet: 'well-defined costume material separation, leather, metal armor and fabric layers' }
    ]
  },
  {
    id: 'tile_texture',
    labelKey: 'arch_tile_texture',
    promptRule:
      'Orthographic top-down game surface texture / tileable material, even lighting without directional drop shadows, consistent pattern density.',
    recommendedSize: '1024x1024',
    modifiers: [
      { id: 'seamless', zh: '四方连续无缝循环', en: 'Seamless Tileable', promptSnippet: 'seamless four-way repeating tileable texture pattern' },
      { id: 'ortho90', zh: '90° 垂直正交俯视', en: '90° Orthographic Top-down', promptSnippet: 'strictly 90-degree flat orthographic top-down camera' },
      { id: 'flat_light', zh: '漫反射无方向阴影', en: 'Flat Diffuse Lighting', promptSnippet: 'uniform ambient diffuse lighting without directional cast shadows' },
      { id: 'handpainted_tex', zh: '手绘风格化肌理', en: 'Stylized Hand-painted', promptSnippet: 'stylized hand-painted surface brushwork and bevels' }
    ]
  },
  {
    id: 'ui_element',
    labelKey: 'arch_ui_element',
    promptRule:
      'Game UI frame / button / badge component, front-facing symmetrical orthographic view, crisp ornamental edges, empty center area, no text.',
    recommendedSize: '1024x1024',
    modifiers: [
      { id: 'nine_slice', zh: '九宫格可拉伸框体', en: '9-Slice Frame Ready', promptSnippet: 'symmetrical rectangular UI border frame with clean empty center for 9-slice scaling' },
      { id: 'ornate_corner', zh: '华丽金属雕花边角', en: 'Ornate Filigree Corners', promptSnippet: 'ornate fantasy gold filigree corner embellishments' },
      { id: 'hud_badge', zh: '技能/成就徽章底座', en: 'HUD Emblem Medallion', promptSnippet: 'circular or hexagonal game HUD skill medallion base' },
      { id: 'sci_hud', zh: '科幻全息几何框', en: 'Sci-Fi Hard-Surface HUD', promptSnippet: 'sleek sci-fi hard-surface geometric HUD panel frame' }
    ]
  },
  {
    id: 'free',
    labelKey: 'arch_free',
    promptRule: '',
    recommendedSize: 'auto',
    modifiers: [
      { id: 'clean_sil', zh: '清晰独立剪影', en: 'Clean Silhouette', promptSnippet: 'single isolated subject with clean readable silhouette' },
      { id: 'studio_light', zh: '影棚级体积光', en: 'Studio Rim Lighting', promptSnippet: 'dramatic three-point studio lighting with subtle rim light' },
      { id: 'game_ready', zh: '游戏资产级纯净度', en: 'Game-Ready Asset', promptSnippet: 'production-ready game art asset, crisp details, zero clutter' }
    ]
  }
];

export interface NegativeGuardrail {
  id: string;
  zh: string;
  en: string;
  negRule: string;
}

export const NEGATIVE_GUARDRAILS: NegativeGuardrail[] = [
  {
    id: 'no_text',
    zh: '禁文字/水印/UI字',
    en: 'No Text / Watermark',
    negRule: 'text, letters, typography, watermark, signature, logo, numbers'
  },
  {
    id: 'no_crop',
    zh: '禁边缘裁切残缺',
    en: 'No Cropped Edges',
    negRule: 'cropped out of frame, cut off edges, incomplete silhouette, touching canvas border'
  },
  {
    id: 'no_shadow',
    zh: '禁地面投射阴影',
    en: 'No Cast Ground Shadow',
    negRule: 'heavy drop shadow on ground, floor reflection, cast environment shadow'
  },
  {
    id: 'no_clutter',
    zh: '禁杂乱背景道具',
    en: 'No Background Clutter',
    negRule: 'busy background scenery, extra floating objects, multiple unrelated items'
  },
  {
    id: 'no_blur',
    zh: '禁景深模糊/噪点',
    en: 'No Depth Blur / Noise',
    negRule: 'depth of field blur, bokeh, motion blur, jpeg artifacts, muddy gradients'
  }
];

/**
 * Expands `[OptionA|OptionB|OptionC]` syntax in a prompt into up to 16 distinct prompts.
 * Example: "一把[火焰|寒冰|雷电]长剑" -> ["一把火焰长剑", "一把寒冰长剑", "一把雷电长剑"]
 */
export function expandPromptVariables(rawPrompt: string, maxVariants = 16): string[] {
  const regex = /\[([^\[\]\|]+(?:\|[^\[\]\|]+)+)\]/;
  let results = [rawPrompt.trim()];

  while (true) {
    let expanded = false;
    const nextBatch: string[] = [];
    for (const current of results) {
      const match = regex.exec(current);
      if (!match) {
        nextBatch.push(current);
        continue;
      }
      expanded = true;
      const options = match[1]!
        .split('|')
        .map((s) => s.trim())
        .filter(Boolean);
      if (options.length === 0) {
        nextBatch.push(current.replace(match[0], ''));
      } else {
        for (const opt of options) {
          nextBatch.push(current.replace(match[0], opt));
          if (nextBatch.length >= maxVariants) break;
        }
      }
      if (nextBatch.length >= maxVariants) break;
    }
    results = nextBatch.slice(0, maxVariants);
    if (!expanded || results.length >= maxVariants) break;
  }

  return results.filter(Boolean);
}

export interface StylePreset {
  id: string;
  labelKey: MessageKey;
  promptSuffix: string;
}

export const STYLE_PRESETS: StylePreset[] = [
  { id: 'default', labelKey: 'style_default', promptSuffix: '' },
  {
    id: 'pixel',
    labelKey: 'style_pixel',
    promptSuffix:
      '16-bit retro pixel art style, strictly uniform 1:1 square pixel grid, zero mixels (consistent pixel block size everywhere), crisp pixel edges, limited cohesive color palette, flat solid color per cell.'
  },
  {
    id: 'cartoon',
    labelKey: 'style_cartoon',
    promptSuffix:
      'Stylized hand-painted cartoon game art, bold clean outlines, vibrant saturated colors, expressive cel-shaded lighting.'
  },
  {
    id: 'anime',
    labelKey: 'style_anime',
    promptSuffix:
      'High-end anime game illustration style, crisp lineart, cel shading with soft rim light, rich material highlights.'
  },
  {
    id: 'render3d',
    labelKey: 'style_render3d',
    promptSuffix:
      '3D stylized isometric game asset render, tactile clay/resin material, soft global illumination, Octane/Blender studio quality.'
  },
  {
    id: 'lowpoly',
    labelKey: 'style_lowpoly',
    promptSuffix:
      'Stylized low-poly 3D game asset, crisp geometric facets, flat shaded polygons, clean minimal aesthetic.'
  },
  {
    id: 'realistic',
    labelKey: 'style_realistic',
    promptSuffix:
      'Next-gen PBR realistic game asset, intricate surface micro-details, physically accurate metallic and roughness response.'
  },
  {
    id: 'watercolor',
    labelKey: 'style_watercolor',
    promptSuffix:
      'Artistic storybook watercolor game illustration, delicate ink linework, organic pigment wash texture.'
  }
];

export interface BackgroundPreset {
  id: string;
  labelKey: MessageKey;
  promptRule: string;
}

export const BACKGROUND_PRESETS: BackgroundPreset[] = [
  {
    id: 'white',
    labelKey: 'bg_white',
    promptRule:
      'CRITICAL BACKGROUND RULE: Pure solid studio white background (#FFFFFF) only, no ground shadow, no backdrop gradients, no environment clutter.'
  },
  {
    id: 'gray',
    labelKey: 'bg_gray',
    promptRule:
      'CRITICAL BACKGROUND RULE: Pure uniform neutral middle-gray background (#808080) only, no ground shadow, no environment.'
  },
  {
    id: 'black',
    labelKey: 'bg_black',
    promptRule:
      'CRITICAL BACKGROUND RULE: Pure solid pitch-black background (#000000) only, no environment.'
  },
  {
    id: 'none',
    labelKey: 'bg_none',
    promptRule: ''
  }
];
