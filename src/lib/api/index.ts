import { invoke, convertFileSrc } from '@tauri-apps/api/core';
import type {
  AppError,
  AppSettings,
  AssetItem,
  CharacterGeneratePayload,
  CharacterGenerateResponse,
  GeneratePayload,
  GenerateResponse,
  MattingModelInfo,
  MattingRequestPayload,
  MattingResponsePayload,
  ProjectInfo,
  RegionEditPayload,
  RegionEditResponse
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
    call<CharacterGenerateResponse>('generate_character_asset', { payload })
};
