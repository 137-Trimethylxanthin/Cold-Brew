<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type {
		AudioOutputDevice,
		JellyfinAccount,
		LastFmScrobbleStatus,
		LibraryStats,
		PlaybackSettings,
		PlaybackStatus,
		ProviderAccount,
		ProviderCapability,
		ProviderCredentialState,
		ProviderLoginStart,
		ProviderLoginState,
		ProviderStatus,
		ScanSummary
	} from '$lib/types';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as DialogComponents from '$lib/components/ui/dialog';
	import * as Select from '$lib/components/ui/select';
	import { Select as SelectPrimitive } from 'bits-ui';
	import * as Card from '$lib/components/ui/card';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import { Badge } from '$lib/components/ui/badge';
	import { Slider } from '$lib/components/ui/slider';
	import ProviderLoginPanel from '$lib/components/ProviderLoginPanel.svelte';
	import { t, setLocale, type Locale, localeName, initLocale } from '$lib/i18n';

	let account: JellyfinAccount | null = null;
	let audioOutputs: AudioOutputDevice[] = [];
	let serviceCapabilities: ProviderCapability[] = [];
	let providerAccounts: ProviderAccount[] = [];
	let providerLoginStates: ProviderLoginState[] = [];
	let providerStatuses: ProviderStatus[] = [];
	let lastFmScrobbleStatus: LastFmScrobbleStatus | null = null;
	let baseUrl = '';
	let userName = '';
	let password = '';
	let selectedAudioOutput = 'default';
	let replayGainMode = 'off';
	let crossfadeEnabled = false;
	let crossfadeDuration = 3000;
	let monoDownmix = false;
	let preampGainDb = 0;
	let playbackSpeed = 1.0;
	let playbackSpeedValue = '1.0';
	let notificationsEnabled = true;
	let selectedProviderId = 'spotify';
	let providerDisplayName = '';
	let providerClientId = '';
	let providerClientSecret = '';
	let providerApiKey = '';
	let providerApiSecret = '';
	let providerAccessToken = '';
	let providerRefreshToken = '';
	let spotifyRedirectUri = 'http://127.0.0.1:9090/callback';
	let spotifyScope = 'playlist-read-private playlist-read-collaborative user-read-private user-read-email user-read-playback-state user-modify-playback-state streaming';
	let spotifyAuthorizationCode = '';
	let spotifyAuthorizationState = '';
	let spotifyAuthorizationUrl = '';
	let tidalRedirectUri = 'http://127.0.0.1:9090/callback';
	let tidalScope = 'search.read playlists.read user.read';
	let tidalAuthorizationCode = '';
	let tidalAuthorizationState = '';
	let tidalAuthorizationUrl = '';
	let youtubeRedirectUri = 'http://127.0.0.1:9090/callback';
	let youtubeScope = 'https://www.googleapis.com/auth/youtube.readonly';
	let youtubeAuthorizationCode = '';
	let youtubeAuthorizationState = '';
	let youtubeAuthorizationUrl = '';
	let lastFmAuthorizationUrl = '';
	let libraryPath = '';
	let loadingLibrary = false;
	let scanSummary: ScanSummary | null = null;
	let message = '';
	let error = '';

	// M20: Accent color
	const ACCENT_STORAGE_KEY = 'coldbrew.accentColor';
	const accentColors = [
		{ id: 'cold-blue', label: 'Cold Blue', cssValue: 'oklch(70% 0.13 205)' },
		{ id: 'caramel', label: 'Caramel', cssValue: 'oklch(68% 0.12 68)' },
		{ id: 'rose', label: 'Rose', cssValue: 'oklch(65% 0.17 15)' },
		{ id: 'mint', label: 'Mint', cssValue: 'oklch(70% 0.13 160)' },
		{ id: 'lavender', label: 'Lavender', cssValue: 'oklch(70% 0.11 290)' },
		{ id: 'amber', label: 'Amber', cssValue: 'oklch(75% 0.14 82)' }
	];
	let selectedAccent = $state('cold-blue');

	function selectAccentColor(id: string) {
		selectedAccent = id;
		const color = accentColors.find((c) => c.id === id);
		if (color) {
			document.documentElement.style.setProperty('--color-brand', color.cssValue);
			if (typeof localStorage !== 'undefined') localStorage.setItem(ACCENT_STORAGE_KEY, id);
		}
	}

	// M20: Layout density
	const DENSITY_STORAGE_KEY = 'coldbrew.density';
	const densityOptions = [
		{ id: 'compact', label: 'Compact' },
		{ id: 'comfortable', label: 'Comfortable' },
		{ id: 'spacious', label: 'Spacious' }
	];
	let density = $state('comfortable');

	function selectDensity(id: string) {
		density = id;
		document.body.setAttribute('data-density', id);
		if (typeof localStorage !== 'undefined') localStorage.setItem(DENSITY_STORAGE_KEY, id);
	}

	function restoreAccentAndDensity() {
		if (typeof localStorage !== 'undefined') {
			const saved = localStorage.getItem(ACCENT_STORAGE_KEY);
			if (saved && accentColors.some((c) => c.id === saved)) {
				selectedAccent = saved;
				const color = accentColors.find((c) => c.id === saved);
				if (color) document.documentElement.style.setProperty('--color-brand', color.cssValue);
			}
			const savedDensity = localStorage.getItem(DENSITY_STORAGE_KEY);
			if (savedDensity && densityOptions.some((d) => d.id === savedDensity)) {
				density = savedDensity;
				document.body.setAttribute('data-density', savedDensity);
			}
		}
	}

	// M22: i18n & accessibility
	let currentLocale = $state<Locale>('en');
	let highContrast = $state(false);

	function toggleHighContrast() {
		highContrast = !highContrast;
		if (highContrast) {
			document.body.setAttribute('data-high-contrast', 'true');
		} else {
			document.body.removeAttribute('data-high-contrast');
		}
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem('coldbrew.highContrast', String(highContrast));
		}
	}

	function changeLocale(locale: Locale) {
		currentLocale = locale;
		setLocale(locale);
	}

	function restoreLocaleAndContrast() {
		currentLocale = initLocale();
		if (typeof localStorage !== 'undefined') {
			highContrast = localStorage.getItem('coldbrew.highContrast') === 'true';
		}
	}

	// Watch folders
	let watchFolders = $state(false);
	let watchingLabel = $state('');

	// About dialog
	let aboutOpen = $state(false);

	// Dev tab state
	let devProviderStates: Record<string, ProviderCredentialState> = {};
	let devFieldValues: Record<string, Record<string, string>> = {};
	let devUseDefault: Record<string, boolean> = {};

	onMount(() => {
	void loadAccount(); void loadAudioOutputs(); void loadPlaybackSettings();
	void loadNotificationSetting();
	void loadServiceCapabilities(); void loadProviderAccounts();
	void loadProviderLoginStates(); void loadLastFmScrobbleStatus();
	void loadProviderStatuses();
	void loadAllDevStates();
	restoreAccentAndDensity();
	restoreLocaleAndContrast();
	});

	function providerBaseFields(providerId: string): { key: string; label: string }[] {
		const byId: Record<string, { key: string; label: string }[]> = {
			spotify: [{ key: 'client_id', label: 'Client ID' }, { key: 'client_secret', label: 'Client Secret' }, { key: 'redirect_uri', label: 'Redirect URI' }],
			tidal: [{ key: 'client_id', label: 'Client ID' }, { key: 'client_secret', label: 'Client Secret' }],
			qobuz: [{ key: 'app_id', label: 'App ID' }, { key: 'app_secret', label: 'App Secret' }],
			youtube: [{ key: 'api_key', label: 'API Key' }],
			lastfm: [{ key: 'api_key', label: 'API Key' }, { key: 'api_secret', label: 'API Secret' }],
			soundcloud: [{ key: 'api_key', label: 'API Key (Client ID)' }],
		};
		return byId[providerId] ?? [];
	}

	async function loadDevProviderState(providerId: string) {
		try {
			const state = await invoke<ProviderCredentialState>('get_provider_credentials', { provider: providerId });
			devProviderStates[providerId] = state;
			devUseDefault[providerId] = state.is_default;
			if (!devFieldValues[providerId]) devFieldValues[providerId] = {};
		} catch (err) {
			console.error('Failed to load dev state for', providerId, err);
		}
	}

	async function saveDevCredentials(providerId: string) {
		error = ''; message = '';
		const fields = providerBaseFields(providerId);
		try {
			for (const f of fields) {
				const val = devFieldValues[providerId]?.[f.key]?.trim();
				if (val) {
					await invoke('set_provider_credentials', { provider: providerId, key: f.key, value: val });
				}
			}
			devUseDefault[providerId] = false;
			await loadDevProviderState(providerId);
			await loadProviderStatuses();
			message = `${providerName(providerId)} custom credentials saved to keyring.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function resetDevToDefaults(providerId: string) {
		error = ''; message = '';
		try {
			await invoke('reset_provider_credentials', { provider: providerId });
			devUseDefault[providerId] = true;
			const fields = providerBaseFields(providerId);
			for (const f of fields) {
				if (devFieldValues[providerId]) devFieldValues[providerId][f.key] = '';
			}
			await loadDevProviderState(providerId);
			await loadProviderStatuses();
			message = `${providerName(providerId)} reset to defaults.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function loadProviderStatuses() {
		error = '';
		try {
			providerStatuses = await invoke<ProviderStatus[]>('get_all_provider_statuses');
		} catch (err) { error = toErrorMessage(err); }
	}

	async function scanLibrary() {
		if (!libraryPath.trim()) { error = 'Enter a local music folder path.'; return; }
		loadingLibrary = true; error = ''; message = '';
		try {
			scanSummary = await invoke<ScanSummary>('scan_library_path', { path: libraryPath });
			message = `Indexed ${scanSummary.indexed_tracks} of ${scanSummary.scanned_files} audio files from ${scanSummary.root}.`;
		} catch (err) { error = toErrorMessage(err); } finally { loadingLibrary = false; }
	}

	async function toggleWatchFolders() {
		error = ''; message = '';
		try {
			if (watchFolders) {
				if (!libraryPath.trim()) { error = 'Set a library path first.'; watchFolders = false; return; }
				await invoke('start_folder_watcher', { path: libraryPath });
				watchingLabel = 'Watching...';
				message = 'Folder watcher started.';
			} else {
				await invoke('stop_folder_watcher');
				watchingLabel = '';
				message = 'Folder watcher stopped.';
			}
		} catch (err) { error = toErrorMessage(err); watchFolders = !watchFolders; }
	}

	async function loadAccount() {
		error = '';
		try {
			account = await invoke<JellyfinAccount | null>('get_jellyfin_account');
			baseUrl = account?.base_url ?? ''; userName = account?.user_name ?? '';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function loadAudioOutputs() {
		error = '';
		try {
			audioOutputs = await invoke<AudioOutputDevice[]>('list_audio_output_devices');
			selectedAudioOutput = audioOutputs.find((d) => d.selected)?.id ?? 'default';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function selectAudioOutput() {
		error = ''; message = '';
		try {
			await invoke<PlaybackStatus>('set_audio_output_device', { device_id: selectedAudioOutput === 'default' ? null : selectedAudioOutput });
			await loadAudioOutputs();
			message = 'Audio output updated.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function loadPlaybackSettings() {
		error = '';
		try {
			const status = await invoke<PlaybackStatus>('get_playback_status');
			replayGainMode = status.replay_gain_mode;
			const settings = await invoke<PlaybackSettings>('get_playback_settings');
			crossfadeEnabled = settings.crossfade_duration_ms !== null;
			crossfadeDuration = settings.crossfade_duration_ms ?? 3000;
			monoDownmix = settings.mono_downmix;
			preampGainDb = settings.preamp_gain_db;
			playbackSpeed = settings.playback_speed;
			playbackSpeedValue = settings.playback_speed.toString();
		}
		catch (err) { error = toErrorMessage(err); }
	}

	async function loadNotificationSetting() {
		try {
			notificationsEnabled = await invoke<boolean>('get_notification_setting');
		} catch (err) {
			notificationsEnabled = true;
		}
	}

	async function toggleNotifications() {
		notificationsEnabled = !notificationsEnabled;
		try {
			await invoke('set_notification_setting', { enabled: notificationsEnabled });
		} catch (err) {
			notificationsEnabled = !notificationsEnabled;
			error = toErrorMessage(err);
		}
	}

	async function loadServiceCapabilities() {
		error = '';
		try {
			serviceCapabilities = await invoke<ProviderCapability[]>('list_service_capabilities');
			if (!credentialProviderOptions().some((p) => p.id === selectedProviderId)) {
				selectedProviderId = credentialProviderOptions()[0]?.id ?? 'spotify';
			}
		} catch (err) { error = toErrorMessage(err); }
	}

	async function loadProviderAccounts() {
		error = ''; try { providerAccounts = await invoke<ProviderAccount[]>('list_provider_accounts'); } catch (err) { error = toErrorMessage(err); }
	}

	async function loadProviderLoginStates() {
		error = ''; try { providerLoginStates = await invoke<ProviderLoginState[]>('list_provider_login_states'); } catch (err) { error = toErrorMessage(err); }
	}

	async function refreshProviderCredentialStatus() {
		await loadProviderAccounts(); await loadProviderLoginStates(); await loadLastFmScrobbleStatus(); await loadProviderStatuses();
	}

	async function loadLastFmScrobbleStatus() {
		error = ''; try { lastFmScrobbleStatus = await invoke<LastFmScrobbleStatus>('get_lastfm_scrobble_status'); } catch (err) { error = toErrorMessage(err); }
	}

	async function retryLastFmScrobbles() {
		error = ''; message = '';
		try { lastFmScrobbleStatus = await invoke<LastFmScrobbleStatus>('retry_lastfm_scrobbles'); message = 'Last.fm scrobble retry finished.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function setReplayGainMode() {
		error = ''; message = '';
		try { const status = await invoke<PlaybackStatus>('set_replay_gain_mode', { mode: replayGainMode }); replayGainMode = status.replay_gain_mode; message = 'ReplayGain mode updated.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function setCrossfade() {
		error = ''; message = '';
		try {
			const duration = crossfadeEnabled ? crossfadeDuration : null;
			await invoke<PlaybackStatus>('set_crossfade', { durationMs: duration });
			message = crossfadeEnabled ? `Crossfade set to ${crossfadeDuration}ms.` : 'Crossfade disabled.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function setMonoDownmix() {
		error = ''; message = '';
		try {
			await invoke<PlaybackStatus>('set_mono_downmix', { enabled: monoDownmix });
			message = monoDownmix ? 'Mono downmix enabled.' : 'Mono downmix disabled.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function setPreampGain() {
		error = ''; message = '';
		try {
			await invoke<PlaybackStatus>('set_preamp_gain', { db: preampGainDb });
			message = `Preamp gain set to ${preampGainDb > 0 ? '+' : ''}${preampGainDb} dB.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function setPlaybackSpeed() {
		error = ''; message = '';
		try {
			playbackSpeed = parseFloat(playbackSpeedValue);
			await invoke<PlaybackStatus>('set_playback_speed', { speed: playbackSpeed });
			message = `Playback speed set to ${playbackSpeed}×.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	function selectedAudioOutputDescription() {
		const device = audioOutputs.find((d) => d.id === selectedAudioOutput);
		return device ? audioOutputDescription(device) : 'No audio output selected';
	}

	function audioOutputDescription(device: AudioOutputDevice) {
		const parts: string[] = [];
		if (device.default_sample_rate) parts.push(formatSampleRate(device.default_sample_rate));
		if (device.default_channels) parts.push(`${device.default_channels} ch`);
		if (device.default_sample_format) parts.push(device.default_sample_format);
		return parts.length > 0 ? parts.join(' / ') : 'No default format reported';
	}

	function formatSampleRate(sampleRate: number) {
		const value = sampleRate / 1000;
		return `${Number.isInteger(value) ? value : value.toFixed(1)} kHz`;
	}

	function stateLabel(value: string) {
		return value.split('_').map((p) => p.charAt(0).toUpperCase() + p.slice(1)).join(' ');
	}

	function yesNo(value: boolean) { return value ? 'Yes' : 'No'; }
	function playbackLabel(provider: ProviderCapability) {
		if (provider.can_stream_full_tracks) return 'Full tracks';
		if (provider.can_stream_previews) return 'Previews';
		if (provider.can_link_out) return 'Link out';
		return 'No';
	}

	function notesPreview(provider: ProviderCapability) { return provider.notes.join(' '); }

	function credentialProviderOptions() {
		return serviceCapabilities.filter((p) => !['local', 'jellyfin', 'lrclib'].includes(p.id));
	}

	function providerName(providerId: string) {
		return serviceCapabilities.find((p) => p.id === providerId)?.name ?? providerId;
	}

	function providerStatusCard(providerId: string) {
		return providerStatuses.find((s) => s.id === providerId) ?? null;
	}

	function loginStateForProvider(providerId: string) {
		return providerLoginStates.find((s) => s.provider_id === providerId) ?? null;
	}

	function credentialStatusLabel(status: ProviderStatus | null) {
		if (!status) return { text: 'Not set', class: 'text-soft' };
		if (status.is_connected) return { text: 'Connected', class: 'text-success' };
		if (status.has_creds && status.is_default) return { text: 'Default', class: 'text-success' };
		if (status.has_creds && !status.is_default) return { text: 'Custom', class: 'text-fg' };
		return { text: 'Not set', class: 'text-soft' };
	}

	function cardBorderClass(status: ProviderStatus | null) {
		if (!status) return 'border-dashed border-outline';
		if (status.is_connected) return 'border-primary/50';
		if (status.has_creds) return 'border-outline';
		return 'border-dashed border-outline';
	}

	function providerIconName(_icon: string) {
		return '';
	}

	function lastFmCredentialsReady() {
		const a = providerAccounts.find((a) => a.provider_id === 'lastfm');
		return Boolean(a?.has_api_key && a.has_api_secret && a.has_access_token);
	}

	async function startSpotifyLogin() {
		error = ''; message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_spotify_pkce_login', { redirect_uri: spotifyRedirectUri, scope: spotifyScope.trim() || null });
			spotifyAuthorizationUrl = login.authorization_url; spotifyAuthorizationState = login.state ?? '';
			message = login.message; await refreshProviderCredentialStatus();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishSpotifyLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_spotify_pkce_login', { code: authCodeFromInput(spotifyAuthorizationCode), state: authStateFromInput(spotifyAuthorizationCode, spotifyAuthorizationState) });
			spotifyAuthorizationCode = ''; spotifyAuthorizationUrl = '';
			await refreshProviderCredentialStatus(); message = 'Spotify tokens saved.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function completeSpotifyLoginInBrowser() {
		error = ''; message = 'Waiting for Spotify authorization in the browser...';
		try {
			await invoke<ProviderAccount>('complete_spotify_pkce_login_in_browser', { redirect_uri: spotifyRedirectUri, scope: spotifyScope.trim() || null });
			spotifyAuthorizationCode = ''; spotifyAuthorizationUrl = '';
			await refreshProviderCredentialStatus(); message = 'Spotify login completed.';
		} catch (err) { error = toErrorMessage(err); message = ''; }
	}

	async function refreshSpotifyToken() {
		error = ''; message = '';
		try { await invoke<ProviderAccount>('refresh_spotify_access_token'); await refreshProviderCredentialStatus(); message = 'Spotify access token refreshed.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function startTidalLogin() {
		error = ''; message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_tidal_pkce_login', { redirect_uri: tidalRedirectUri, scope: tidalScope.trim() || null });
			tidalAuthorizationUrl = login.authorization_url; tidalAuthorizationState = login.state ?? '';
			message = login.message; await refreshProviderCredentialStatus();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishTidalLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_tidal_pkce_login', { code: authCodeFromInput(tidalAuthorizationCode), state: authStateFromInput(tidalAuthorizationCode, tidalAuthorizationState) });
			tidalAuthorizationCode = ''; tidalAuthorizationUrl = '';
			await refreshProviderCredentialStatus(); message = 'TIDAL tokens saved.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function refreshTidalToken() {
		error = ''; message = '';
		try { await invoke<ProviderAccount>('refresh_tidal_access_token'); await refreshProviderCredentialStatus(); message = 'TIDAL access token refreshed.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function startYoutubeLogin() {
		error = ''; message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_youtube_oauth_login', { redirect_uri: youtubeRedirectUri, scope: youtubeScope.trim() || null });
			youtubeAuthorizationUrl = login.authorization_url; youtubeAuthorizationState = login.state ?? '';
			message = login.message; await refreshProviderCredentialStatus();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishYoutubeLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_youtube_oauth_login', { code: authCodeFromInput(youtubeAuthorizationCode), state: authStateFromInput(youtubeAuthorizationCode, youtubeAuthorizationState) });
			youtubeAuthorizationCode = ''; youtubeAuthorizationUrl = '';
			await refreshProviderCredentialStatus(); message = 'YouTube OAuth tokens saved.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function refreshYoutubeToken() {
		error = ''; message = '';
		try { await invoke<ProviderAccount>('refresh_youtube_access_token'); await refreshProviderCredentialStatus(); message = 'YouTube access token refreshed.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function startLastFmLogin() {
		error = ''; message = '';
		try { const login = await invoke<ProviderLoginStart>('start_lastfm_login'); lastFmAuthorizationUrl = login.authorization_url; message = login.message; await refreshProviderCredentialStatus(); }
		catch (err) { error = toErrorMessage(err); }
	}

	async function finishLastFmLogin() {
		error = ''; message = '';
		try { await invoke<ProviderAccount>('finish_lastfm_login'); lastFmAuthorizationUrl = ''; await refreshProviderCredentialStatus(); message = 'Last.fm session key saved.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function saveAccount() {
		error = ''; message = '';
		try { account = await invoke<JellyfinAccount>('save_jellyfin_account', { base_url: baseUrl, user_name: userName, password }); password = ''; message = 'Jellyfin account saved.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	async function clearAccount() {
		error = ''; message = '';
		try { await invoke('clear_jellyfin_account'); account = null; baseUrl = ''; userName = ''; password = ''; message = 'Jellyfin account cleared.'; }
		catch (err) { error = toErrorMessage(err); }
	}

	function authCodeFromInput(value: string) { const t = value.trim(); if (!t) return t; return parameterFromAuthInput(t, 'code') ?? t; }
	function authStateFromInput(value: string, fallbackState: string) { return parameterFromAuthInput(value.trim(), 'state') ?? (fallbackState.trim() || null); }
	function parameterFromAuthInput(value: string, paramName: string) {
		if (!value) return null;
		try {
			const url = new URL(value);
			const qv = url.searchParams.get(paramName); if (qv) return qv;
			if (url.hash.startsWith('#')) {
				const hv = new URLSearchParams(url.hash.slice(1)).get(paramName); if (hv) return hv;
			}
		} catch { return null; }
		return null;
	}

	function toErrorMessage(err: unknown) {
		const message = typeof err === 'string' ? err : err instanceof Error ? err.message : null;
		if (message?.includes('__TAURI_INTERNALS__')) return '';
		if (message) return message; return 'Unexpected application error.';
	}

	// Dev tab helper
	async function loadAllDevStates() {
		for (const pid of ['spotify', 'tidal', 'qobuz', 'youtube', 'lastfm', 'bandcamp', 'soundcloud']) {
			await loadDevProviderState(pid);
		}
	}

	const devProviderNames: Record<string, string> = {
		spotify: 'Spotify', tidal: 'TIDAL', qobuz: 'Qobuz', youtube: 'YouTube Music', lastfm: 'Last.fm', bandcamp: 'Bandcamp', soundcloud: 'SoundCloud'
	};

	const ALL_PROVIDER_IDS = ['spotify', 'tidal', 'qobuz', 'youtube', 'lastfm', 'bandcamp', 'soundcloud'];
</script>

<section class="settings-page" data-od-id="settings-page">
	<div class="heading-bg relative overflow-hidden border border-outline rounded-3xl shadow-2xl p-4 pt-10 pb-10 min-h-[180px]">
		<h1 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(42px,6vw,76px)] leading-[0.94]">Settings</h1>
		<p class="text-soft text-sm">Accounts, audio, and library configuration</p>
	</div>

	{#if error}<p class="mt-3 px-3 py-2 border border-outline rounded-2xl bg-danger/20 text-danger">{error}</p>{/if}
	{#if message}<p class="mt-3 px-3 py-2 border border-outline rounded-2xl bg-success/20 text-success">{message}</p>{/if}

	<Tabs.Root value="general" class="tabs-root">
		<Tabs.List>
			<Tabs.Trigger value="general">{t('settings.general')}</Tabs.Trigger>
			<Tabs.Trigger value="accounts">{t('settings.accounts')}</Tabs.Trigger>
			<Tabs.Trigger value="providers">{t('settings.providers')}</Tabs.Trigger>
			<Tabs.Trigger value="dev">{t('settings.dev')}</Tabs.Trigger>
			<Tabs.Trigger value="audio">{t('settings.audio')}</Tabs.Trigger>
			<Tabs.Trigger value="library">{t('settings.library')}</Tabs.Trigger>
		</Tabs.List>

		<!-- ===== GENERAL TAB ===== -->
		<Tabs.Content value="general" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.accent_color')}</h2>
					<p class="text-soft text-sm">{t('settings.accent_color_desc')}</p>
				</div>
				<div class="flex flex-wrap gap-3">
					{#each accentColors as color}
						<button
							class="color-swatch"
							class:color-swatch-active={selectedAccent === color.id}
							style="--swatch-color: {color.cssValue}"
							onclick={() => selectAccentColor(color.id)}
							aria-label={t(`color.${color.id.replace(/-/g, '_')}` as any)}
							title={t(`color.${color.id.replace(/-/g, '_')}` as any)}
						></button>
					{/each}
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.layout_density')}</h2>
					<p class="text-soft text-sm">{t('settings.layout_density_desc')}</p>
				</div>
				<div class="flex gap-2">
					{#each densityOptions as option}
						<button
							class="density-btn"
							class:density-btn-active={density === option.id}
							onclick={() => selectDensity(option.id)}
						>
							{t(`${option.id}_label` as any)}
						</button>
					{/each}
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.notifications')}</h2>
					<p class="text-soft text-sm">{t('settings.notifications_desc')}</p>
				</div>
				<label class="flex items-center gap-3 cursor-pointer">
					<input type="checkbox" checked={notificationsEnabled} onchange={toggleNotifications} class="w-4 h-4 rounded border-outline" />
					<span class="text-soft text-sm">{t('settings.notifications_label')}</span>
				</label>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.language')}</h2>
					<p class="text-soft text-sm">{t('settings.language_desc')}</p>
				</div>
				<div class="flex gap-2">
					{#each ['en', 'de'] as loc}
						{@const locale = loc as Locale}
						<button
							class="density-btn"
							class:density-btn-active={currentLocale === locale}
							onclick={() => changeLocale(locale)}
						>
							{localeName(locale)}
						</button>
					{/each}
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.high_contrast')}</h2>
					<p class="text-soft text-sm">{t('settings.high_contrast_desc')}</p>
				</div>
				<label class="flex items-center gap-3 cursor-pointer">
					<input type="checkbox" checked={highContrast} onchange={toggleHighContrast} class="w-4 h-4 rounded border-outline" />
					<span class="text-soft text-sm">{t('settings.high_contrast_label')}</span>
				</label>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.services')}</h2>
					<p class="text-soft text-sm">{t('settings.services_desc')}</p>
				</div>
				<div class="overflow-x-auto">
					<table class="w-full min-w-[920px] border-collapse">
						<thead>
							<tr>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Service</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">State</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Auth</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Search</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Playlists</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Playback</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase">Scrobble</th>
								<th class="border-b border-outline px-2 py-2 text-left align-top text-soft font-mono text-xs uppercase min-w-[260px]">Notes</th>
							</tr>
						</thead>
						<tbody>
							{#each serviceCapabilities as provider}
								<tr>
									<td class="border-b border-outline px-2 py-2 align-top text-sm min-w-[130px]">
										<strong>{provider.name}</strong>
										{#if provider.documentation_url}
											<a class="block mt-0.5 text-brand text-xs" href={provider.documentation_url} target="_blank" rel="noreferrer">Docs</a>
										{/if}
									</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">
										<span class="state-pill {provider.integration_state}">{stateLabel(provider.integration_state)}</span>
									</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">{provider.auth_model}</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">{yesNo(provider.can_search)}</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">{yesNo(provider.can_list_playlists)}</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">{playbackLabel(provider)}</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm">{yesNo(provider.can_scrobble)}</td>
									<td class="border-b border-outline px-2 py-2 align-top text-sm text-soft">{notesPreview(provider)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{t('settings.replaygain')}</h2>
					<p class="text-soft text-sm">{t('settings.replaygain_desc')}</p>
				</div>
				<label class="grid gap-2 text-soft text-sm">
					Mode
					<Select.Root bind:value={replayGainMode} onValueChange={setReplayGainMode}>
						<Select.Trigger class="w-[180px]">
							<SelectPrimitive.Value placeholder="Select mode" />
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="off">Off</Select.Item>
							<Select.Item value="track">Track</Select.Item>
							<Select.Item value="album">Album</Select.Item>
						</Select.Content>
					</Select.Root>
				</label>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">About Cold Brew</h2>
					<p class="text-soft text-sm">Version 0.2.0 — A local-first music player</p>
				</div>
				<div class="flex flex-wrap gap-2">
					<Button onclick={() => aboutOpen = true}>About</Button>
				</div>
			</section>
		</Tabs.Content>

		<!-- ===== ACCOUNTS TAB ===== -->
		<Tabs.Content value="accounts" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Jellyfin</h2>
					<p class="text-soft text-sm">
						{#if account}Stored from {account.source}; password present: {account.has_password ? 'yes' : 'no'}
						{:else}No Jellyfin account saved
						{/if}
					</p>
				</div>
				<label class="grid gap-2 text-soft text-sm">Server URL <Input bind:value={baseUrl} placeholder="https://jellyfin.example" class="w-full" /></label>
				<label class="grid gap-2 text-soft text-sm">Username <Input bind:value={userName} autocomplete="username" class="w-full" /></label>
				<label class="grid gap-2 text-soft text-sm">Password <Input bind:value={password} type="password" autocomplete="current-password" class="w-full" /></label>
				<div class="flex flex-wrap gap-2">
					<Button onclick={saveAccount}>Save</Button>
					<Button variant="outline" onclick={clearAccount}>Clear</Button>
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">OAuth Connections</h2>
					<p class="text-soft text-sm">Sign in with your streaming service accounts. Credentials are managed in the Dev tab or from .env defaults.</p>
				</div>

				<div class="grid grid-cols-2 gap-3 max-lg:grid-cols-1">
					<ProviderLoginPanel providerId="spotify" providerName="Spotify" description="OAuth PKCE" loginState={loginStateForProvider('spotify')}>
						<div class="flex flex-wrap gap-2"><Button onclick={completeSpotifyLoginInBrowser}>Login in browser</Button><Button variant="secondary" onclick={startSpotifyLogin}>Manual login URL</Button><Button variant="secondary" onclick={refreshSpotifyToken}>Refresh token</Button></div>
						{#if spotifyAuthorizationUrl}
							<a class="text-brand text-sm" href={spotifyAuthorizationUrl} target="_blank" rel="noreferrer">Open Spotify authorization</a>
							<label class="grid gap-2 text-soft text-sm">Returned URL or code <Input bind:value={spotifyAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-2 text-soft text-sm">State <Input bind:value={spotifyAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishSpotifyLogin}>Finish Spotify login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="tidal" providerName="TIDAL" description="OAuth PKCE" loginState={loginStateForProvider('tidal')}>
						<div class="flex flex-wrap gap-2"><Button onclick={startTidalLogin}>Start TIDAL login</Button><Button variant="secondary" onclick={refreshTidalToken}>Refresh token</Button></div>
						{#if tidalAuthorizationUrl}
							<a class="text-brand text-sm" href={tidalAuthorizationUrl} target="_blank" rel="noreferrer">Open TIDAL authorization</a>
							<label class="grid gap-2 text-soft text-sm">Returned URL or code <Input bind:value={tidalAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-2 text-soft text-sm">State <Input bind:value={tidalAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishTidalLogin}>Finish TIDAL login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="youtube" providerName="YouTube" description="Google OAuth" loginState={loginStateForProvider('youtube')}>
						<div class="flex flex-wrap gap-2"><Button onclick={startYoutubeLogin}>Start YouTube login</Button><Button variant="secondary" onclick={refreshYoutubeToken}>Refresh token</Button></div>
						{#if youtubeAuthorizationUrl}
							<a class="text-brand text-sm" href={youtubeAuthorizationUrl} target="_blank" rel="noreferrer">Open Google authorization</a>
							<label class="grid gap-2 text-soft text-sm">Returned URL or code <Input bind:value={youtubeAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-2 text-soft text-sm">State <Input bind:value={youtubeAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishYoutubeLogin}>Finish YouTube login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="lastfm" providerName="Last.fm" description="Desktop session" loginState={loginStateForProvider('lastfm')}>
						<div class="flex flex-wrap gap-2"><Button onclick={startLastFmLogin}>Start Last.fm login</Button></div>
						{#if lastFmAuthorizationUrl}
							<a class="text-brand text-sm" href={lastFmAuthorizationUrl} target="_blank" rel="noreferrer">Open Last.fm authorization</a>
							<div class="flex flex-wrap gap-2"><Button onclick={finishLastFmLogin}>Finish Last.fm login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="qobuz" providerName="Qobuz" description="App credentials" loginState={loginStateForProvider('qobuz')}>
						<p>{loginStateForProvider('qobuz')?.message ?? 'No Qobuz credentials saved'}</p>
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="bandcamp" providerName="Bandcamp" description="Link-out" loginState={loginStateForProvider('bandcamp')}>
						<p>{loginStateForProvider('bandcamp')?.message ?? 'No Bandcamp login state available'}</p>
						<a class="text-brand text-sm" href="https://bandcamp.com/developer" target="_blank" rel="noreferrer">Open Bandcamp developer docs</a>
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="soundcloud" providerName="SoundCloud" description="Built-in API key" loginState={loginStateForProvider('soundcloud')}>
						<p>{loginStateForProvider('soundcloud')?.message ?? 'SoundCloud search uses a built-in client ID. Register your own app for higher rate limits.'}</p>
						<a class="text-brand text-sm" href="https://developers.soundcloud.com/" target="_blank" rel="noreferrer">Open SoundCloud for Developers</a>
					</ProviderLoginPanel>
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Last.fm Scrobbling</h2>
					<p class="text-soft text-sm">{lastFmCredentialsReady() ? 'Last.fm scrobbling credentials are ready' : 'Save a Last.fm API key, API secret, and session key'}</p>
				</div>
				<div class="grid grid-cols-3 gap-2.5 max-md:grid-cols-1">
					<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Pending</span><strong class="text-xl">{lastFmScrobbleStatus?.pending_count ?? 0}</strong></div>
					<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Submitted</span><strong class="text-xl">{lastFmScrobbleStatus?.submitted_count ?? 0}</strong></div>
					<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Failed</span><strong class="text-xl">{lastFmScrobbleStatus?.failed_count ?? 0}</strong></div>
				</div>
				{#if lastFmScrobbleStatus?.last_error}<p class="text-red-600">{lastFmScrobbleStatus.last_error}</p>{/if}
				<div class="flex flex-wrap gap-2"><Button onclick={retryLastFmScrobbles} disabled={!lastFmCredentialsReady()}>Retry pending scrobbles</Button><Button variant="secondary" onclick={loadLastFmScrobbleStatus}>Refresh status</Button></div>
			</section>
		</Tabs.Content>

		<!-- ===== PROVIDERS TAB ===== -->
		<Tabs.Content value="providers" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Provider Status</h2>
					<p class="text-soft text-sm">Credential and connection status for each streaming service</p>
				</div>
				<div class="grid grid-cols-2 gap-3 max-lg:grid-cols-1">
					{#each ALL_PROVIDER_IDS as pid}
						{@const status = providerStatusCard(pid)}
						<Card.Root class={cardBorderClass(status) + ' transition-colors'}>
							<Card.Header>
								<Card.Title class="flex items-center gap-2.5 text-sm">
									<span class="text-lg">{providerIconName(status?.icon ?? '')}</span>
									{status?.name ?? providerName(pid)}
								</Card.Title>
								<Card.Description>
									<div class="flex flex-col gap-1 mt-1">
										<span class={credentialStatusLabel(status).class}>
											Credentials: {credentialStatusLabel(status).text}
										</span>
										{#if status}
											<span class="text-soft">
											{status.is_connected ? 'Status: Connected' : status.has_creds ? 'Status: Ready for OAuth' : 'Status: Not configured'}
											</span>
										{:else}
											<span class="text-soft">Configure in Dev tab</span>
										{/if}
									</div>
								</Card.Description>
							</Card.Header>
							<Card.Footer>
								{#if status?.is_connected}
									<Badge variant="outline" class="border-green-600/40 text-green-600">Connected</Badge>
								{:else if status?.has_creds}
									{#if pid === 'spotify'}
										<Button size="sm" variant="outline" onclick={completeSpotifyLoginInBrowser}>Connect</Button>
									{:else if pid === 'tidal'}
										<Button size="sm" variant="outline" onclick={startTidalLogin}>Connect</Button>
									{:else if pid === 'youtube'}
										<Button size="sm" variant="outline" onclick={startYoutubeLogin}>Connect</Button>
									{:else if pid === 'lastfm'}
										<Button size="sm" variant="outline" onclick={startLastFmLogin}>Connect</Button>
									{:else}
										<span class="text-soft text-sm">No OAuth flow</span>
									{/if}
								{:else}
									<span class="text-soft text-sm">Not configured</span>
								{/if}
							</Card.Footer>
						</Card.Root>
					{/each}
				</div>
				<div class="flex flex-wrap gap-2 mt-3">
					<Button variant="secondary" onclick={loadProviderStatuses}>Refresh status</Button>
				</div>
			</section>
		</Tabs.Content>

		<!-- ===== DEV TAB ===== -->
		<Tabs.Content value="dev" class="settings-tab-content">
			<div class="mb-4 px-3 py-2 border border-warning/40 rounded-2xl bg-warning/10">
			<p class="m-0 text-warning text-sm">
			<strong>Developer Settings</strong> &mdash; changing credentials may break playback. Use the Providers tab for normal setup.
			</p>
			</div>

			{#each ALL_PROVIDER_IDS as pid}
				{@const state = devProviderStates[pid]}
				<section class="settings-panel">
					<div>
						<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(20px,2vw,26px)] leading-[1.04]">{devProviderNames[pid] ?? providerName(pid)}</h2>
						<p class="text-soft text-sm">
							{#if state}
								{#if state.is_default}
									<span class="text-green-600">Using defaults (from .env / keyring)</span>
								{:else if state.has_creds}
									<span class="text-accent">Custom credentials active</span>
								{:else}
									<span class="text-soft">No credentials configured</span>
								{/if}
							{:else}
								<span class="text-soft">Loading...</span>
							{/if}
						</p>
					</div>

					{#each providerBaseFields(pid) as field}
						{@const fieldKey = field.key}
						<label class="grid gap-2 text-soft text-sm">
							{field.label}
							<div class="flex gap-2">
								<Input
									type={fieldKey.includes('secret') || fieldKey.includes('key') ? 'password' : 'text'}
									value={devFieldValues[pid]?.[fieldKey] ?? ''}
									oninput={(e) => {
										if (!devFieldValues[pid]) devFieldValues[pid] = {};
										devFieldValues[pid][fieldKey] = (e.target as HTMLInputElement).value;
									}}
									placeholder={state?.is_default ? '•••••••• (using defaults)' : `Enter ${field.label}`}
									disabled={devUseDefault[pid] ?? true}
									class="flex-1"
									autocomplete="off"
								/>
							</div>
						</label>
					{/each}

					<div class="flex flex-wrap gap-2 items-center mt-3">
						<label class="flex items-center gap-2 text-sm cursor-pointer">
							<input type="checkbox" bind:checked={devUseDefault[pid]} class="w-4 h-4 rounded border-outline" />
							<span class="text-soft">Use Default</span>
						</label>
					</div>

					<div class="flex flex-wrap gap-2 mt-2">
						<Button size="sm" onclick={() => saveDevCredentials(pid)} disabled={devUseDefault[pid] ?? true}>Save</Button>
						<Button size="sm" variant="outline" onclick={() => resetDevToDefaults(pid)}>Reset to Defaults</Button>
						<Button size="sm" variant="secondary" onclick={() => loadDevProviderState(pid)}>Reload</Button>
					</div>
				</section>
			{/each}
		</Tabs.Content>

		<!-- ===== AUDIO TAB ===== -->
		<Tabs.Content value="audio" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Audio Output</h2>
					<p class="text-soft text-sm">{selectedAudioOutputDescription()}</p>
				</div>
				<label class="grid gap-2 text-soft text-sm">Output device
					<Select.Root bind:value={selectedAudioOutput} onValueChange={selectAudioOutput}>
						<Select.Trigger class="w-full">
							<SelectPrimitive.Value placeholder="Select output device" />
						</Select.Trigger>
						<Select.Content>
							{#each audioOutputs as device}
								<Select.Item value={device.id}>{device.name}</Select.Item>
							{/each}
						</Select.Content>
					</Select.Root>
				</label>
				<div class="flex flex-wrap gap-2"><Button variant="secondary" onclick={loadAudioOutputs}>Refresh devices</Button></div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Crossfade</h2>
					<p class="text-soft text-sm">Smoothly transition between tracks</p>
				</div>
				<label class="flex items-center gap-3 cursor-pointer">
					<input type="checkbox" bind:checked={crossfadeEnabled} onchange={setCrossfade} class="w-4 h-4 rounded border-outline" />
					<span class="text-soft text-sm">Enable crossfade</span>
				</label>
				{#if crossfadeEnabled}
					<label class="grid gap-2 text-soft text-sm">
						Duration: {crossfadeDuration}ms
						<Slider
							value={[crossfadeDuration]}
							min={500}
							max={12000}
							step={500}
							onValueChange={(v: number[]) => { crossfadeDuration = v[0]; }}
							onValueCommit={() => setCrossfade()}
						/>
					</label>
				{/if}
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Mono Downmix</h2>
					<p class="text-soft text-sm">Convert stereo audio to mono</p>
				</div>
				<label class="flex items-center gap-3 cursor-pointer">
					<input type="checkbox" bind:checked={monoDownmix} onchange={setMonoDownmix} class="w-4 h-4 rounded border-outline" />
					<span class="text-soft text-sm">Enable mono output</span>
				</label>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Pre-amplifier</h2>
					<p class="text-soft text-sm">Boost or attenuate audio before output</p>
				</div>
				<label class="grid gap-2 text-soft text-sm">
					Gain: {preampGainDb > 0 ? '+' : ''}{preampGainDb} dB
					<Slider
						value={[preampGainDb]}
						min={-12}
						max={12}
						step={0.5}
						onValueChange={(v: number[]) => { preampGainDb = v[0]; }}
						onValueCommit={() => setPreampGain()}
					/>
				</label>
				<div class="flex flex-wrap gap-2"><Button onclick={setPreampGain}>Apply</Button></div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Playback Speed</h2>
					<p class="text-soft text-sm">Adjust playback speed (0.5× to 2.0×)</p>
				</div>
				<label class="grid gap-2 text-soft text-sm">
					Speed: {playbackSpeed}&times;
					<Select.Root bind:value={playbackSpeedValue} onValueChange={async () => { await setPlaybackSpeed(); }}>
						<Select.Trigger class="w-[120px]">
							<SelectPrimitive.Value placeholder={`${playbackSpeed}×`} />
						</Select.Trigger>
						<Select.Content>
							<Select.Item value="0.5">0.5×</Select.Item>
							<Select.Item value="0.75">0.75×</Select.Item>
							<Select.Item value="1.0">1.0×</Select.Item>
							<Select.Item value="1.25">1.25×</Select.Item>
							<Select.Item value="1.5">1.5×</Select.Item>
							<Select.Item value="2.0">2.0×</Select.Item>
						</Select.Content>
					</Select.Root>
				</label>
			</section>
		</Tabs.Content>

		<!-- ===== LIBRARY TAB ===== -->
		<Tabs.Content value="library" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Library Scan</h2>
					<p class="text-soft text-sm">Index local music files for browsing and playback</p>
				</div>
				<div class="flex gap-2"><Input bind:value={libraryPath} placeholder="/path/to/music" aria-label="Music folder path" class="flex-1" /><Button onclick={scanLibrary} disabled={loadingLibrary}>Scan</Button></div>
				{#if scanSummary}
					<div class="grid grid-cols-4 gap-2.5 max-xl:grid-cols-2">
						<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Scanned</span><strong class="text-xl break-words">{scanSummary.scanned_files}</strong></div>
						<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Indexed</span><strong class="text-xl break-words">{scanSummary.indexed_tracks}</strong></div>
						<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Skipped</span><strong class="text-xl break-words">{scanSummary.skipped_files}</strong></div>
						<div class="grid gap-1 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-xs uppercase">Root</span><strong class="font-mono text-xs break-words">{scanSummary.root}</strong></div>
					</div>
				{/if}
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Watch Folders</h2>
					<p class="text-soft text-sm">Automatically scan for new audio files in the library root</p>
				</div>
				<label class="flex items-center gap-3 cursor-pointer">
					<input type="checkbox" bind:checked={watchFolders} onchange={toggleWatchFolders} class="w-4 h-4 rounded border-outline" />
					<span class="text-soft text-sm">Watch folders for new files</span>
					{#if watchingLabel}
						<span class="text-xs font-mono text-soft ml-2">{watchingLabel}</span>
					{/if}
				</label>
			</section>
		</Tabs.Content>
	</Tabs.Root>

	<!-- About Dialog -->
	<DialogComponents.Root bind:open={aboutOpen}>
		<DialogComponents.Portal>
			<DialogComponents.Overlay />
			<DialogComponents.Content class="max-w-md">
				<DialogComponents.Header>
					<DialogComponents.Title>About Cold Brew</DialogComponents.Title>
					<DialogComponents.Description>
						A local-first music player and streaming aggregator
					</DialogComponents.Description>
				</DialogComponents.Header>
				<div class="px-6 py-4 space-y-3 text-sm text-soft">
					<p>
						<strong class="text-fg">Cold Brew</strong> v0.2.0
					</p>
					<p>
						Play local music files, stream from Spotify, TIDAL, YouTube Music,
						Jellyfin, and more. All from one unified queue.
					</p>
					<p>
						Made with <span class="text-brand">☕</span> and Rust
					</p>
					<p>
						<a
							href="https://github.com/maxisitter/cold-brew"
							target="_blank"
							rel="noreferrer"
							class="text-brand underline"
						>
							View on GitHub
						</a>
					</p>
					<p>
						Licensed under MIT
					</p>
				</div>
				<DialogComponents.Footer>
				<Button variant="outline" onclick={() => aboutOpen = false}>Close</Button>
				</DialogComponents.Footer>
			</DialogComponents.Content>
		</DialogComponents.Portal>
	</DialogComponents.Root>
</section>
