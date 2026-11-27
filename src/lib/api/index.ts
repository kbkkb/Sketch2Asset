import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type {
  AgentHubStatus,
  AppError,
  AppSettings,
  AppUpdateInfo,
  AssetItem,
  CharacterGeneratePayload,
  CharacterGenerateResponse,
  ComfyWorkflowAnalysis,
  GeneratePayload,
  GenerateResponse,
  MattingModelInfo,
  MattingRequestPayload,
  MattingResponsePayload,
  McpPoseBridgeRequest,
  PoseActionClip,
  ProjectInfo,
  RegionEditPayload,
  RegionEditResponse,
  DecomposeEnvStatus,
  DecomposeJobResult,
  DecomposeHistoryItem,
  DecomposeLayerInfo,
  DualBgMattingPayload,
  RunDecomposeOptions
} from './types';

function normalizeError(err: unknown): AppError {
  if (err && typeof err === 'object' && 'code' in err && 'message' in err) {
    return err as AppError;
  }
  return {
    code: 'internal',
    message: err instanceof Error ? err.message : String(err)
  };
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw normalizeError(e);
  }
}

export function assetUrl(filePath: string): string {
  if (!filePath) return '';
  if (filePath.startsWith('data:') || filePath.startsWith('http')) return filePath;
  return convertFileSrc(filePath);
}

export const api = {
  getSettings: () => call<AppSettings>('get_settings'),
  saveSettings: (newSettings: AppSettings) =>
    call<AppSettings>('save_settings', { newSettings }),
  setApiKey: (profileId: string, apiKey: string) =>
    call<AppSettings>('set_api_key', { profileId, apiKey }),
  getApiKey: (profileId: string) =>
    call<string>('get_api_key', { profileId }),
  getApiKeyMasked: (profileId: string) =>
    call<string>('get_api_key_masked', { profileId }),
  fetchProviderModels: (profileId: string, apiBase: string, apiKeyOverride?: string) =>
    call<string[]>('fetch_provider_models', {
      profileId,
      apiBase,
      apiKeyOverride: apiKeyOverride || null
    }),
  analyzeComfyuiWorkflow: (apiBase: string, rawContent: string) =>
    call<ComfyWorkflowAnalysis>('analyze_comfyui_workflow', { apiBase, rawContent }),
  generateAssets: (payload: GeneratePayload) =>
    call<GenerateResponse>('generate_assets', { payload }),
  listProjects: () => call<ProjectInfo[]>('list_projects'),
  upsertProject: (
    name: string,
    style: string,
    stylePrompt: string,
    referenceImages?: string[]
  ) =>
    call<ProjectInfo[]>('upsert_project', {
      name,
      style,
      stylePrompt,
      referenceImages: referenceImages ?? null
    }),
  renameProject: (oldName: string, newName: string) =>
    call<ProjectInfo[]>('rename_project', { oldName, newName }),
  listAssets: (project?: string, limit = 100) =>
    call<AssetItem[]>('list_assets', { project: project ?? null, limit }),
  getLibraryRevision: () => call<string>('get_library_revision'),
  deleteAsset: (id: string) => call<void>('delete_asset', { id }),
  updateAssetArchetype: (id: string, archetype: string) =>
    call<void>('update_asset_archetype', { id, archetype }),
  revealInExplorer: (filePath: string) =>
    call<void>('reveal_in_explorer', { filePath }),
  exportConfigBundle: () => call<string>('export_config_bundle'),
  importConfigBundle: (rawJson: string) =>
    call<AppSettings>('import_config_bundle', { rawJson }),
  listMattingModels: () => call<MattingModelInfo[]>('list_matting_models'),
  downloadMattingModel: (modelId: string) =>
    call<MattingModelInfo[]>('download_matting_model', { modelId }),
  deleteMattingModel: (modelId: string) =>
    call<MattingModelInfo[]>('delete_matting_model', { modelId }),
  openModelsFolder: () => call<void>('open_models_folder'),
  runAiMatting: (payload: MattingRequestPayload) =>
    call<MattingResponsePayload>('run_ai_matting', { payload }),
  runDualBgMatting: (payload: DualBgMattingPayload) =>
    call<MattingResponsePayload>('run_dual_bg_matting', { payload }),
  readImageDataUrl: (filePath: string) =>
    call<string>('read_image_data_url', { filePath }),
  saveStudioAsset: (
    imageDataUrl: string,
    project: string,
    archetype: string,
    promptLabel?: string
  ) =>
    call<AssetItem>('save_studio_asset', {
      imageDataUrl,
      project,
      archetype,
      promptLabel: promptLabel ?? null
    }),
  exportStudioPng: (imageDataUrl: string, fileName: string, reveal = true) =>
    call<string>('export_studio_png', { imageDataUrl, fileName, reveal }),
  runStudioRegionEdit: (payload: RegionEditPayload) =>
    call<RegionEditResponse>('run_studio_region_edit', { payload }),
  generateCharacterAsset: (payload: CharacterGeneratePayload) =>
    call<CharacterGenerateResponse>('generate_character_asset', { payload }),
  checkAppUpdate: () => call<AppUpdateInfo>('check_app_update'),
  applyAppUpdate: (downloadUrl: string) => call<void>('apply_app_update', { downloadUrl }),
  openExternalUrl: (url: string) => call<void>('open_external_url', { url }),
  performWindowClose: (action: 'tray' | 'exit') =>
    call<void>('perform_window_close', { action }),
  getAgentHubStatus: () => call<AgentHubStatus>('get_agent_hub_status'),
  installAgentIntegration: (agentId: string) =>
    call<AgentHubStatus>('install_agent_integration', { agentId }),
  uninstallAgentIntegration: (agentId: string) =>
    call<AgentHubStatus>('uninstall_agent_integration', { agentId }),
  exportAgentSkillBundle: (reveal = true) =>
    call<string>('export_agent_skill_bundle', { reveal }),
  listPoseActions: () => call<PoseActionClip[]>('list_pose_actions'),
  savePoseAction: (clip: PoseActionClip) =>
    call<PoseActionClip>('save_pose_action', { clip }),
  deletePoseAction: (id: string) =>
    call<boolean>('delete_pose_action', { id }),
  renderPoseActionGrid: (params: {
    actionId: string;
    rows: number;
    cols: number;
    width?: number;
    height?: number;
  }) => call<string>('render_pose_action_grid', params),
  pollMcpPoseRequest: () =>
    call<McpPoseBridgeRequest | null>('poll_mcp_pose_request'),
  completeMcpPoseRequest: (reqId: string, dataUrl: string) =>
    call<boolean>('complete_mcp_pose_request', { reqId, dataUrl }),
  getDecomposeStatus: () => call<DecomposeEnvStatus>('get_decompose_status'),
  pickDecomposeDir: (initialDir?: string) =>
    call<string | null>('pick_decompose_dir', { initialDir: initialDir ?? null }),
  setDecomposeDir: (dirPath: string) =>
    call<DecomposeEnvStatus>('set_decompose_dir', { dirPath }),
  installDecomposeExtension: (
    targetDir: string,
    quantMode: string,
    mirror: string,
    pypiMirror?: string
  ) =>
    call<DecomposeEnvStatus>('install_decompose_extension', {
      targetDir,
      quantMode,
      mirror,
      pypiMirror: pypiMirror ?? null
    }),
  cancelDecomposeInstall: () => call<void>('cancel_decompose_install'),
  cancelDecomposeRun: () => call<void>('cancel_decompose_run'),
  cancelMcpTask: (taskId: string) => call<boolean>('cancel_mcp_task', { taskId }),
  uninstallDecomposeExtension: () =>
    call<DecomposeEnvStatus>('uninstall_decompose_extension'),
  runDecomposeCharacter: (opts: RunDecomposeOptions) =>
    call<DecomposeJobResult>('run_decompose_character', { opts }),
  saveDecomposeLayers: (manifestPath: string, layers: DecomposeLayerInfo[]) =>
    call<void>('save_decompose_layers', { manifestPath, layers }),
  splitDecomposeLayer: (
    manifestPath: string,
    layers: DecomposeLayerInfo[],
    layerId: string,
    mode: 'lr' | 'depth'
  ) =>
    call<DecomposeJobResult>('split_decompose_layer', {
      manifestPath,
      layers,
      layerId,
      mode
    }),
  listDecomposeHistory: () =>
    call<DecomposeHistoryItem[]>('list_decompose_history'),
  loadDecomposeJob: (manifestPath: string) =>
    call<DecomposeJobResult>('load_decompose_job', { manifestPath }),
  defringeDecomposeLayer: (layerImagePath: string, chokePx: number) =>
    call<string>('defringe_decompose_layer', { layerImagePath, chokePx }),
  exportDecomposePsd: (
    manifestPath: string,
    layers: DecomposeLayerInfo[],
    outPsdPath?: string,
    reveal = true
  ) =>
    call<string>('export_decompose_psd', {
      manifestPath,
      layers,
      outPsdPath: outPsdPath ?? null,
      reveal
    }),
  exportDecomposeBundle: (
    manifestPath: string,
    destDir: string,
    layers: DecomposeLayerInfo[],
    reveal = true
  ) =>
    call<string>('export_decompose_bundle', {
      manifestPath,
      destDir,
      layers,
      reveal
    }),
  pickExportDir: (initialDir?: string) =>
    call<string | null>('pick_export_dir', { initialDir: initialDir ?? null })
};
