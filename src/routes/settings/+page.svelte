<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type {
		AudioOutputDevice,
		JellyfinAccount,
		LastFmScrobbleStatus,
		PlaybackStatus,
		ProviderAccount,
		ProviderCapability,
		ProviderLoginStart,
		ProviderLoginState,
		ScanSummary
	} from '$lib/types';
	import * as Tabs from '$lib/components/ui/tabs';
	import * as Select from '$lib/components/ui/select';
	import { Select as SelectPrimitive } from 'bits-ui';
	import { Input } from '$lib/components/ui/input';
	import { Button } from '$lib/components/ui/button';
	import ProviderLoginPanel from '$lib/components/ProviderLoginPanel.svelte';

	let account: JellyfinAccount | null = null;
	let audioOutputs: AudioOutputDevice[] = [];
	let serviceCapabilities: ProviderCapability[] = [];
	let providerAccounts: ProviderAccount[] = [];
	let providerLoginStates: ProviderLoginState[] = [];
	let lastFmScrobbleStatus: LastFmScrobbleStatus | null = null;
	let baseUrl = '';
	let userName = '';
	let password = '';
	let selectedAudioOutput = 'default';
	let replayGainMode = 'off';
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

	onMount(() => {
		void loadAccount(); void loadAudioOutputs(); void loadPlaybackSettings();
		void loadServiceCapabilities(); void loadProviderAccounts();
		void loadProviderLoginStates(); void loadLastFmScrobbleStatus();
	});

	async function scanLibrary() {
		if (!libraryPath.trim()) { error = 'Enter a local music folder path.'; return; }
		loadingLibrary = true; error = ''; message = '';
		try {
			scanSummary = await invoke<ScanSummary>('scan_library_path', { path: libraryPath });
			message = `Indexed ${scanSummary.indexed_tracks} of ${scanSummary.scanned_files} audio files from ${scanSummary.root}.`;
		} catch (err) { error = toErrorMessage(err); } finally { loadingLibrary = false; }
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
		try { const status = await invoke<PlaybackStatus>('get_playback_status'); replayGainMode = status.replay_gain_mode; }
		catch (err) { error = toErrorMessage(err); }
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
		await loadProviderAccounts(); await loadProviderLoginStates(); await loadLastFmScrobbleStatus();
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

	function accountForProvider(providerId: string) {
		return providerAccounts.find((a) => a.provider_id === providerId) ?? null;
	}

	function providerLoginStateRows() {
		const ids = new Set(credentialProviderOptions().map((p) => p.id));
		return providerLoginStates.filter((s) => ids.has(s.provider_id));
	}

	function providerName(providerId: string) {
		return serviceCapabilities.find((p) => p.id === providerId)?.name ?? providerId;
	}

	function savedProviderFlags(account: ProviderAccount) {
		const flags: string[] = [];
		if (account.display_name) flags.push('label');
		if (account.has_client_id) flags.push('client id');
		if (account.has_client_secret) flags.push('client secret');
		if (account.has_api_key) flags.push('api key');
		if (account.has_api_secret) flags.push('api secret');
		if (account.has_access_token) flags.push('access token');
		if (account.has_refresh_token) flags.push('refresh token');
		return flags.join(', ');
	}

	function lastFmCredentialsReady() {
		const a = accountForProvider('lastfm');
		return Boolean(a?.has_api_key && a.has_api_secret && a.has_access_token);
	}

	function loginStateForProvider(providerId: string) {
		return providerLoginStates.find((s) => s.provider_id === providerId) ?? null;
	}

	function selectProviderCredentials(providerId: string) {
		selectedProviderId = providerId;
		message = `${providerName(providerId)} selected in Service Credentials.`; error = '';
	}

	function optionalCredential(value: string) { const t = value.trim(); return t.length > 0 ? t : null; }

	function authorizationCodeFromInput(value: string) {
		const t = value.trim(); if (!t) return t;
		return parameterFromAuthorizationInput(t, 'code') ?? t;
	}

	function authorizationStateFromInput(value: string, fallbackState: string) {
		return parameterFromAuthorizationInput(value.trim(), 'state') ?? optionalCredential(fallbackState);
	}

	function parameterFromAuthorizationInput(value: string, parameterName: string) {
		if (!value) return null;
		try {
			const url = new URL(value);
			const qv = url.searchParams.get(parameterName); if (qv) return qv;
			if (url.hash.startsWith('#')) {
				const hv = new URLSearchParams(url.hash.slice(1)).get(parameterName); if (hv) return hv;
			}
		} catch { return null; }
		return null;
	}

	function clearProviderFormSecrets() {
		providerClientId = ''; providerClientSecret = ''; providerApiKey = ''; providerApiSecret = '';
		providerAccessToken = ''; providerRefreshToken = '';
	}

	async function saveProviderAccount() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('save_provider_account', {
				provider_id: selectedProviderId, display_name: optionalCredential(providerDisplayName),
				client_id: optionalCredential(providerClientId), client_secret: optionalCredential(providerClientSecret),
				api_key: optionalCredential(providerApiKey), api_secret: optionalCredential(providerApiSecret),
				access_token: optionalCredential(providerAccessToken), refresh_token: optionalCredential(providerRefreshToken)
			});
			await loadProviderAccounts(); await loadProviderLoginStates();
			providerDisplayName = ''; clearProviderFormSecrets();
			message = `${providerName(selectedProviderId)} credentials saved.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function clearProviderAccount() {
		error = ''; message = '';
		try {
			await invoke('clear_provider_account', { provider_id: selectedProviderId });
			await loadProviderAccounts(); await loadProviderLoginStates();
			providerDisplayName = ''; clearProviderFormSecrets();
			message = `${providerName(selectedProviderId)} credentials cleared.`;
		} catch (err) { error = toErrorMessage(err); }
	}

	async function startSpotifyLogin() {
		error = ''; message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_spotify_pkce_login', { redirect_uri: spotifyRedirectUri, scope: optionalCredential(spotifyScope) });
			spotifyAuthorizationUrl = login.authorization_url; spotifyAuthorizationState = login.state ?? '';
			message = login.message; await loadProviderLoginStates();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishSpotifyLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_spotify_pkce_login', { code: authorizationCodeFromInput(spotifyAuthorizationCode), state: authorizationStateFromInput(spotifyAuthorizationCode, spotifyAuthorizationState) });
			spotifyAuthorizationCode = ''; spotifyAuthorizationUrl = '';
			await refreshProviderCredentialStatus(); message = 'Spotify tokens saved.';
		} catch (err) { error = toErrorMessage(err); }
	}

	async function completeSpotifyLoginInBrowser() {
		error = ''; message = 'Waiting for Spotify authorization in the browser...';
		try {
			await invoke<ProviderAccount>('complete_spotify_pkce_login_in_browser', { redirect_uri: spotifyRedirectUri, scope: optionalCredential(spotifyScope) });
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
			const login = await invoke<ProviderLoginStart>('start_tidal_pkce_login', { redirect_uri: tidalRedirectUri, scope: optionalCredential(tidalScope) });
			tidalAuthorizationUrl = login.authorization_url; tidalAuthorizationState = login.state ?? '';
			message = login.message; await loadProviderLoginStates();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishTidalLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_tidal_pkce_login', { code: authorizationCodeFromInput(tidalAuthorizationCode), state: authorizationStateFromInput(tidalAuthorizationCode, tidalAuthorizationState) });
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
			const login = await invoke<ProviderLoginStart>('start_youtube_oauth_login', { redirect_uri: youtubeRedirectUri, scope: optionalCredential(youtubeScope) });
			youtubeAuthorizationUrl = login.authorization_url; youtubeAuthorizationState = login.state ?? '';
			message = login.message; await loadProviderLoginStates();
		} catch (err) { error = toErrorMessage(err); }
	}

	async function finishYoutubeLogin() {
		error = ''; message = '';
		try {
			await invoke<ProviderAccount>('finish_youtube_oauth_login', { code: authorizationCodeFromInput(youtubeAuthorizationCode), state: authorizationStateFromInput(youtubeAuthorizationCode, youtubeAuthorizationState) });
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
		try { const login = await invoke<ProviderLoginStart>('start_lastfm_login'); lastFmAuthorizationUrl = login.authorization_url; message = login.message; await loadProviderLoginStates(); }
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

	function toErrorMessage(err: unknown) {
		const message = typeof err === 'string' ? err : err instanceof Error ? err.message : null;
		if (message?.includes('__TAURI_INTERNALS__')) return '';
		if (message) return message; return 'Unexpected application error.';
	}
</script>

<section class="settings-page">
	<div class="heading-bg relative overflow-hidden border border-outline rounded-3xl shadow-2xl p-[clamp(22px,5vw,52px)]" style="min-height: 180px">
		<h1 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(42px,6vw,76px)] leading-[0.94]">Settings</h1>
		<p class="text-soft text-[0.9rem]">Accounts, audio, and library configuration</p>
	</div>

	{#if error}<p class="mt-3 px-3.5 py-2.5 border border-outline rounded-[20px] bg-danger/20 text-danger/70">{error}</p>{/if}
	{#if message}<p class="mt-3 px-3.5 py-2.5 border border-outline rounded-[20px] bg-success/20 text-success/80">{message}</p>{/if}

	<Tabs.Root value="general" class="tabs-root">
		<Tabs.List>
			<Tabs.Trigger value="general">General</Tabs.Trigger>
			<Tabs.Trigger value="accounts">Accounts</Tabs.Trigger>
			<Tabs.Trigger value="audio">Audio</Tabs.Trigger>
			<Tabs.Trigger value="library">Library</Tabs.Trigger>
		</Tabs.List>

		<Tabs.Content value="general" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Services</h2>
					<p class="text-soft text-[0.9rem]">Provider capabilities and current implementation state</p>
				</div>

				<div class="overflow-x-auto">
					<table class="w-full min-w-[920px] border-collapse">
						<thead>
							<tr>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Service</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">State</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Auth</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Search</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Playlists</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Playback</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Scrobble</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase min-w-[260px]">Notes</th>
							</tr>
						</thead>
						<tbody>
							{#each serviceCapabilities as provider}
								<tr>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem] min-w-[130px]">
										<strong>{provider.name}</strong>
										{#if provider.documentation_url}
											<a class="block mt-0.5 text-brand text-[0.76rem]" href={provider.documentation_url} target="_blank" rel="noreferrer">Docs</a>
										{/if}
									</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">
										<span class="state-pill {provider.integration_state}">{stateLabel(provider.integration_state)}</span>
									</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{provider.auth_model}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{yesNo(provider.can_search)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{yesNo(provider.can_list_playlists)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{playbackLabel(provider)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{yesNo(provider.can_scrobble)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem] text-soft">{notesPreview(provider)}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">ReplayGain</h2>
					<p class="text-soft text-[0.9rem]">Applied during local playback when matching ReplayGain tags are present</p>
				</div>
				<label class="grid gap-[6px] text-soft text-[0.86rem]">
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
		</Tabs.Content>

		<Tabs.Content value="accounts" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Jellyfin</h2>
					<p class="text-soft text-[0.9rem]">
						{#if account}Stored from {account.source}; password present: {account.has_password ? 'yes' : 'no'}
						{:else}No Jellyfin account saved
						{/if}
					</p>
				</div>
				<label class="grid gap-[6px] text-soft text-[0.86rem]">Server URL <Input bind:value={baseUrl} placeholder="https://jellyfin.example" class="w-full" /></label>
				<label class="grid gap-[6px] text-soft text-[0.86rem]">Username <Input bind:value={userName} autocomplete="username" class="w-full" /></label>
				<label class="grid gap-[6px] text-soft text-[0.86rem]">Password <Input bind:value={password} type="password" autocomplete="current-password" class="w-full" /></label>
				<div class="flex flex-wrap gap-2">
					<Button onclick={saveAccount}>Save</Button>
					<Button variant="outline" onclick={clearAccount}>Clear</Button>
				</div>
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Service Credentials</h2>
					<p class="text-soft text-[0.9rem]">Secure storage for provider OAuth/API material and user app credentials</p>
				</div>

				<div class="grid grid-cols-2 gap-3 max-lg:grid-cols-1">
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Service
						<Select.Root bind:value={selectedProviderId}>
							<Select.Trigger class="w-full">
								<SelectPrimitive.Value placeholder="Select service" />
							</Select.Trigger>
							<Select.Content>
								{#each credentialProviderOptions() as provider}
									<Select.Item value={provider.id}>{provider.name}</Select.Item>
								{/each}
							</Select.Content>
						</Select.Root>
					</label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Account label <Input bind:value={providerDisplayName} placeholder="Personal account" autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Client ID / App ID <Input bind:value={providerClientId} autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Client secret / App secret <Input bind:value={providerClientSecret} type="password" autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">API key <Input bind:value={providerApiKey} autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">API secret <Input bind:value={providerApiSecret} type="password" autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Access token / Session key <Input bind:value={providerAccessToken} type="password" autocomplete="off" class="w-full" /></label>
					<label class="grid gap-[6px] text-soft text-[0.86rem]">Refresh token <Input bind:value={providerRefreshToken} type="password" autocomplete="off" class="w-full" /></label>
				</div>

				<div class="flex flex-wrap gap-2">
					<Button onclick={saveProviderAccount}>Save provider credentials</Button>
					<Button variant="outline" onclick={clearProviderAccount}>Clear selected service</Button>
					<Button variant="secondary" onclick={refreshProviderCredentialStatus}>Refresh status</Button>
				</div>

				{#if providerLoginStateRows().length > 0}
					<table class="w-full border-collapse">
						<thead>
							<tr>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Service</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Login state</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Details</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Last failure</th>
							</tr>
						</thead>
						<tbody>
							{#each providerLoginStateRows() as loginState}
								<tr>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{providerName(loginState.provider_id)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]"><span class="state-pill {loginState.status}">{stateLabel(loginState.status)}</span></td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{loginState.message}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem] {loginState.status === 'failed' ? 'text-danger' : ''}">{loginState.last_error ?? 'None'}</td>
								</tr>
							{/each}
						</tbody>
					</table>
				{/if}

				<div class="grid grid-cols-2 gap-3 max-lg:grid-cols-1">
					<ProviderLoginPanel providerId="spotify" providerName="Spotify" description="OAuth PKCE" loginState={loginStateForProvider('spotify')}>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Redirect URI <Input bind:value={spotifyRedirectUri} autocomplete="off" class="w-full" /></label>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Scope <Input bind:value={spotifyScope} autocomplete="off" class="w-full" /></label>
						<div class="flex flex-wrap gap-2"><Button onclick={completeSpotifyLoginInBrowser}>Login in browser</Button><Button variant="secondary" onclick={startSpotifyLogin}>Manual login URL</Button><Button variant="secondary" onclick={refreshSpotifyToken}>Refresh Spotify token</Button></div>
						{#if spotifyAuthorizationUrl}
							<a class="text-brand text-[0.86rem]" href={spotifyAuthorizationUrl} target="_blank" rel="noreferrer">Open Spotify authorization</a>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">Returned URL or code <Input bind:value={spotifyAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">State <Input bind:value={spotifyAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishSpotifyLogin}>Finish Spotify login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="tidal" providerName="TIDAL" description="OAuth PKCE" loginState={loginStateForProvider('tidal')}>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Redirect URI <Input bind:value={tidalRedirectUri} autocomplete="off" class="w-full" /></label>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Scope <Input bind:value={tidalScope} autocomplete="off" class="w-full" /></label>
						<div class="flex flex-wrap gap-2"><Button onclick={startTidalLogin}>Start TIDAL login</Button><Button variant="secondary" onclick={refreshTidalToken}>Refresh TIDAL token</Button></div>
						{#if tidalAuthorizationUrl}
							<a class="text-brand text-[0.86rem]" href={tidalAuthorizationUrl} target="_blank" rel="noreferrer">Open TIDAL authorization</a>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">Returned URL or code <Input bind:value={tidalAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">State <Input bind:value={tidalAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishTidalLogin}>Finish TIDAL login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="youtube" providerName="YouTube" description="Google OAuth" loginState={loginStateForProvider('youtube')}>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Redirect URI <Input bind:value={youtubeRedirectUri} autocomplete="off" class="w-full" /></label>
						<label class="grid gap-[6px] text-soft text-[0.86rem]">Scope <Input bind:value={youtubeScope} autocomplete="off" class="w-full" /></label>
						<div class="flex flex-wrap gap-2"><Button onclick={startYoutubeLogin}>Start YouTube login</Button><Button variant="secondary" onclick={refreshYoutubeToken}>Refresh YouTube token</Button></div>
						{#if youtubeAuthorizationUrl}
							<a class="text-brand text-[0.86rem]" href={youtubeAuthorizationUrl} target="_blank" rel="noreferrer">Open Google authorization</a>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">Returned URL or code <Input bind:value={youtubeAuthorizationCode} autocomplete="off" class="w-full" /></label>
							<label class="grid gap-[6px] text-soft text-[0.86rem]">State <Input bind:value={youtubeAuthorizationState} autocomplete="off" class="w-full" /></label>
							<div class="flex flex-wrap gap-2"><Button onclick={finishYoutubeLogin}>Finish YouTube login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="lastfm" providerName="Last.fm" description="Desktop session" loginState={loginStateForProvider('lastfm')}>
						<div class="flex flex-wrap gap-2"><Button onclick={startLastFmLogin}>Start Last.fm login</Button></div>
						{#if lastFmAuthorizationUrl}
							<a class="text-brand text-[0.86rem]" href={lastFmAuthorizationUrl} target="_blank" rel="noreferrer">Open Last.fm authorization</a>
							<div class="flex flex-wrap gap-2"><Button onclick={finishLastFmLogin}>Finish Last.fm login</Button></div>
						{/if}
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="qobuz" providerName="Qobuz" description="App credentials" loginState={loginStateForProvider('qobuz')}>
						<p>{loginStateForProvider('qobuz')?.message ?? 'No Qobuz credentials saved'}</p>
						<div class="flex flex-wrap gap-2"><Button variant="secondary" onclick={() => selectProviderCredentials('qobuz')}>Edit Qobuz credentials</Button></div>
					</ProviderLoginPanel>

					<ProviderLoginPanel providerId="bandcamp" providerName="Bandcamp" description="Link-out" loginState={loginStateForProvider('bandcamp')}>
						<p>{loginStateForProvider('bandcamp')?.message ?? 'No Bandcamp login state available'}</p>
						<div class="flex flex-wrap gap-2"><Button variant="secondary" onclick={() => selectProviderCredentials('bandcamp')}>Edit Bandcamp note</Button></div>
						<a class="text-brand text-[0.86rem]" href="https://bandcamp.com/developer" target="_blank" rel="noreferrer">Open Bandcamp developer docs</a>
					</ProviderLoginPanel>
				</div>

				{#if providerAccounts.length > 0}
					<table class="w-full border-collapse">
						<thead>
							<tr><th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Service</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Saved fields</th>
								<th class="border-b border-outline px-[0.6rem] py-[0.55rem] text-left align-top text-soft font-mono text-[0.76rem] uppercase">Source</th></tr>
						</thead>
						<tbody>
							{#each providerAccounts as pa}
								<tr><td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{providerName(pa.provider_id)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{savedProviderFlags(pa)}</td>
									<td class="border-b border-outline px-[0.6rem] py-[0.55rem] align-top text-[0.82rem]">{pa.source}</td></tr>
							{/each}
						</tbody>
					</table>
				{/if}
			</section>

			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Last.fm Scrobbling</h2>
					<p class="text-soft text-[0.9rem]">{lastFmCredentialsReady() ? 'Last.fm scrobbling credentials are ready' : 'Save a Last.fm API key, API secret, and session key in Service Credentials'}</p>
				</div>
				<div class="grid grid-cols-3 gap-2.5 max-md:grid-cols-1">
					<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Pending</span><strong class="text-xl">{lastFmScrobbleStatus?.pending_count ?? 0}</strong></div>
					<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Submitted</span><strong class="text-xl">{lastFmScrobbleStatus?.submitted_count ?? 0}</strong></div>
					<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Failed</span><strong class="text-xl">{lastFmScrobbleStatus?.failed_count ?? 0}</strong></div>
				</div>
				{#if lastFmScrobbleStatus?.last_error}<p class="text-danger">{lastFmScrobbleStatus.last_error}</p>{/if}
				<div class="flex flex-wrap gap-2"><Button onclick={retryLastFmScrobbles} disabled={!lastFmCredentialsReady()}>Retry pending scrobbles</Button><Button variant="secondary" onclick={loadLastFmScrobbleStatus}>Refresh status</Button></div>
			</section>
		</Tabs.Content>

		<Tabs.Content value="audio" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Audio Output</h2>
					<p class="text-soft text-[0.9rem]">{selectedAudioOutputDescription()}</p>
				</div>
				<label class="grid gap-[6px] text-soft text-[0.86rem]">Output device
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
		</Tabs.Content>

		<Tabs.Content value="library" class="settings-tab-content">
			<section class="settings-panel">
				<div>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Library Scan</h2>
					<p class="text-soft text-[0.9rem]">Index local music files for browsing and playback</p>
				</div>
				<div class="flex gap-2"><Input bind:value={libraryPath} placeholder="/path/to/music" aria-label="Music folder path" class="flex-1" /><Button onclick={scanLibrary} disabled={loadingLibrary}>Scan</Button></div>
				{#if scanSummary}
					<div class="grid grid-cols-4 gap-2.5 max-xl:grid-cols-2">
						<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Scanned</span><strong class="text-xl break-words">{scanSummary.scanned_files}</strong></div>
						<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Indexed</span><strong class="text-xl break-words">{scanSummary.indexed_tracks}</strong></div>
						<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Skipped</span><strong class="text-xl break-words">{scanSummary.skipped_files}</strong></div>
						<div class="grid gap-1 border border-outline rounded-[20px] p-2.5 bg-surface-2/[0.42]"><span class="text-soft font-mono text-[0.78rem] uppercase">Root</span><strong class="font-mono text-[0.76rem] break-words">{scanSummary.root}</strong></div>
					</div>
				{/if}
			</section>
		</Tabs.Content>
	</Tabs.Root>
</section>


