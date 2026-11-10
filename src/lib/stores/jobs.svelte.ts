import { api } from '$lib/api';
import type { AssetItem, GeneratePayload } from '$lib/api/types';
import { appStore } from './settings.svelte';

export interface JobUiSnapshot {
  archetype: string;
  style: string;
  bg: string;
  size: string;
  count: number;
  modifierIds: string[];
  negativeIds: string[];
  customNegative: string;
  sketchPreview: string | null;
}

export interface ActiveJob {
  id: string;
  prompt: string;
  model: string;
  count: number;
  startedAt: number;
  elapsedSec: string;
  status: 'running' | 'error';
  errorMsg?: string;
  payload: GeneratePayload;
  uiSnapshot?: JobUiSnapshot;
}

class JobsStore {
  activeJobs = $state<ActiveJob[]>([]);
  assets = $state<AssetItem[]>([]);
  sketchPreview = $state<string | null>(null);
  promptDraft = $state<string>('');

  #timer: ReturnType<typeof setInterval> | null = null;

  constructor() {
    this.#timer = setInterval(() => {
      const now = performance.now();
      for (const job of this.activeJobs) {
        if (job.status === 'running') {
          job.elapsedSec = ((now - job.startedAt) / 1000).toFixed(1);
        }
      }
    }, 150);
  }

  async loadAssets(project?: string) {
    try {
      const filter =
        project === undefined ? (appStore.settings?.activeProject ?? '') : project;
      this.assets = await api.listAssets(filter, 120);
    } catch (e: any) {
      appStore.toast('err', e?.message ?? 'Failed to load assets');
    }
  }

  async enqueueGeneration(payload: GeneratePayload, uiSnapshot?: JobUiSnapshot) {
    const jobId = 'job_' + Math.random().toString(36).slice(2, 9);
    const job: ActiveJob = $state({
      id: jobId,
      prompt: payload.prompt,
      model: payload.model,
      count: payload.count,
      startedAt: performance.now(),
      elapsedSec: '0.0',
      status: 'running',
      payload,
      uiSnapshot
    });

    this.activeJobs = [job, ...this.activeJobs];
    await this.#executeJob(job);
  }

  async retryJob(jobId: string) {
    const job = this.activeJobs.find((j) => j.id === jobId);
    if (!job || job.status === 'running') return;

    // Sync with currently active profile/model in case user just switched to a working provider/model
    if (appStore.activeProfile) {
      job.payload = {
        ...job.payload,
        profileId: appStore.activeProfile.id,
        model: appStore.activeProfile.apiModel || job.payload.model
      };
      job.model = job.payload.model;
    }

    job.status = 'running';
    job.errorMsg = undefined;
    job.startedAt = performance.now();
    job.elapsedSec = '0.0';

    await this.#executeJob(job);
  }

  async #executeJob(job: ActiveJob) {
    try {
      const res = await api.generateAssets(job.payload);
      this.activeJobs = this.activeJobs.filter((j) => j.id !== job.id);
      this.assets = [...res.assets, ...this.assets];
      appStore.notifyAssetsChanged();
      await appStore.refreshProjects();
    } catch (e: any) {
      job.status = 'error';
      job.errorMsg = e?.details ? `${e.message} — ${e.details}` : (e?.message ?? 'Unknown error');
      appStore.toast('err', job.errorMsg!);
    }
  }

  dismissJob(jobId: string) {
    this.activeJobs = this.activeJobs.filter((j) => j.id !== jobId);
  }
}

export const jobsStore = new JobsStore();
