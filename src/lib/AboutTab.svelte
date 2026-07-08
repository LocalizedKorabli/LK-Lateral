<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getTranslation, type Language } from "$lib/i18n";
  import { openUrl } from "@tauri-apps/plugin-opener";

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
  <img src="/favicon.png" alt="LK-Lateral" class="w-24 h-24 rounded-xl" />

  <h1 class="text-2xl font-bold">{t('app.title_brand')}</h1>
  <p class="text-sm text-muted -mt-4">{t('app.title_sub')}</p>

  <div class="flex items-center gap-2 text-muted">
    <span class="text-sm">v{appVersion}</span>
    {#if updateInfo}
      <span class="badge badge-success">{t('about.update_available', { version: updateInfo.version })}</span>
    {:else if !checkingUpdate}
      <span class="badge badge-muted">{t('label.latest')}</span>
    {/if}
  </div>

  {#if checkingUpdate}
    <button class="btn btn-sm btn-outline" disabled>
      <svg class="animate-spin h-3 w-3 text-muted" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
      {t('about.checking_update')}
    </button>
  {:else if downloadingUpdate}
    <div class="flex flex-col items-center gap-2 w-64">
      <progress class="progress w-full" value={downloadPercent} max="100"></progress>
      <span class="text-xs text-muted">{t('about.update_download_progress', { percent: String(downloadPercent) })}</span>
    </div>
  {:else if updateInfo}
    <button class="btn btn-sm btn-primary" onclick={doUpdate}>
      {t('about.update_now')}
    </button>
  {:else}
    <button class="btn btn-sm btn-outline" onclick={checkUpdate}>
      {t('about.check_update')}
    </button>
  {/if}

  <div class="flex flex-wrap items-center justify-center gap-1 mt-2">
    <button class="btn btn-ghost p-2" onclick={() => openUrl('https://localizedkorabli.org')} title={t('about.website')}>
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 12a9 9 0 01-9 9m9-9a9 9 0 00-9-9m9 9H3m9 9a9 9 0 01-9-9m9 9c1.657 0 3-4.03 3-9s-1.343-9-3-9m0 18c-1.657 0-3-4.03-3-9s1.343-9 3-9m-9 9a9 9 0 019-9" /></svg>
    </button>
    <button class="btn btn-ghost p-2" onclick={() => openUrl('https://github.com/LocalizedKorabli/LK-Lateral')} title={t('about.github')}>
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="currentColor" viewBox="0 0 16 16"><path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z"/></svg>
    </button>
    <button class="btn btn-ghost p-2" onclick={() => openUrl('https://www.gnu.org/licenses/agpl-3.0.html')} title={t('about.license')}>
      <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 6l3 1m0 0l-3 9a5.002 5.002 0 006.001 0M6 7l3 9M6 7l6-2m6 2l3-1m-3 1l-3 9a5.002 5.002 0 006.001 0M18 7l3 9m-3-9l-6-2m0-2v2m0 16V5m0 16H9m3 0h3" /></svg>
    </button>
  </div>

  <p class="text-xs text-faint text-center">
    LocalizedKorabli &copy; {new Date().getFullYear()}
  </p>
</div>

{#if toastMessage}
  <div class="toast-container">
    <div class="toast toast-{toastType}">
      {#if toastType === 'success'}
        <svg xmlns="http://www.w3.org/2000/svg" class="shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/></svg>
      {:else if toastType === 'error'}
        <svg xmlns="http://www.w3.org/2000/svg" class="shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
      {/if}
      <span>{toastMessage}</span>
    </div>
  </div>
{/if}
