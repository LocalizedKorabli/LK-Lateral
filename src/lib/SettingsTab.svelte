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
  <div class="card">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4 flex items-center gap-2">
        {t('settings.language')}
        <svg class="h-4 w-4 text-muted shrink-0" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m5 8 6 6"/><path d="m4 14 6-6 2-3"/><path d="M2 5h12"/><path d="M7 2h1"/><path d="m22 22-5-10-5 10"/><path d="M14 18h6"/></svg>
      </h3>
      <select
        class="select w-full max-w-xs"
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

  <div class="card">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4">{t('settings.proxy')}</h3>

      <div class="form-control mb-2">
        <label class="label cursor-pointer justify-start gap-3 py-1">
          <input
            type="radio"
            name="proxyMode"
            class="radio"
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
            class="radio"
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
                class="input w-full"
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
                class="input w-full"
                bind:value={proxyConfig.custom_port}
                placeholder="7890"
              />
            </label>
          </div>
          <details class="pl-1">
            <summary class="cursor-pointer text-xs font-medium text-muted py-1 select-none list-none">{t('settings.auth_optional')}
              <svg class="inline-block h-3 w-3 ml-0.5 transition-transform " xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"/></svg>
            </summary>
            <div class="flex flex-col gap-3 pt-2">
              <label class="form-control w-full">
                <div class="label">
                  <span class="label-text">{t('settings.username')}</span>
                </div>
                <input
                  type="text"
                  class="input w-full"
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
                  class="input w-full"
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
          <button class="btn btn-sm btn-primary" onclick={saveProxy} disabled={loading}>
            {#if loading}
              <svg class="animate-spin h-3 w-3 text-muted" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
            {/if}
            {t('button.save')}
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="card">
    <div class="card-body">
      <h3 class="card-title text-lg mb-4">{t('settings.files')}</h3>

      <div class="flex items-center justify-between py-1 gap-3">
        <span class="text-sm truncate" title={dataDir}>
          <span class="text-muted">{t('settings.data_dir_label')}:</span>
          <span class="text-xs font-mono">{dataDir}</span>
        </span>
        <button class="btn btn-outline btn-sm shrink-0" onclick={openDataDir}>{t('settings.open_dir')}</button>
      </div>

      <div class="flex items-center justify-between py-1 gap-3">
        <span class="text-sm"><span class="text-muted">{t('settings.cache')}:</span> {cacheSize}</span>
        <button class="btn btn-outline btn-sm shrink-0" onclick={() => (showClearCacheConfirm = true)}>{t('settings.clear_cache')}</button>
      </div>
    </div>
  </div>
</div>

{#if showClearCacheConfirm}
  <div class="modal-backdrop" style="z-index: 1000;">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('settings.clear_cache_confirm_title')}</h3>
      <p class="mb-4 text-sm whitespace-pre-line">{t('settings.clear_cache_confirm_msg')}</p>
      <div class="modal-action">
        <button class="btn btn-sm btn-outline" onclick={() => (showClearCacheConfirm = false)} disabled={clearingCache}>{t('button.cancel')}</button>
        <button class="btn btn-sm btn-danger" onclick={doClearCache} disabled={clearingCache}>
          {#if clearingCache}
            <svg class="animate-spin h-3 w-3 text-muted" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"/><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"/></svg>
          {/if}
          {t('button.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}

{#if toastMessage}
  <div class="toast-container">
    <div class="toast toast-{toastType}">
      {#if toastType === 'success'}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m9 12 2 2 4-4"/></svg>
      {:else if toastType === 'error'}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="m15 9-6 6"/><path d="m9 9 6 6"/></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
      {/if}
      <span class="text-sm font-medium">{toastMessage}</span>
    </div>
  </div>
{/if}
