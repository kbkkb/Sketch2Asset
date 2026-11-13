export type AppErrorCode =
  | 'invalid_input'
  | 'not_found'
  | 'missing_api_key'
  | 'provider_auth'
  | 'provider_rate_limited'
  | 'provider_bad_response'
  | 'provider_unavailable'
  | 'network'
  | 'image_decode'
  | 'matting_failed'
  | 'model_missing'
  | 'io'
  | 'database'
  | 'internal';

export interface AppError {
  code: AppErrorCode;
  message: string;
  details?: string | null;
}

export interface ComfyWorkflowConfig {
  mode: 'builtin' | 'custom';
  workflowName: string;
  rawWorkflowJson: string;
  positiveNodeId: string;
  positiveField: string;
  negativeNodeId: string;
  negativeField: string;
  imageNodeId: string;
  secondaryImageNodeId: string;
  latentNodeId: string;
  seedNodeId: string;
  denoiseStrength: number;
}

export interface ComfyNodeSlotOption {
  nodeId: string;
  classType: string;
  title: string;
  field: string;
  preview: string;
}

export interface ComfyWorkflowAnalysis {
  format: 'api' | 'ui_converted' | 'png_embedded' | string;
  nodeCount: number;
  normalizedApiJson: string;
  textNodes: ComfyNodeSlotOption[];
  imageNodes: ComfyNodeSlotOption[];
  latentNodes: ComfyNodeSlotOption[];
  seedNodes: ComfyNodeSlotOption[];
  suggestedPositiveNodeId: string;
  suggestedPositiveField: string;
  suggestedNegativeNodeId: string;
  suggestedNegativeField: string;
  suggestedImageNodeId: string;
  suggestedSecondaryImageNodeId: string;
  suggestedLatentNodeId: string;
  suggestedSeedNodeId: string;
}

export interface ProviderProfile {
  id: string;
  name: string;
  apiBase: string;
  apiMode: 'images' | 'chat' | 'comfyui';
  apiModel: string;
  /** Enabled models shown in the bottom Command Dock of Asset Forge */
  models: string[];
  /** Full pool of models fetched from remote `/v1/models` */
  fetchedModels?: string[];
  hasKey: boolean;
  comfyConfig?: ComfyWorkflowConfig | null;
}

export interface AppSettings {
  version: number;
  theme: 'dark' | 'light';
  language: 'zh-CN' | 'en';
  activeProfileId: string;
  profiles: ProviderProfile[];
  activeProject: string;
  defaultArchetype: string;
  defaultStyle: string;
  defaultBackground: string;
  defaultSize: string;
}

export interface ProjectInfo {
  name: string;
  style: string;
  stylePrompt: string;
  referenceImages: string[];
  assetCount: number;
}

export interface AssetItem {
  id: string;
  project: string;
  archetype?: string;
  filePath: string;
  prompt: string;
  model: string;
  via: string;
  width: number;
  height: number;
  createdAt: number;
}

export interface GeneratePayload {
  profileId: string;
  model: string;
  prompt: string;
  archetype?: string;
  negativePrompt?: string;
  archetypePrompt: string;
  stylePrompt: string;
  projectStylePrompt: string;
  backgroundPrompt: string;
  size: string;
  project: string;
  count: number;
  sketchDataUrl?: string | null;
  sketchPath?: string | null;
}

export interface GenerateResponse {
  assets: AssetItem[];
  elapsedMs: number;
}

export interface MattingModelInfo {
  id: string;
  name: string;
  fileName: string;
  description: string;
  sizeMb: number;
  installed: boolean;
  filePath?: string | null;
  urls: string[];
}

export interface MattingRequestPayload {
  imageDataUrl?: string | null;
  imagePath?: string | null;
  modelId: string;
  device: 'auto' | 'gpu' | 'cpu';
  enhanced: boolean;
  choke: number;
}

export interface MattingResponsePayload {
  imageDataUrl: string;
  width: number;
  height: number;
  providerInfo: string;
  elapsedMs: number;
}

export interface RegionAnnotationItem {
  index: number;
  x: number;
  y: number;
  w: number;
  h: number;
  note: string;
}

export interface RegionEditPayload {
  profileId: string;
  model: string;
  mode: 'visual_guide' | 'mask_edit';
  originalDataUrl: string;
  guideDataUrl?: string | null;
  maskDataUrl?: string | null;
  annotations: RegionAnnotationItem[];
  globalNote?: string;
  size?: string;
}

export interface RegionEditResponse {
  imageDataUrl: string;
  via: string;
  elapsedMs: number;
}

export interface CharacterGeneratePayload {
  profileId: string;
  model: string;
  subMode: 'turnaround' | 'outfit' | 'spritesheet';
  prompt: string;
  stylePrompt: string;
  negativePrompt?: string;
  size: string;
  project: string;
  archetype: string;
  anchorDataUrl?: string | null;
  secondaryRefDataUrl?: string | null;
}

export interface CharacterGenerateResponse {
  asset: AssetItem;
  imageDataUrl: string;
  via: string;
  elapsedMs: number;
}

export interface AppUpdateInfo {
  currentVersion: string;
  latestVersion: string;
  hasUpdate: boolean;
  releaseUrl: string;
  releaseNotes: string;
  publishedAt: string;
}

export interface AgentTargetInfo {
  id: string;
  name: string;
  vendor: string;
  detected: boolean;
  mcpInstalled: boolean;
  skillInstalled: boolean;
  mcpConfigPath: string;
  skillPath: string;
}

export interface AgentHubStatus {
  mcpExePath: string;
  appSkillDir: string;
  mcpJsonSnippet: string;
  codexTomlSnippet: string;
  cliExampleSnippet: string;
  skillMarkdown: string;
  agents: AgentTargetInfo[];
}
