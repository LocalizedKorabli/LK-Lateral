<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { getTranslation, LANGUAGE_DISPLAY, type Language } from "$lib/i18n";

  let { lang = 'zh_CN', onLangChange }: { lang?: Language; onLangChange?: (lang: Language) => void } = $props();

  interface ProxyConfig {
    use_system_proxy: boolean;
    custom_host: string;
    custom_port: number;
    custom_username: string | null;
  }

  interface AppConfig {
    language: string;
    proxy: ProxyConfig;
  }

  let proxyConfig: ProxyConfig = $state({
    use_system_proxy: true,
    custom_host: '',
    custom_port: 7890,
    custom_username: null,
  });
  let proxyPassword: string = $state('');
  let savedProxy: ProxyConfig = $state({
    use_system_proxy: true,
    custom_host: '',
    custom_port: 7890,
    custom_username: null,
  });
  let savedProxyPassword: string = $state('');
  let proxyDirty: boolean = $derived(
    proxyConfig.use_system_proxy !== savedProxy.use_system_proxy ||
    proxyConfig.custom_host !== savedProxy.custom_host ||
    proxyConfig.custom_port !== savedProxy.custom_port ||
    (proxyConfig.custom_username || '') !== (savedProxy.custom_username || '') ||
    proxyPassword !== savedProxyPassword
  );
  let loading: boolean = $state(false);
  let dataDir: string = $state('');
  let cacheSize: string = $state('');
  let clearingCache: boolean = $state(false);
  let showClearCacheConfirm: boolean = $state(false);
  let toastMessage: string | null = $state(null);
  let toastType: 'success' | 'error' | 'info' = $state('info');

  function t(key: string): string {
    return getTranslation(key, lang);
  }

  function showToast(message: string, type: 'success' | 'error' | 'info' = 'info') {
    toastMessage = message;
    toastType = type;
    setTimeout(() => {
      toastMessage = null;
    }, 4000);
  }

  async function loadConfig() {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      if (config.proxy) {
        const p = {
          use_system_proxy: config.proxy.use_system_proxy,
          custom_host: config.proxy.custom_host || '',
          custom_port: config.proxy.custom_port || 7890,
          custom_username: config.proxy.custom_username || null,
        };
        proxyConfig = { ...p };
        savedProxy = { ...p };
      }
    } catch {}
    try {
      const savedPassword = await invoke<string | null>('load_proxy_password');
      if (savedPassword) {
        proxyPassword = savedPassword;
        savedProxyPassword = savedPassword;
      }
    } catch {
      proxyPassword = '';
      savedProxyPassword = '';
    }
    try {
      dataDir = await invoke<string>('get_data_dir');
    } catch {
      dataDir = '-';
    }
    try {
      cacheSize = await invoke<string>('get_cache_size');
    } catch {
      cacheSize = '-';
    }
  }

  async function handleLanguageChange(newLang: Language) {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      config.language = newLang;
      await invoke('save_app_config', { appConfig: config });
      onLangChange?.(newLang);
      showToast(t('dialog.settings_saved'), 'success');
    } catch {
      showToast(t('error.scan_failed'), 'error');
    }
  }

  async function saveProxy() {
    if (!proxyConfig.use_system_proxy) {
      if (!proxyConfig.custom_host.trim()) {
        showToast(t('settings.proxy_host_required'), 'error');
        return;
      }
      if (!proxyConfig.custom_port) {
        showToast(t('settings.proxy_port_required'), 'error');
        return;
      }
    }
    loading = true;
    try {
      const config = await invoke<AppConfig>('get_app_config');
      config.proxy = {
        use_system_proxy: proxyConfig.use_system_proxy,
        custom_host: proxyConfig.custom_host,
        custom_port: proxyConfig.custom_port,
        custom_username: proxyConfig.custom_username || null,
      };
      await invoke('save_app_config', { appConfig: config });
      if (proxyPassword) {
        await invoke('save_proxy_password', { password: proxyPassword });
      } else {
        await invoke('delete_proxy_password');
      }
      savedProxy = { ...proxyConfig };
      savedProxyPassword = proxyPassword;
      showToast(t('dialog.settings_saved'), 'success');
    } catch {
      showToast(t('error.scan_failed'), 'error');
    }
    loading = false;
  }

  async function openDataDir() {
    try {
      const dir = await invoke<string>('get_data_dir');
      await revealItemInDir(dir);
    } catch {
      showToast(t('error.scan_failed'), 'error');
    }
  }

  async function doClearCache() {
    clearingCache = true;
    try {
      await invoke('clear_cache');
      showClearCacheConfirm = false;
      showToast(t('settings.clear_cache_success'), 'success');
      try {
        cacheSize = await invoke<string>('get_cache_size');
      } catch {
        cacheSize = '-';
      }
    } catch {
      showToast(t('error.scan_failed'), 'error');
    }
    clearingCache = false;
  }

  $effect(() => {
    loadConfig();
  });
</script>

<div class="flex flex-col gap-6 max-w-xl mx-auto p-4">
  <div class="card card-bordered bg-base-100 shadow-sm">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4">{t('settings.language')}</h3>
      <select
        class="select select-bordered w-full max-w-xs"
        value={lang}
        onchange={(e: Event) => {
          const target = e.target as HTMLSelectElement;
          handleLanguageChange(target.value as Language);
        }}
      >
        {#each Object.entries(LANGUAGE_DISPLAY) as [code, name]}
          <option value={code}>{name}</option>
        {/each}
      </select>
    </div>
  </div>

  <div class="card card-bordered bg-base-100 shadow-sm">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4">{t('settings.proxy')}</h3>

      <div class="form-control mb-2">
        <label class="label cursor-pointer justify-start gap-3 py-1">
          <input
            type="radio"
            name="proxyMode"
            class="radio radio-primary radio-sm"
            checked={proxyConfig.use_system_proxy}
            onchange={() => (proxyConfig = { ...proxyConfig, use_system_proxy: true })}
          />
          <span class="label-text">{t('settings.use_system_proxy')}</span>
        </label>
      </div>
      <div class="form-control mb-4">
        <label class="label cursor-pointer justify-start gap-3 py-1">
          <input
            type="radio"
            name="proxyMode"
            class="radio radio-primary radio-sm"
            checked={!proxyConfig.use_system_proxy}
            onchange={() => (proxyConfig = { ...proxyConfig, use_system_proxy: false, custom_host: proxyConfig.custom_host || '127.0.0.1', custom_port: proxyConfig.custom_port || 7890 })}
          />
          <span class="label-text">{t('settings.custom_proxy')}</span>
        </label>
      </div>

      {#if !proxyConfig.use_system_proxy}
        <div class="flex flex-col gap-3 pl-8">
          <div class="flex gap-3">
            <label class="form-control flex-1">
              <div class="label">
                <span class="label-text">{t('settings.host')}</span>
              </div>
              <input
                type="text"
                class="input input-bordered w-full"
                bind:value={proxyConfig.custom_host}
                placeholder="127.0.0.1"
              />
            </label>
            <label class="form-control w-28">
              <div class="label">
                <span class="label-text">{t('settings.port')}</span>
              </div>
              <input
                type="number"
                class="input input-bordered w-full"
                bind:value={proxyConfig.custom_port}
                placeholder="7890"
              />
            </label>
          </div>
          <details class="pl-1">
            <summary class="cursor-pointer text-sm font-medium py-1 select-none">{t('settings.auth_optional')}</summary>
            <div class="flex flex-col gap-3 pt-2">
              <label class="form-control w-full">
                <div class="label">
                  <span class="label-text">{t('settings.username')}</span>
                </div>
                <input
                  type="text"
                  class="input input-bordered w-full"
                  bind:value={proxyConfig.custom_username}
                  placeholder=""
                />
              </label>
              <label class="form-control w-full">
                <div class="label">
                  <span class="label-text">{t('settings.password')}</span>
                </div>
                <input
                  type="password"
                  class="input input-bordered w-full"
                  bind:value={proxyPassword}
                  placeholder=""
                />
              </label>
            </div>
          </details>
        </div>
      {/if}

      <div class="card-actions mt-4">
        {#if proxyDirty}
          <button class="btn btn-primary" onclick={saveProxy} disabled={loading}>
            {#if loading}
              <span class="loading loading-spinner loading-xs"></span>
            {/if}
            {t('button.save')}
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="card card-bordered bg-base-100 shadow-sm">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4">{t('settings.files')}</h3>

      <div class="flex items-center justify-between py-1">
        <span class="text-sm"><span class="text-base-content/60">{t('settings.data_dir_label')}:</span> <span class="text-xs font-mono break-all">{dataDir}</span></span>
        <button class="btn btn-outline btn-sm shrink-0 ml-3" onclick={openDataDir}>{t('settings.open_dir')}</button>
      </div>

      <div class="flex items-center justify-between py-1">
        <span class="text-sm"><span class="text-base-content/60">{t('settings.cache')}:</span> {cacheSize}</span>
        <button class="btn btn-outline btn-sm shrink-0 ml-3" onclick={() => (showClearCacheConfirm = true)}>{t('settings.clear_cache')}</button>
      </div>
    </div>
  </div>
</div>

{#if showClearCacheConfirm}
  <div class="modal modal-open" style="z-index: 1000;">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('settings.clear_cache_confirm_title')}</h3>
      <p class="mb-4 text-sm whitespace-pre-line">{t('settings.clear_cache_confirm_msg')}</p>
      <div class="modal-action">
        <button class="btn btn-outline btn-sm" onclick={() => (showClearCacheConfirm = false)} disabled={clearingCache}>{t('button.cancel')}</button>
        <button class="btn btn-error btn-sm" onclick={doClearCache} disabled={clearingCache}>
          {#if clearingCache}
            <span class="loading loading-spinner loading-xs"></span>
          {/if}
          {t('button.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if toastMessage}
  <div class="toast toast-bottom toast-center z-50 mb-4">
    <div class="alert alert-{toastType} shadow-lg gap-3 min-w-[320px]">
      {#if toastType === 'success'}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      {:else if toastType === 'error'}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      {/if}
      <span class="text-sm font-medium">{toastMessage}</span>
    </div>
  </div>
{/if}
