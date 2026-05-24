<script lang="ts">
  import HomeTab from "$lib/HomeTab.svelte";
  import SettingsTab from "$lib/SettingsTab.svelte";
  import AboutTab from "$lib/AboutTab.svelte";
  import { getTranslation, type Language } from "$lib/i18n";
  import { invoke } from "@tauri-apps/api/core";

  let activeTab: number = $state(0);
  let lang: Language = $state('zh_CN');
  let theme: 'light' | 'dark' = $state('light');

  let gLgcPath: string = $state('');
  let gMostPath: string = $state('');
  let gLgcRunning: boolean = $state(false);
  let gMostRunning: boolean = $state(false);

  let key = $state(0);

  function t(keyStr: string): string {
    return getTranslation(keyStr, lang);
  }

  async function checkRunning() {
    if (gLgcPath) gLgcRunning = await invoke<boolean>('is_app_running', { installPath: gLgcPath });
    if (gMostPath) gMostRunning = await invoke<boolean>('is_app_running', { installPath: gMostPath });
  }

  let runningPollTimer: ReturnType<typeof setInterval> | null = null;
  $effect(() => {
    checkRunning();
    runningPollTimer = setInterval(checkRunning, 5000);
    return () => { if (runningPollTimer) { clearInterval(runningPollTimer); runningPollTimer = null; } };
  });

  function onPathsChange(lgcPath: string, mostPath: string) {
    gLgcPath = lgcPath;
    gMostPath = mostPath;
  }

  async function onRefreshRunning() {
    await checkRunning();
  }

  async function loadConfig() {
    try {
      const langCode = await invoke<string>('resolve_initial_language');
      lang = langCode as Language;
      const config = await invoke<{ language: string; theme: string }>('get_app_config');
      if (config.language) {
        lang = config.language as Language;
      }
      if (config.theme === 'dark') {
        theme = 'dark';
      }
    } catch {
      lang = 'en';
    }
  }

  function onLangChange(newLang: Language) {
    lang = newLang;
    key++;
  }

  async function onThemeToggle() {
    const next = theme === 'dark' ? 'light' : 'dark';
    theme = next;
    try {
      const config = await invoke<{ language: string; theme: string }>('get_app_config');
      config.theme = next;
      await invoke('save_app_config', { appConfig: config });
    } catch {}
  }

  $effect(() => {
    document.documentElement.setAttribute('data-theme', theme);
  });

  $effect(() => {
    loadConfig();
  });
</script>

<div class="flex flex-col h-screen">
  <div class="flex items-center tabs-bordered px-4 pt-3 bg-base-100">
    <div class="tabs grow">
      <button
        class="tab tab-lg"
        class:tab-active={activeTab === 0}
        onclick={() => (activeTab = 0)}
      >
        {t('menu.home')}
      </button>
      <button
        class="tab tab-lg"
        class:tab-active={activeTab === 1}
        onclick={() => (activeTab = 1)}
      >
        {t('menu.settings')}
      </button>
      <button
        class="tab tab-lg"
        class:tab-active={activeTab === 2}
        onclick={() => (activeTab = 2)}
      >
        {t('menu.about')}
      </button>
    </div>
    <button
      class="p-2 rounded-lg opacity-70 hover:opacity-100 transition-opacity"
      onclick={onThemeToggle}
      title={t('settings.theme')}
    >
      {#if theme === 'dark'}
        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
        </svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        </svg>
      {/if}
    </button>
  </div>

  <div class="flex-1 overflow-y-scroll p-4">
    {#if activeTab === 0}
      <HomeTab {lang} lgcRunning={gLgcRunning} mostRunning={gMostRunning} {onPathsChange} {onRefreshRunning} />
    {:else if activeTab === 1}
      <SettingsTab {lang} {onLangChange} />
    {:else if activeTab === 2}
      <AboutTab {lang} />
    {/if}
  </div>
</div>
