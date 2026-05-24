<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getTranslation, type Language } from "$lib/i18n";

  let { lang = 'zh_CN' }: { lang?: Language } = $props();

  let appVersion: string = $state('');
  let updateInfo: { version: string; path: string } | null = $state(null);
  let checkingUpdate: boolean = $state(false);
  let downloadingUpdate: boolean = $state(false);
  let downloadPercent: number = $state(0);
  let toastMessage: string | null = $state(null);
  let toastType: 'success' | 'error' | 'info' = $state('info');

  function t(key: string, params?: Record<string, string>): string {
    return getTranslation(key, lang, params);
  }

  function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
    toastMessage = message;
    toastType = type;
    setTimeout(() => { toastMessage = null; }, 4000);
  }

  async function loadInfo() {
    try {
      appVersion = await invoke<string>('get_app_version');
    } catch {
      appVersion = '0.0.0';
    }
  }

  async function checkUpdate() {
    checkingUpdate = true;
    try {
      updateInfo = await invoke<{ version: string; path: string } | null>('check_lateral_update');
    } catch {
      updateInfo = null;
      showToast(t('error.update_check_failed'), 'error');
    }
    checkingUpdate = false;
  }

  async function doUpdate() {
    if (!updateInfo) return;
    downloadingUpdate = true;
    downloadPercent = 0;
    try {
      await invoke('download_and_install_update', { downloadUrl: updateInfo.path });
    } catch {
      downloadingUpdate = false;
      showToast(t('error.update_download_failed'), 'error');
    }
  }

  const unlisteners: (() => void)[] = [];

  $effect(() => {
    return () => {
      for (const fn of unlisteners) fn();
    };
  });

  $effect(() => {
    loadInfo();
  });

  $effect(() => {
    if (appVersion) checkUpdate();
  });

  $effect(() => {
    const cancel = listen<{ percent: number; downloaded_bytes: number; total_bytes: number }>(
      'update-download-progress',
      (event) => {
        downloadPercent = event.payload.percent;
      }
    );
    cancel.then((fn) => { unlisteners.push(fn); });
  });
</script>

<div class="flex flex-col items-center justify-center h-full gap-6 p-8">
  <img src="/favicon.png" alt="LK-Lateral" class="w-24 h-24 rounded-xl shadow-lg" />

  <h1 class="text-2xl font-bold">{t('app.title')}</h1>

  <div class="flex items-center gap-2 text-base-content/70">
    <span class="text-sm">v{appVersion}</span>
    {#if updateInfo}
      <span class="badge badge-success badge-xs">{t('about.update_available', { version: updateInfo.version })}</span>
    {:else if !checkingUpdate}
      <span class="badge badge-primary badge-xs">{t('label.latest')}</span>
    {/if}
  </div>

  {#if checkingUpdate}
    <button class="btn btn-outline btn-sm" disabled>
      <span class="loading loading-spinner loading-xs"></span>
      {t('about.checking_update')}
    </button>
  {:else if downloadingUpdate}
    <div class="flex flex-col items-center gap-2 w-64">
      <progress class="progress progress-primary w-full" value={downloadPercent} max="100"></progress>
      <span class="text-xs text-base-content/60">{t('about.update_download_progress', { percent: String(downloadPercent) })}</span>
    </div>
  {:else if updateInfo}
    <button class="btn btn-primary btn-sm" onclick={doUpdate}>
      {t('about.update_now')}
    </button>
  {:else}
    <button class="btn btn-outline btn-sm" onclick={checkUpdate}>
      {t('about.check_update')}
    </button>
  {/if}

  <div class="divider w-full"></div>

  <p class="text-xs text-base-content/40 text-center">
    LocalizedKorabli &copy; {new Date().getFullYear()}
  </p>
</div>

{#if toastMessage}
  <div class="toast toast-bottom toast-center">
    <div class="alert" class:alert-success={toastType === 'success'} class:alert-error={toastType === 'error'} class:alert-info={toastType === 'info'}>
      <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        {#if toastType === 'success'}
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
        {:else if toastType === 'error'}
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
        {:else}
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        {/if}
      </svg>
      <span>{toastMessage}</span>
    </div>
  </div>
{/if}
