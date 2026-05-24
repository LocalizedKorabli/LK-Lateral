<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { listen } from "@tauri-apps/api/event";
  import { getTranslation, type Language, APP_LANG_TO_LGC_ID, APP_LANG_TO_MOST_ID } from "$lib/i18n";

  let { lang = 'zh_CN', lgcRunning = false, mostRunning = false, onPathsChange, onRefreshRunning }: { lang?: Language; lgcRunning?: boolean; mostRunning?: boolean; onPathsChange?: (lgcPath: string, mostPath: string) => void; onRefreshRunning?: () => Promise<void> } = $props();

  interface ProgressPayload {
    step: string;
    percent: number;
    message: string;
    message_key: string;
    message_params: Record<string, string>;
    instance: string;
    downloaded_bytes: number;
    total_bytes: number;
  }

  interface ProgressState {
    visible: boolean;
    percent: number;
    message: string;
    downloadedBytes: number;
    totalBytes: number;
  }

  interface BaseStatus {
    path: string;
    version: string;
    loc_installed: boolean;
    loc_version: string;
    loc_language: string;
  }

  type LgcStatus = BaseStatus;

  interface MostStatus {
    path: string;
    version: string;
    loc_installed: boolean;
    loc_app_version: string;
    loc_mods_version: string;
    loc_language: string;
  }

  interface LgcLanguage {
    id: string;
    name: string;
  }

  interface LgcMetadata {
    path: string;
    version: string;
    supported_lgc_version: string;
    supported_languages: LgcLanguage[];
  }

  interface L10nPackage {
    path: string;
    version: string;
    supported_most_version?: string;
  }

  interface MostMetadataItem {
    id: string;
    name: string;
    l10n_app: L10nPackage;
    l10n_mods: L10nPackage;
  }

  interface AppConfig {
    language: string;
    proxy: unknown;
    lgc_paths: string[];
    most_paths: string[];
    most_lang_id?: string;
    lgc_lang_id?: string;
  }

  let lgcPaths: string[] = $state([]);
  let mostPaths: string[] = $state([]);
  let lgcPath: string = $state('');
  let mostPath: string = $state('');

  let lgcStatus: LgcStatus | null = $state(null);
  let mostStatus: MostStatus | null = $state(null);

  let lgcMetadata: LgcMetadata | null = $state(null);
  let mostMetadata: MostMetadataItem[] | null = $state(null);

  let installLoading: { lgc: boolean; most: boolean } = $state({ lgc: false, most: false });
  let lgcProgress: ProgressState = $state({ visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 });
  let mostProgress: ProgressState = $state({ visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 });
  let refreshing: boolean = $state(false);
  let scanning: boolean = $state(false);

  let toastMessage: string | null = $state(null);
  let toastType: 'success' | 'error' | 'warning' | 'info' = $state('info');

  let showLgcSettings: boolean = $state(false);
  let showMostSettings: boolean = $state(false);
  let showLgcUninstall: boolean = $state(false);
  let showMostUninstall: boolean = $state(false);
  let lgcPathInput: string = $state('');
  let mostPathInput: string = $state('');
  let lgcLangId: string = $state('');
  let mostLangId: string = $state('');

  type PendingAction = {
    instance: 'lgc' | 'most';
    run: () => Promise<void>;
    label: string;
  } | null;
  let pendingAction: PendingAction = $state(null);
  let waitingForClose: boolean = $state(false);
  let waitingPollTimer: ReturnType<typeof setInterval> | null = null;
  let showKillConfirm: boolean = $state(false);
  let killingInProgress: boolean = $state(false);

  function t(key: string, params?: Record<string, string>): string { return getTranslation(key, lang, params); }

  function showToast(message: string, type: 'success' | 'error' | 'warning' | 'info' = 'info') {
    toastMessage = message;
    toastType = type;
    setTimeout(() => { toastMessage = null; }, 4000);
  }

  function startWaitPolling() {
    if (waitingPollTimer) clearInterval(waitingPollTimer);
    waitingForClose = true;
    waitingPollTimer = setInterval(async () => {
      if (!pendingAction) {
        stopWaitPolling();
        return;
      }
      const path = pendingAction.instance === 'lgc' ? lgcPath : mostPath;
      const running = await invoke<boolean>('is_app_running', { installPath: path });
      if (!running) {
        stopWaitPolling();
        await onRefreshRunning?.();
        const action = pendingAction.run;
        pendingAction = null;
        await action();
      }
    }, 1500);
  }

  function stopWaitPolling() {
    waitingForClose = false;
    if (waitingPollTimer) {
      clearInterval(waitingPollTimer);
      waitingPollTimer = null;
    }
  }

  function cancelPendingAction() {
    stopWaitPolling();
    showKillConfirm = false;
    pendingAction = null;
  }

  function openKillConfirm() {
    showKillConfirm = true;
  }

  function closeKillConfirm() {
    showKillConfirm = false;
  }

  async function forceKillAndProceed() {
    if (!pendingAction) return;
    killingInProgress = true;
    const path = pendingAction.instance === 'lgc' ? lgcPath : mostPath;
    try {
      await invoke('force_kill_app', { installPath: path });
    } catch { /* may fail if already dead */ }
    stopWaitPolling();
    killingInProgress = false;
    showKillConfirm = false;
    await new Promise(r => setTimeout(r, 500));
    await onRefreshRunning?.();
    const action = pendingAction.run;
    pendingAction = null;
    await action();
  }

  async function ensureAppClosed(instance: 'lgc' | 'most', action: () => Promise<void>, label: string) {
    const path = instance === 'lgc' ? lgcPath : mostPath;
    const running = await invoke<boolean>('is_app_running', { installPath: path });
    if (!running) {
      await onRefreshRunning?.();
      await action();
      return;
    }
    pendingAction = { instance, run: action, label };
    startWaitPolling();
  }

  function compareVersions(a: string, b: string): number {
    const partsA = a.split('.').map(Number);
    const partsB = b.split('.').map(Number);
    for (let i = 0; i < Math.max(partsA.length, partsB.length); i++) {
      const na = partsA[i] || 0;
      const nb = partsB[i] || 0;
      if (na > nb) return 1;
      if (na < nb) return -1;
    }
    return 0;
  }

  function addPathUnique(paths: string[], newPath: string): string[] {
    const normalized = newPath.replace(/[\\/]+$/, '');
    const filtered = paths.filter(p => p.replace(/[\\/]+$/, '') !== normalized);
    return [newPath, ...filtered];
  }

  async function savePathsToConfig(lgc: string[], most: string[]) {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      config.lgc_paths = lgc;
      config.most_paths = most;
      await invoke('save_app_config', { appConfig: config });
    } catch {}
  }

  async function selectLgcPath(idx: number) {
    const selected = lgcPaths[idx];
    lgcPaths = addPathUnique(lgcPaths, selected);
    lgcPath = selected;
    await savePathsToConfig(lgcPaths, mostPaths);
    await checkLgcStatus();
  }

  async function selectMostPath(idx: number) {
    const selected = mostPaths[idx];
    mostPaths = addPathUnique(mostPaths, selected);
    mostPath = selected;
    await savePathsToConfig(lgcPaths, mostPaths);
    await checkMostStatus();
  }

  async function checkLgcStatus() {
    if (!lgcPath) { lgcStatus = null; return; }
    try { lgcStatus = await invoke<LgcStatus>('check_lgc_status', { lgcPath }); } catch { lgcStatus = null; }
  }

  async function checkMostStatus() {
    if (!mostPath) { mostStatus = null; return; }
    try { mostStatus = await invoke<MostStatus>('check_most_status', { mostPath }); } catch { mostStatus = null; }
  }

  async function fetchMetadata() {
    const cachedLgc = await invoke<LgcMetadata | null>('read_cached_lgc_metadata');
    if (cachedLgc) lgcMetadata = cachedLgc;
    const cachedMost = await invoke<MostMetadataItem[] | null>('read_cached_most_metadata');
    if (cachedMost) mostMetadata = cachedMost;
    try { lgcMetadata = await invoke<LgcMetadata>('fetch_lgc_metadata'); } catch {}
    try { mostMetadata = await invoke<MostMetadataItem[]>('fetch_most_metadata'); } catch {}
  }

  async function refreshAll() {
    refreshing = true;
    try {
      await Promise.all([checkLgcStatus(), checkMostStatus(), fetchMetadata(), onRefreshRunning?.()]);
      showToast(t('dialog.refresh_complete'), 'success');
    } catch {
      showToast(t('error.scan_failed'), 'error');
    }
    refreshing = false;
  }

  async function autoScan() {
    scanning = true;
    let changedLgc = lgcPaths;
    let changedMost = mostPaths;
    let foundNew = false;
    try {
      const foundLgc = await invoke<string | null>('scan_lgc');
      if (foundLgc) {
        changedLgc = addPathUnique(changedLgc, foundLgc);
        if (!lgcPath) {
          lgcPaths = changedLgc;
          lgcPath = foundLgc;
          await checkLgcStatus();
          foundNew = true;
        } else if (!lgcPaths.some(p => p.replace(/[\\/]+$/, '') === foundLgc.replace(/[\\/]+$/, ''))) {
          foundNew = true;
        }
      }
    } catch { showToast(t('error.scan_failed'), 'error'); }
    try {
      const foundMost = await invoke<string | null>('scan_most');
      if (foundMost) {
        changedMost = addPathUnique(changedMost, foundMost);
        if (!mostPath) {
          mostPaths = changedMost;
          mostPath = foundMost;
          await checkMostStatus();
          foundNew = true;
        } else if (!mostPaths.some(p => p.replace(/[\\/]+$/, '') === foundMost.replace(/[\\/]+$/, ''))) {
          foundNew = true;
        }
      }
    } catch { showToast(t('error.scan_failed'), 'error'); }
    lgcPaths = changedLgc;
    mostPaths = changedMost;
    await savePathsToConfig(lgcPaths, mostPaths);
    showToast(t('dialog.scan_complete'), foundNew ? 'success' : 'info');
    scanning = false;
  }

  async function loadAndSelect() {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      if (config.lgc_paths?.length) {
        lgcPaths = config.lgc_paths;
        lgcPath = lgcPaths[0];
      }
      if (config.most_paths?.length) {
        mostPaths = config.most_paths;
        mostPath = mostPaths[0];
      }
      if (config.lgc_lang_id) {
        lgcLangId = config.lgc_lang_id;
      }
      if (config.most_lang_id) {
        mostLangId = config.most_lang_id;
      }
    } catch {}
    if (lgcPath) await checkLgcStatus();
    if (mostPath) await checkMostStatus();
    await fetchMetadata();
    autoSelectLgcLang();
    autoSelectMostLang();
    await onRefreshRunning?.();
    if (!lgcPath || !mostPath) await autoScan();
  }

  function autoSelectLgcLang() {
    if (lgcLangId) return;
    if (!lgcMetadata) return;
    const preferred = APP_LANG_TO_LGC_ID[lang];
    if (lgcMetadata.supported_languages.some(l => l.id === preferred)) {
      lgcLangId = preferred;
      saveLgcLangIdToConfig();
    }
  }

  function autoSelectMostLang() {
    if (mostLangId) return;
    if (!mostMetadata) return;
    const preferred = APP_LANG_TO_MOST_ID[lang];
    if (mostMetadata.some(m => m.id === preferred)) {
      mostLangId = preferred;
      saveMostLangIdToConfig();
    }
  }

  async function installLgc() {
    if (!lgcPath || !lgcMetadata || !lgcStatus?.version) return;
    await ensureAppClosed('lgc', doInstallLgc, t('button.install'));
  }

  async function doInstallLgc() {
    if (!lgcPath || !lgcMetadata || !lgcStatus?.version) return;
    const cmp = compareVersions(lgcStatus.version, lgcMetadata.supported_lgc_version);
    if (cmp < 0) { showToast(t('label.update_first'), 'warning'); return; }
    if (cmp > 0) { showToast(t('label.localization_not_ready'), 'warning'); return; }
    installLoading = { ...installLoading, lgc: true };
    try {
      await invoke('download_and_install_lgc', { lgcPath, langId: lgcLangId || null });
      showToast(t('dialog.install_success'), 'success');
      await checkLgcStatus();
    } catch { lgcProgress = { visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 }; showToast(t('dialog.install_failed'), 'error'); }
    installLoading = { ...installLoading, lgc: false };
  }

  async function updateLgc() {
    if (!lgcPath) return;
    await ensureAppClosed('lgc', doUpdateLgc, t('button.update'));
  }

  async function doUpdateLgc() {
    if (!lgcPath) return;
    installLoading = { ...installLoading, lgc: true };
    try {
      await invoke('download_and_install_lgc', { lgcPath, langId: lgcLangId || null });
      showToast(t('dialog.install_success'), 'success');
      await checkLgcStatus();
    } catch { lgcProgress = { visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 }; showToast(t('dialog.install_failed'), 'error'); }
    installLoading = { ...installLoading, lgc: false };
  }

  async function installMost() {
    if (!mostPath || !mostMetadata || !mostLangId || !mostStatus?.version) return;
    await ensureAppClosed('most', doInstallMost, t('button.install'));
  }

  async function doInstallMost() {
    if (!mostPath || !mostMetadata || !mostLangId || !mostStatus?.version) return;
    const langEntry = mostMetadata.find(m => m.id === mostLangId);
    if (!langEntry) { showToast(t('error.fetch_metadata_failed'), 'error'); return; }
    const supportedVer = langEntry.l10n_app.supported_most_version || '';
    if (supportedVer) {
      const cmp = compareVersions(mostStatus.version, supportedVer);
      if (cmp < 0) { showToast(t('label.update_first'), 'warning'); return; }
      if (cmp > 0) { showToast(t('label.localization_not_ready'), 'warning'); return; }
    }
    installLoading = { ...installLoading, most: true };
    try {
      await invoke('download_and_install_most', { mostPath, langId: mostLangId });
      showToast(t('dialog.install_success'), 'success');
      await checkMostStatus();
    } catch { mostProgress = { visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 }; showToast(t('dialog.install_failed'), 'error'); }
    installLoading = { ...installLoading, most: false };
  }

  async function updateMost() {
    if (!mostPath) return;
    await ensureAppClosed('most', doUpdateMost, t('button.update'));
  }

  async function doUpdateMost() {
    if (!mostPath) return;
    installLoading = { ...installLoading, most: true };
    try {
      await invoke('download_and_install_most', { mostPath, langId: mostLangId || mostStatus?.loc_language || '' });
      showToast(t('dialog.install_success'), 'success');
      await checkMostStatus();
    } catch { mostProgress = { visible: false, percent: 0, message: '', downloadedBytes: 0, totalBytes: 0 }; showToast(t('dialog.install_failed'), 'error'); }
    installLoading = { ...installLoading, most: false };
  }

  async function fullUninstallLgc() {
    try {
      await invoke('full_uninstall', { path: lgcPath });
      showLgcUninstall = false;
      showToast(t('dialog.uninstall_success'), 'success');
      await checkLgcStatus();
    } catch { showToast(t('dialog.uninstall_failed'), 'error'); }
  }

  async function fullUninstallMost() {
    try {
      await invoke('full_uninstall', { path: mostPath });
      showMostUninstall = false;
      showToast(t('dialog.uninstall_success'), 'success');
      await checkMostStatus();
    } catch { showToast(t('dialog.uninstall_failed'), 'error'); }
  }

  async function launchLgc() {
    try { await invoke('launch_app', { path: lgcPath + '\\lgc.exe' }); }
    catch { showToast(t('error.launch_failed'), 'error'); }
    await new Promise(r => setTimeout(r, 500));
    await onRefreshRunning?.();
  }

  async function launchMost() {
    try { await invoke('launch_app', { path: mostPath + '\\Korabli.Most.exe' }); }
    catch { showToast(t('error.launch_failed'), 'error'); }
    await new Promise(r => setTimeout(r, 500));
    await onRefreshRunning?.();
  }

  async function browseLgcPath() {
    const selected = await open({ directory: true });
    if (selected && typeof selected === 'string') lgcPathInput = selected;
  }

  async function browseMostPath() {
    const selected = await open({ directory: true });
    if (selected && typeof selected === 'string') mostPathInput = selected;
  }

  function openLgcSettings() { lgcPathInput = lgcPath; showLgcSettings = true; }
  function openMostSettings() { mostPathInput = mostPath; showMostSettings = true; }

  async function saveLgcSettings() {
    if (!lgcPathInput) { showLgcSettings = false; return; }
    const valid = await invoke<boolean>('validate_lgc_path', { path: lgcPathInput });
    if (!valid) { showToast(t('error.invalid_dir'), 'error'); return; }
    lgcPaths = addPathUnique(lgcPaths, lgcPathInput);
    lgcPath = lgcPathInput;
    showLgcSettings = false;
    await saveLgcLangIdToConfig();
    await savePathsToConfig(lgcPaths, mostPaths);
    await checkLgcStatus();
  }

  async function saveMostSettings() {
    if (!mostPathInput) { showMostSettings = false; return; }
    const valid = await invoke<boolean>('validate_most_path', { path: mostPathInput });
    if (!valid) { showToast(t('error.invalid_dir'), 'error'); return; }
    mostPaths = addPathUnique(mostPaths, mostPathInput);
    mostPath = mostPathInput;
    showMostSettings = false;
    await saveMostLangIdToConfig();
    await savePathsToConfig(lgcPaths, mostPaths);
    await checkMostStatus();
  }

  async function saveMostLangIdToConfig() {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      config.most_lang_id = mostLangId;
      await invoke('save_app_config', { appConfig: config });
    } catch {}
  }

  let lgcBadgeClass = $derived.by(() => {
    if (!lgcStatus) return 'badge badge-ghost';
    if (!lgcStatus.loc_installed) return 'badge badge-error';
    return 'badge badge-success';
  });

  let lgcBadgeText = $derived.by(() => {
    if (!lgcStatus || !lgcStatus.loc_installed) return t('status.not_installed');
    return t('status.installed');
  });

  let mostBadgeClass = $derived.by(() => {
    if (!mostStatus) return 'badge badge-ghost';
    if (!mostStatus.loc_installed) return 'badge badge-error';
    return 'badge badge-success';
  });

  let mostBadgeText = $derived.by(() => {
    if (!mostStatus || !mostStatus.loc_installed) return t('status.not_installed');
    return t('status.installed');
  });

  let lgcNeedsUpdate = $derived.by(() => {
    if (!lgcStatus?.loc_installed || !lgcMetadata) return false;
    return compareVersions(lgcMetadata.version, lgcStatus.loc_version) > 0;
  });

  let lgcIsLatest = $derived.by(() => {
    if (!lgcStatus?.loc_installed || !lgcMetadata) return false;
    return compareVersions(lgcMetadata.version, lgcStatus.loc_version) <= 0;
  });

  let lgcAppTooOld = $derived.by(() => {
    if (!lgcPath || !lgcStatus?.version || !lgcMetadata) return false;
    return compareVersions(lgcStatus.version, lgcMetadata.supported_lgc_version) < 0;
  });

  let lgcAppTooNew = $derived.by(() => {
    if (!lgcPath || !lgcStatus?.version || !lgcMetadata) return false;
    return compareVersions(lgcStatus.version, lgcMetadata.supported_lgc_version) > 0;
  });

  let mostAppTooOld = $derived.by(() => {
    if (!mostPath || !mostStatus?.version || !mostMetadata || !mostLangId) return false;
    const langEntry = mostMetadata.find(m => m.id === mostLangId);
    if (!langEntry) return false;
    const sv = langEntry.l10n_app.supported_most_version || '';
    return !!sv && compareVersions(mostStatus.version, sv) < 0;
  });

  let mostAppTooNew = $derived.by(() => {
    if (!mostPath || !mostStatus?.version || !mostMetadata || !mostLangId) return false;
    const langEntry = mostMetadata.find(m => m.id === mostLangId);
    if (!langEntry) return false;
    const sv = langEntry.l10n_app.supported_most_version || '';
    return !!sv && compareVersions(mostStatus.version, sv) > 0;
  });

  let mostNeedsUpdate = $derived.by(() => {
    if (!mostStatus?.loc_installed || !mostMetadata) return false;
    const langEntry = mostMetadata.find(m => m.id === mostStatus.loc_language);
    if (!langEntry) return false;
    return compareVersions(langEntry.l10n_app.version, mostStatus.loc_app_version) > 0
        || compareVersions(langEntry.l10n_mods.version, mostStatus.loc_mods_version) > 0;
  });

  let mostIsLatest = $derived.by(() => {
    if (!mostStatus?.loc_installed || !mostMetadata) return false;
    const langEntry = mostMetadata.find(m => m.id === mostStatus.loc_language);
    if (!langEntry) return true;
    return compareVersions(langEntry.l10n_app.version, mostStatus.loc_app_version) <= 0
        && compareVersions(langEntry.l10n_mods.version, mostStatus.loc_mods_version) <= 0;
  });

  let mostAppNeedsUpdate = $derived.by(() => {
    if (!mostStatus?.loc_installed || !mostMetadata) return false;
    const langEntry = mostMetadata.find(m => m.id === mostStatus.loc_language);
    if (!langEntry) return false;
    return compareVersions(langEntry.l10n_app.version, mostStatus.loc_app_version) > 0;
  });

  let mostModsNeedsUpdate = $derived.by(() => {
    if (!mostStatus?.loc_installed || !mostMetadata) return false;
    const langEntry = mostMetadata.find(m => m.id === mostStatus.loc_language);
    if (!langEntry) return false;
    return compareVersions(langEntry.l10n_mods.version, mostStatus.loc_mods_version) > 0;
  });

  function lgcInstallDisabled() {
    return !lgcPath || !lgcStatus?.version || installLoading.lgc
      || lgcAppTooOld || lgcAppTooNew;
  }

  function resolveMostLangName(langId: string): string {
    return mostMetadata?.find(m => m.id === langId)?.name || langId;
  }

  function mostLangDisplay(): string {
    const selected = mostLangId ? resolveMostLangName(mostLangId) : '';
    const installed = mostStatus?.loc_language ? resolveMostLangName(mostStatus.loc_language) : '';
    if (selected && installed && selected !== installed) {
      return `${selected} (${t('label.installed')}: ${installed})`;
    }
    return selected || installed || '';
  }

  function resolveLgcLangName(langId: string): string {
    return lgcMetadata?.supported_languages?.find(l => l.id.toLowerCase() === langId.toLowerCase())?.name || langId;
  }

  function lgcLangDisplay(): string {
    const selectedName = lgcLangId ? resolveLgcLangName(lgcLangId) : '';
    const installedName = lgcStatus?.loc_language ? resolveLgcLangName(lgcStatus.loc_language) : '';
    const sameLang = lgcLangId && lgcStatus?.loc_language
      && lgcLangId.toLowerCase() === lgcStatus.loc_language.toLowerCase();
    if (selectedName && installedName && !sameLang) {
      return `${selectedName} (${t('label.current_language')}: ${installedName})`;
    }
    return selectedName || installedName || '';
  }

  async function saveLgcLangIdToConfig() {
    try {
      const config = await invoke<AppConfig>('get_app_config');
      config.lgc_lang_id = lgcLangId;
      await invoke('save_app_config', { appConfig: config });
    } catch {}
  }

  async function setLgcLanguage() {
    if (!lgcPath || !lgcLangId) return;
    await ensureAppClosed('lgc', doSetLgcLanguage, t('button.apply_language'));
  }

  async function doSetLgcLanguage() {
    if (!lgcPath || !lgcLangId) return;
    try {
      await invoke('set_lgc_language', { lgcPath, langId: lgcLangId });
      await checkLgcStatus();
      showToast(t('dialog.language_applied'), 'success');
    } catch { showToast(t('error.scan_failed'), 'error'); }
  }

  function mostInstallDisabled() {
    if (!mostPath || !mostStatus?.version || installLoading.most || !mostLangId) return true;
    return mostAppTooOld || mostAppTooNew;
  }

  let initialized = false;
  $effect(() => { if (!initialized) { initialized = true; loadAndSelect(); } });

  $effect(() => {
    onPathsChange?.(lgcPath, mostPath);
  });

  $effect(() => {
    let unlistenFn: (() => void) | null = null;
    listen<ProgressPayload>('install-progress', (event) => {
      const p = event.payload;
      const displayMsg = p.message_key ? t(p.message_key, p.message_params) : p.message;
      if (p.instance === 'most') {
        mostProgress.visible = true;
        mostProgress.percent = p.percent;
        mostProgress.message = displayMsg;
        mostProgress.downloadedBytes = p.downloaded_bytes;
        mostProgress.totalBytes = p.total_bytes;
      } else {
        lgcProgress.visible = true;
        lgcProgress.percent = p.percent;
        lgcProgress.message = displayMsg;
        lgcProgress.downloadedBytes = p.downloaded_bytes;
        lgcProgress.totalBytes = p.total_bytes;
      }
      if (p.percent >= 100) {
        setTimeout(() => {
          if (p.instance === 'most') {
            mostProgress.visible = false;
            mostProgress.percent = 0;
            mostProgress.message = '';
          } else {
            lgcProgress.visible = false;
            lgcProgress.percent = 0;
            lgcProgress.message = '';
          }
        }, 1500);
      }
    }).then(fn => { unlistenFn = fn; });
    return () => { unlistenFn?.(); };
  });
</script>

<div class="flex flex-col gap-4 h-full">
  <div class="flex items-center justify-between">
    <h2 class="text-xl font-bold">{t('label.app')}</h2>
    <div class="flex gap-2">
      <button class="btn btn-sm btn-outline" onclick={refreshAll} disabled={refreshing}>
        {#if refreshing}<span class="loading loading-spinner loading-xs"></span>{/if}
        {t('button.refresh')}
      </button>
      <button class="btn btn-sm btn-primary" onclick={autoScan} disabled={scanning}>
        {#if scanning}<span class="loading loading-spinner loading-xs"></span>{/if}
        {scanning ? t('label.scanning') : t('button.auto_scan')}
      </button>
    </div>
  </div>

  <div class="card card-bordered bg-base-100 shadow-sm">
    <div class="card-body p-5">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
          <h3 class="card-title text-lg m-0">{t('label.lgc')}</h3>
          {#if lgcRunning}<span class="badge badge-warning badge-sm">{t('status.running')}</span>{/if}
          <span class={lgcBadgeClass}>{lgcBadgeText}</span>
        </div>
        {#if lgcPaths.length > 1}
          <select class="select select-sm select-bordered max-w-[60%]" value={lgcPath} onchange={(e: Event) => selectLgcPath(lgcPaths.indexOf((e.target as HTMLSelectElement).value))}>
            {#each lgcPaths as p}
              <option value={p}>{p}</option>
            {/each}
          </select>
        {/if}
      </div>
      <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
        <span class="text-base-content/60">{t('label.path')}:</span>
        <span class="truncate">{lgcPath || '-'}</span>
        <span class="text-base-content/60">{t('label.version')}:</span>
        <span>
          {lgcStatus?.version || '-'}
          {#if lgcAppTooOld}
            <span class="badge badge-warning badge-xs ml-1.5">{t('status.needs_update')}</span>
          {:else if lgcAppTooNew}
            <span class="badge badge-warning badge-xs ml-1.5">{t('status.awaiting_adaptation')}</span>
          {/if}
        </span>
        <span class="text-base-content/60">{t('label.localization')}:</span>
        <span>
          {#if lgcStatus?.loc_installed}
            {lgcStatus.loc_version || '?'}
            {#if lgcNeedsUpdate}
              <span class="badge badge-info badge-xs ml-1.5">{t('label.update_available')}</span>
            {/if}
          {:else}
            {t('status.not_installed')}
          {/if}
        </span>
        <span class="text-base-content/60">{t('label.lgc_language')}:</span>
        {#if lgcMetadata?.supported_languages}
          {#if lgcLangId || lgcStatus?.loc_language}
            <span>{lgcLangDisplay()}</span>
          {:else}
            <span class="text-warning text-xs font-medium">{t('label.select_language_hint')}</span>
          {/if}
        {:else}
          <span class="loading loading-spinner loading-xs"></span>
        {/if}
      </div>
      {#if lgcProgress.visible}
        <div class="mt-2">
          <div class="flex items-center justify-between mb-1">
            <span class="text-xs font-medium">{lgcProgress.message}</span>
            <span class="text-xs text-base-content/60">{lgcProgress.percent}%</span>
          </div>
          <progress class="progress progress-primary w-full" value={lgcProgress.percent} max="100"></progress>
        </div>
      {/if}
      <div class="card-actions justify-end mt-3">
        {#if lgcStatus?.loc_installed && lgcLangId && lgcLangId.toLowerCase() !== lgcStatus.loc_language.toLowerCase()}
          <button class="btn btn-sm btn-outline" onclick={setLgcLanguage}>{t('button.apply_language')}</button>
        {/if}
        {#if lgcStatus?.loc_installed}
          <div class="{!lgcNeedsUpdate ? 'tooltip tooltip-left' : ''}" data-tip={!lgcNeedsUpdate ? t('status.up_to_date') : ''}>
            <button class="btn btn-sm btn-primary" onclick={updateLgc} disabled={!lgcNeedsUpdate || installLoading.lgc}>
              {#if installLoading.lgc}<span class="loading loading-spinner loading-xs"></span>{/if}
              {t('button.update')}
            </button>
          </div>
        {:else}
          <div class="{lgcAppTooOld || lgcAppTooNew ? 'tooltip tooltip-left' : ''}" data-tip={lgcAppTooOld ? t('label.app_version_unsupported') : (lgcAppTooNew ? t('label.localization_not_ready') : '')}>
            <button class="btn btn-sm btn-primary" onclick={installLgc} disabled={lgcInstallDisabled()}>
              {#if installLoading.lgc}<span class="loading loading-spinner loading-xs"></span>{/if}
              {t('button.install')}
            </button>
          </div>
        {/if}
        <button class="btn btn-sm btn-outline btn-error" onclick={() => (showLgcUninstall = true)} disabled={!lgcStatus?.loc_installed}>
          {t('button.uninstall')}
        </button>
        <button class="btn btn-sm btn-outline" onclick={launchLgc} disabled={!lgcPath}>
          {t('button.launch')}
        </button>
        <button class="btn btn-sm btn-outline" onclick={openLgcSettings}>
          {t('button.settings')}
        </button>
      </div>
    </div>
  </div>

  <div class="card card-bordered bg-base-100 shadow-sm">
    <div class="card-body p-5">
      <div class="flex items-center justify-between mb-3">
        <div class="flex items-center gap-2">
          <h3 class="card-title text-lg m-0">{t('label.most')}</h3>
          {#if mostRunning}<span class="badge badge-warning badge-sm">{t('status.running')}</span>{/if}
          <span class={mostBadgeClass}>{mostBadgeText}</span>
        </div>
        {#if mostPaths.length > 1}
          <select class="select select-sm select-bordered max-w-[60%]" value={mostPath} onchange={(e: Event) => selectMostPath(mostPaths.indexOf((e.target as HTMLSelectElement).value))}>
            {#each mostPaths as p}
              <option value={p}>{p}</option>
            {/each}
          </select>
        {/if}
      </div>
      <div class="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
        <span class="text-base-content/60">{t('label.path')}:</span>
        <span class="truncate">{mostPath || '-'}</span>
        <span class="text-base-content/60">{t('label.version')}:</span>
        <span>
          {mostStatus?.version || '-'}
          {#if mostAppTooOld}
            <span class="badge badge-warning badge-xs ml-1.5">{t('status.needs_update')}</span>
          {:else if mostAppTooNew}
            <span class="badge badge-warning badge-xs ml-1.5">{t('status.awaiting_adaptation')}</span>
          {/if}
        </span>
        <span class="text-base-content/60">{t('label.localization_app')}:</span>
        <span>
          {#if mostStatus?.loc_installed}
            {mostStatus.loc_app_version || '?'}
            {#if mostAppNeedsUpdate}
              <span class="badge badge-info badge-xs ml-1.5">{t('label.update_available')}</span>
            {/if}
          {:else}
            {t('status.not_installed')}
          {/if}
        </span>
        <span class="text-base-content/60">{t('label.localization_mods')}:</span>
        <span>
          {#if mostStatus?.loc_installed}
            {mostStatus.loc_mods_version || '?'}
            {#if mostModsNeedsUpdate}
              <span class="badge badge-info badge-xs ml-1.5">{t('label.update_available')}</span>
            {/if}
          {:else}
            {t('status.not_installed')}
          {/if}
        </span>
        <span class="text-base-content/60">{t('label.language')}:</span>
        {#if mostMetadata}
          {#if mostLangId || mostStatus?.loc_language}
            <span>{mostLangDisplay()}</span>
          {:else}
            <span class="text-warning text-xs font-medium">{t('label.select_language_hint')}</span>
          {/if}
        {:else}
          <span class="loading loading-spinner loading-xs"></span>
        {/if}
      </div>
      {#if mostProgress.visible}
        <div class="mt-2">
          <div class="flex items-center justify-between mb-1">
            <span class="text-xs font-medium">{mostProgress.message}</span>
            <span class="text-xs text-base-content/60">{mostProgress.percent}%</span>
          </div>
          <progress class="progress progress-primary w-full" value={mostProgress.percent} max="100"></progress>
        </div>
      {/if}
      <div class="card-actions justify-end mt-3">
        {#if mostStatus?.loc_installed}
          <div class="{!mostNeedsUpdate ? 'tooltip tooltip-left' : ''}" data-tip={!mostNeedsUpdate && !mostLangId ? t('label.select_language_first') : (!mostNeedsUpdate ? t('status.up_to_date') : '')}>
            <button class="btn btn-sm btn-primary" onclick={updateMost} disabled={!mostNeedsUpdate || installLoading.most}>
              {#if installLoading.most}<span class="loading loading-spinner loading-xs"></span>{/if}
              {t('button.update')}
            </button>
          </div>
        {:else}
          <div class="{mostInstallDisabled() && !mostLangId || mostAppTooOld || mostAppTooNew ? 'tooltip tooltip-left' : ''}" data-tip={mostAppTooOld ? t('label.app_version_unsupported') : (mostAppTooNew ? t('label.localization_not_ready') : (mostInstallDisabled() && !mostLangId ? t('label.select_language_first') : ''))}>
            <button class="btn btn-sm btn-primary" onclick={installMost} disabled={mostInstallDisabled()}>
              {#if installLoading.most}<span class="loading loading-spinner loading-xs"></span>{/if}
              {t('button.install')}
            </button>
          </div>
        {/if}
        <button class="btn btn-sm btn-outline btn-error" onclick={() => (showMostUninstall = true)} disabled={!mostStatus?.loc_installed}>
          {t('button.uninstall')}
        </button>
        <button class="btn btn-sm btn-outline" onclick={launchMost} disabled={!mostPath}>
          {t('button.launch')}
        </button>
        <button class="btn btn-sm btn-outline" onclick={openMostSettings}>
          {t('button.settings')}
        </button>
      </div>
    </div>
  </div>
</div>

{#if showLgcSettings}
  <div class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-4">LGC {t('button.settings')}</h3>
      <div class="form-control mb-4">
        <div class="label"><span class="label-text">{t('label.lgc_path')}</span></div>
        <div class="join w-full">
          <input type="text" class="input input-bordered join-item flex-1" bind:value={lgcPathInput} placeholder="C:\Games\Lesta Game Center" />
          <button class="btn btn-outline join-item" onclick={browseLgcPath}>{t('button.browse')}</button>
        </div>
      </div>
      <div class="form-control mb-4">
        <div class="label"><span class="label-text">{t('label.lgc_language')}</span></div>
        <select class="select select-bordered w-full" bind:value={lgcLangId} onchange={saveLgcLangIdToConfig}>
          <option value="">{t('label.select_language')}</option>
          {#each lgcMetadata?.supported_languages || [] as langItem}
            <option value={langItem.id}>{langItem.name}</option>
          {/each}
        </select>
      </div>
      <div class="modal-action">
        <button class="btn btn-outline" onclick={() => (showLgcSettings = false)}>{t('button.cancel')}</button>
        <button class="btn btn-primary" onclick={saveLgcSettings}>{t('button.save')}</button>
      </div>
    </div>
  </div>
{/if}

{#if showMostSettings}
  <div class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-4">Most {t('button.settings')}</h3>
      <div class="form-control mb-4">
        <div class="label"><span class="label-text">{t('label.path')}</span></div>
        <div class="join w-full">
          <input type="text" class="input input-bordered join-item flex-1" bind:value={mostPathInput} placeholder="C:\Games\Mir Korabley" />
          <button class="btn btn-outline join-item" onclick={browseMostPath}>{t('button.browse')}</button>
        </div>
      </div>
      <div class="form-control mb-4">
        <div class="label"><span class="label-text">{t('label.most_language')}</span></div>
        <select class="select select-bordered w-full" bind:value={mostLangId} onchange={saveMostLangIdToConfig}>
          <option value="">{t('label.select_language')}</option>
          {#each mostMetadata || [] as item}
            <option value={item.id}>{item.name}</option>
          {/each}
        </select>
      </div>
      <div class="modal-action">
        <button class="btn btn-outline" onclick={() => (showMostSettings = false)}>{t('button.cancel')}</button>
        <button class="btn btn-primary" onclick={saveMostSettings} disabled={!mostPathInput}>{t('button.save')}</button>
      </div>
    </div>
  </div>
{/if}

{#if showLgcUninstall}
  <div class="modal modal-open" style="z-index: 1000;">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('dialog.uninstall_confirm_title')}</h3>
      <p class="mb-4 text-sm whitespace-pre-line">{t('dialog.uninstall_confirm_msg')}</p>
      <div class="modal-action">
        <button class="btn btn-outline btn-sm" onclick={() => (showLgcUninstall = false)}>{t('button.cancel')}</button>
        <button class="btn btn-error btn-sm" onclick={() => { showLgcUninstall = false; ensureAppClosed('lgc', fullUninstallLgc, t('button.uninstall')); }}>{t('button.uninstall')}</button>
      </div>
    </div>
  </div>
{/if}

{#if showMostUninstall}
  <div class="modal modal-open" style="z-index: 1000;">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('dialog.uninstall_confirm_title')}</h3>
      <p class="mb-4 text-sm whitespace-pre-line">{t('dialog.uninstall_confirm_msg')}</p>
      <div class="modal-action">
        <button class="btn btn-outline btn-sm" onclick={() => (showMostUninstall = false)}>{t('button.cancel')}</button>
        <button class="btn btn-error btn-sm" onclick={() => { showMostUninstall = false; ensureAppClosed('most', fullUninstallMost, t('button.uninstall')); }}>{t('button.uninstall')}</button>
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
      {:else if toastType === 'warning'}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" /></svg>
      {:else}
        <svg xmlns="http://www.w3.org/2000/svg" class="stroke-current shrink-0 h-6 w-6" fill="none" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" /></svg>
      {/if}
      <span class="text-sm font-medium">{toastMessage}</span>
    </div>
  </div>
{/if}

{#if waitingForClose && pendingAction}
  <div class="modal modal-open">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('dialog.app_running_title')}</h3>
      <p class="mb-4 text-sm">
        {t('dialog.app_running_msg', { app: t(`label.${pendingAction.instance}`), action: pendingAction.label })}
      </p>
      <div class="flex items-center gap-2 mb-4 text-sm text-base-content/60">
        <span class="loading loading-spinner loading-sm"></span>
        <span>{t('dialog.waiting_for_close', { app: t(`label.${pendingAction.instance}`) })}</span>
      </div>
      <div class="modal-action">
        <button class="btn btn-outline btn-sm" onclick={cancelPendingAction}>{t('button.cancel')}</button>
        <button class="btn btn-error btn-sm" onclick={openKillConfirm}>{t('button.force_kill')}</button>
      </div>
    </div>
  </div>
{/if}

{#if showKillConfirm && pendingAction}
  <div class="modal modal-open" style="z-index: 1000;">
    <div class="modal-box">
      <h3 class="text-lg font-bold mb-2">{t('dialog.force_kill_confirm_title')}</h3>
      <p class="mb-4 text-sm">
        {t('dialog.force_kill_confirm_msg', { app: t(`label.${pendingAction.instance}`) })}
      </p>
      <div class="modal-action">
        <button class="btn btn-outline btn-sm" onclick={closeKillConfirm} disabled={killingInProgress}>{t('button.cancel')}</button>
        <button class="btn btn-error btn-sm" onclick={forceKillAndProceed} disabled={killingInProgress}>
          {#if killingInProgress}
            <span class="loading loading-spinner loading-xs"></span>
          {/if}
          {killingInProgress ? t('dialog.force_kill_progress', { app: t(`label.${pendingAction.instance}`) }) : t('button.confirm')}
        </button>
      </div>
    </div>
  </div>
{/if}
