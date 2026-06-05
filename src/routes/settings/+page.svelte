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
		ProviderLoginState
	} from '$lib/types';

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
	let spotifyScope =
		'playlist-read-private playlist-read-collaborative user-read-private user-read-email user-read-playback-state user-modify-playback-state streaming';
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
	let message = '';
	let error = '';

	onMount(() => {
		void loadAccount();
		void loadAudioOutputs();
		void loadPlaybackSettings();
		void loadServiceCapabilities();
		void loadProviderAccounts();
		void loadProviderLoginStates();
		void loadLastFmScrobbleStatus();
	});

	async function loadAccount() {
		error = '';
		try {
			account = await invoke<JellyfinAccount | null>('get_jellyfin_account');
			baseUrl = account?.base_url ?? '';
			userName = account?.user_name ?? '';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadAudioOutputs() {
		error = '';
		try {
			audioOutputs = await invoke<AudioOutputDevice[]>('list_audio_output_devices');
			selectedAudioOutput = audioOutputs.find((device) => device.selected)?.id ?? 'default';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function selectAudioOutput() {
		error = '';
		message = '';
		try {
			await invoke<PlaybackStatus>('set_audio_output_device', {
				device_id: selectedAudioOutput === 'default' ? null : selectedAudioOutput
			});
			await loadAudioOutputs();
			message = 'Audio output updated.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadPlaybackSettings() {
		error = '';
		try {
			const status = await invoke<PlaybackStatus>('get_playback_status');
			replayGainMode = status.replay_gain_mode;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadServiceCapabilities() {
		error = '';
		try {
			serviceCapabilities = await invoke<ProviderCapability[]>('list_service_capabilities');
			if (!credentialProviderOptions().some((provider) => provider.id === selectedProviderId)) {
				selectedProviderId = credentialProviderOptions()[0]?.id ?? 'spotify';
			}
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadProviderAccounts() {
		error = '';
		try {
			providerAccounts = await invoke<ProviderAccount[]>('list_provider_accounts');
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadProviderLoginStates() {
		error = '';
		try {
			providerLoginStates = await invoke<ProviderLoginState[]>('list_provider_login_states');
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function refreshProviderCredentialStatus() {
		await loadProviderAccounts();
		await loadProviderLoginStates();
		await loadLastFmScrobbleStatus();
	}

	async function loadLastFmScrobbleStatus() {
		error = '';
		try {
			lastFmScrobbleStatus = await invoke<LastFmScrobbleStatus>('get_lastfm_scrobble_status');
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function retryLastFmScrobbles() {
		error = '';
		message = '';
		try {
			lastFmScrobbleStatus = await invoke<LastFmScrobbleStatus>('retry_lastfm_scrobbles');
			message = 'Last.fm scrobble retry finished.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function setReplayGainMode() {
		error = '';
		message = '';
		try {
			const status = await invoke<PlaybackStatus>('set_replay_gain_mode', { mode: replayGainMode });
			replayGainMode = status.replay_gain_mode;
			message = 'ReplayGain mode updated.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	function selectedAudioOutputDescription() {
		const device = audioOutputs.find((device) => device.id === selectedAudioOutput);
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
		return value
			.split('_')
			.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
			.join(' ');
	}

	function yesNo(value: boolean) {
		return value ? 'Yes' : 'No';
	}

	function playbackLabel(provider: ProviderCapability) {
		if (provider.can_stream_full_tracks) return 'Full tracks';
		if (provider.can_stream_previews) return 'Previews';
		if (provider.can_link_out) return 'Link out';
		return 'No';
	}

	function notesPreview(provider: ProviderCapability) {
		return provider.notes.join(' ');
	}

	function credentialProviderOptions() {
		return serviceCapabilities.filter(
			(provider) => !['local', 'jellyfin', 'lrclib'].includes(provider.id)
		);
	}

	function accountForProvider(providerId: string) {
		return providerAccounts.find((account) => account.provider_id === providerId) ?? null;
	}

	function providerLoginStateRows() {
		const providerIds = new Set(credentialProviderOptions().map((provider) => provider.id));
		return providerLoginStates.filter((state) => providerIds.has(state.provider_id));
	}

	function providerName(providerId: string) {
		return serviceCapabilities.find((provider) => provider.id === providerId)?.name ?? providerId;
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
		const account = accountForProvider('lastfm');
		return Boolean(account?.has_api_key && account.has_api_secret && account.has_access_token);
	}

	function loginStateForProvider(providerId: string) {
		return providerLoginStates.find((state) => state.provider_id === providerId) ?? null;
	}

	function selectProviderCredentials(providerId: string) {
		selectedProviderId = providerId;
		message = `${providerName(providerId)} selected in Service Credentials.`;
		error = '';
	}

	function optionalCredential(value: string) {
		const trimmed = value.trim();
		return trimmed.length > 0 ? trimmed : null;
	}

	function authorizationCodeFromInput(value: string) {
		const trimmed = value.trim();
		if (!trimmed) return trimmed;
		return parameterFromAuthorizationInput(trimmed, 'code') ?? trimmed;
	}

	function authorizationStateFromInput(value: string, fallbackState: string) {
		return (
			parameterFromAuthorizationInput(value.trim(), 'state') ?? optionalCredential(fallbackState)
		);
	}

	function parameterFromAuthorizationInput(value: string, parameterName: string) {
		if (!value) return null;
		try {
			const url = new URL(value);
			const queryValue = url.searchParams.get(parameterName);
			if (queryValue) return queryValue;
			if (url.hash.startsWith('#')) {
				const hashValue = new URLSearchParams(url.hash.slice(1)).get(parameterName);
				if (hashValue) return hashValue;
			}
		} catch {
			return null;
		}
		return null;
	}

	function clearProviderFormSecrets() {
		providerClientId = '';
		providerClientSecret = '';
		providerApiKey = '';
		providerApiSecret = '';
		providerAccessToken = '';
		providerRefreshToken = '';
	}

	async function saveProviderAccount() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('save_provider_account', {
				provider_id: selectedProviderId,
				display_name: optionalCredential(providerDisplayName),
				client_id: optionalCredential(providerClientId),
				client_secret: optionalCredential(providerClientSecret),
				api_key: optionalCredential(providerApiKey),
				api_secret: optionalCredential(providerApiSecret),
				access_token: optionalCredential(providerAccessToken),
				refresh_token: optionalCredential(providerRefreshToken)
			});
			await loadProviderAccounts();
			await loadProviderLoginStates();
			providerDisplayName = '';
			clearProviderFormSecrets();
			message = `${providerName(selectedProviderId)} credentials saved to secure credential storage.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function clearProviderAccount() {
		error = '';
		message = '';
		try {
			await invoke('clear_provider_account', { provider_id: selectedProviderId });
			await loadProviderAccounts();
			await loadProviderLoginStates();
			providerDisplayName = '';
			clearProviderFormSecrets();
			message = `${providerName(selectedProviderId)} credentials cleared.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function startSpotifyLogin() {
		error = '';
		message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_spotify_pkce_login', {
				redirect_uri: spotifyRedirectUri,
				scope: optionalCredential(spotifyScope)
			});
			spotifyAuthorizationUrl = login.authorization_url;
			spotifyAuthorizationState = login.state ?? '';
			message = login.message;
			await loadProviderLoginStates();
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function finishSpotifyLogin() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('finish_spotify_pkce_login', {
				code: authorizationCodeFromInput(spotifyAuthorizationCode),
				state: authorizationStateFromInput(spotifyAuthorizationCode, spotifyAuthorizationState)
			});
			spotifyAuthorizationCode = '';
			spotifyAuthorizationUrl = '';
			await refreshProviderCredentialStatus();
			message = 'Spotify tokens saved to secure credential storage.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function completeSpotifyLoginInBrowser() {
		error = '';
		message = 'Waiting for Spotify authorization in the browser...';
		try {
			await invoke<ProviderAccount>('complete_spotify_pkce_login_in_browser', {
				redirect_uri: spotifyRedirectUri,
				scope: optionalCredential(spotifyScope)
			});
			spotifyAuthorizationCode = '';
			spotifyAuthorizationUrl = '';
			await refreshProviderCredentialStatus();
			message = 'Spotify login completed and playback tokens were saved.';
		} catch (err) {
			error = toErrorMessage(err);
			message = '';
		}
	}

	async function refreshSpotifyToken() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('refresh_spotify_access_token');
			await refreshProviderCredentialStatus();
			message = 'Spotify access token refreshed.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function startTidalLogin() {
		error = '';
		message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_tidal_pkce_login', {
				redirect_uri: tidalRedirectUri,
				scope: optionalCredential(tidalScope)
			});
			tidalAuthorizationUrl = login.authorization_url;
			tidalAuthorizationState = login.state ?? '';
			message = login.message;
			await loadProviderLoginStates();
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function finishTidalLogin() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('finish_tidal_pkce_login', {
				code: authorizationCodeFromInput(tidalAuthorizationCode),
				state: authorizationStateFromInput(tidalAuthorizationCode, tidalAuthorizationState)
			});
			tidalAuthorizationCode = '';
			tidalAuthorizationUrl = '';
			await refreshProviderCredentialStatus();
			message = 'TIDAL tokens saved to secure credential storage.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function refreshTidalToken() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('refresh_tidal_access_token');
			await refreshProviderCredentialStatus();
			message = 'TIDAL access token refreshed.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function startYoutubeLogin() {
		error = '';
		message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_youtube_oauth_login', {
				redirect_uri: youtubeRedirectUri,
				scope: optionalCredential(youtubeScope)
			});
			youtubeAuthorizationUrl = login.authorization_url;
			youtubeAuthorizationState = login.state ?? '';
			message = login.message;
			await loadProviderLoginStates();
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function finishYoutubeLogin() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('finish_youtube_oauth_login', {
				code: authorizationCodeFromInput(youtubeAuthorizationCode),
				state: authorizationStateFromInput(youtubeAuthorizationCode, youtubeAuthorizationState)
			});
			youtubeAuthorizationCode = '';
			youtubeAuthorizationUrl = '';
			await refreshProviderCredentialStatus();
			message = 'YouTube OAuth tokens saved to secure credential storage.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function refreshYoutubeToken() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('refresh_youtube_access_token');
			await refreshProviderCredentialStatus();
			message = 'YouTube access token refreshed.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function startLastFmLogin() {
		error = '';
		message = '';
		try {
			const login = await invoke<ProviderLoginStart>('start_lastfm_login');
			lastFmAuthorizationUrl = login.authorization_url;
			message = login.message;
			await loadProviderLoginStates();
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function finishLastFmLogin() {
		error = '';
		message = '';
		try {
			await invoke<ProviderAccount>('finish_lastfm_login');
			lastFmAuthorizationUrl = '';
			await refreshProviderCredentialStatus();
			message = 'Last.fm session key saved to secure credential storage.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function saveAccount() {
		error = '';
		message = '';
		try {
			account = await invoke<JellyfinAccount>('save_jellyfin_account', {
				base_url: baseUrl,
				user_name: userName,
				password
			});
			password = '';
			message = 'Jellyfin account saved to secure credential storage.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function clearAccount() {
		error = '';
		message = '';
		try {
			await invoke('clear_jellyfin_account');
			account = null;
			baseUrl = '';
			userName = '';
			password = '';
			message = 'Jellyfin account cleared.';
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	function toErrorMessage(err: unknown) {
		const message = typeof err === 'string' ? err : err instanceof Error ? err.message : null;
		if (message?.includes('__TAURI_INTERNALS__')) return '';
		if (message) return message;
		return 'Unexpected application error.';
	}
</script>

<section class="settings">
	<div class="heading">
		<h1>Settings</h1>
		<p>Accounts and library foundations</p>
	</div>

	{#if error}
		<p class="error">{error}</p>
	{/if}
	{#if message}
		<p class="message">{message}</p>
	{/if}

	<section class="panel">
		<div>
			<h2>Services</h2>
			<p>Provider capabilities and current implementation state</p>
		</div>

		<table class="service-table">
			<thead>
				<tr>
					<th>Service</th>
					<th>State</th>
					<th>Auth</th>
					<th>Search</th>
					<th>Playlists</th>
					<th>Playback</th>
					<th>Scrobble</th>
					<th>Notes</th>
				</tr>
			</thead>
			<tbody>
				{#each serviceCapabilities as provider}
					<tr>
						<td>
							<strong>{provider.name}</strong>
							{#if provider.documentation_url}
								<a href={provider.documentation_url} target="_blank" rel="noreferrer">Docs</a>
							{/if}
						</td>
						<td
							><span class={`state ${provider.integration_state}`}
								>{stateLabel(provider.integration_state)}</span
							></td
						>
						<td>{provider.auth_model}</td>
						<td>{yesNo(provider.can_search)}</td>
						<td>{yesNo(provider.can_list_playlists)}</td>
						<td>{playbackLabel(provider)}</td>
						<td>{yesNo(provider.can_scrobble)}</td>
						<td>{notesPreview(provider)}</td>
					</tr>
				{/each}
			</tbody>
		</table>
	</section>

	<section class="panel">
		<div>
			<h2>Service Credentials</h2>
			<p>Secure storage for provider OAuth/API material and user app credentials</p>
		</div>

		<div class="credential-grid">
			<label>
				Service
				<select bind:value={selectedProviderId}>
					{#each credentialProviderOptions() as provider}
						<option value={provider.id}>{provider.name}</option>
					{/each}
				</select>
			</label>
			<label>
				Account label
				<input bind:value={providerDisplayName} placeholder="Personal account" autocomplete="off" />
			</label>
			<label>
				Client ID / App ID
				<input bind:value={providerClientId} autocomplete="off" />
			</label>
			<label>
				Client secret / App secret
				<input bind:value={providerClientSecret} type="password" autocomplete="off" />
			</label>
			<label>
				API key
				<input bind:value={providerApiKey} autocomplete="off" />
			</label>
			<label>
				API secret
				<input bind:value={providerApiSecret} type="password" autocomplete="off" />
			</label>
			<label>
				Access token / Session key
				<input bind:value={providerAccessToken} type="password" autocomplete="off" />
			</label>
			<label>
				Refresh token
				<input bind:value={providerRefreshToken} type="password" autocomplete="off" />
			</label>
		</div>

		<div class="actions">
			<button onclick={saveProviderAccount}>Save provider credentials</button>
			<button onclick={clearProviderAccount}>Clear selected service</button>
			<button onclick={refreshProviderCredentialStatus}>Refresh status</button>
		</div>

		{#if providerLoginStateRows().length > 0}
			<table class="credential-table">
				<thead>
					<tr>
						<th>Service</th>
						<th>Login state</th>
						<th>Details</th>
						<th>Last failure</th>
					</tr>
				</thead>
				<tbody>
					{#each providerLoginStateRows() as loginState}
						<tr>
							<td>{providerName(loginState.provider_id)}</td>
							<td
								><span class={`state ${loginState.status}`}>{stateLabel(loginState.status)}</span
								></td
							>
							<td>{loginState.message}</td>
							<td class:error-cell={loginState.status === 'failed'}>
								{loginState.last_error ?? 'None'}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}

		<div class="login-grid">
			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>Spotify</h3>
						<p>OAuth PKCE</p>
					</div>
					<span class={`state ${loginStateForProvider('spotify')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('spotify')?.status ?? 'missing')}
					</span>
				</div>
				<label>
					Redirect URI
					<input bind:value={spotifyRedirectUri} autocomplete="off" />
				</label>
				<label>
					Scope
					<input bind:value={spotifyScope} autocomplete="off" />
				</label>
				<div class="actions">
					<button onclick={completeSpotifyLoginInBrowser}>Login in browser</button>
					<button onclick={startSpotifyLogin}>Manual login URL</button>
					<button onclick={refreshSpotifyToken}>Refresh Spotify token</button>
				</div>
				{#if spotifyAuthorizationUrl}
					<a class="auth-link" href={spotifyAuthorizationUrl} target="_blank" rel="noreferrer">
						Open Spotify authorization
					</a>
					<label>
						Returned URL or code
						<input bind:value={spotifyAuthorizationCode} autocomplete="off" />
					</label>
					<label>
						State
						<input bind:value={spotifyAuthorizationState} autocomplete="off" />
					</label>
					<div class="actions">
						<button onclick={finishSpotifyLogin}>Finish Spotify login</button>
					</div>
				{/if}
			</section>

			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>TIDAL</h3>
						<p>OAuth PKCE</p>
					</div>
					<span class={`state ${loginStateForProvider('tidal')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('tidal')?.status ?? 'missing')}
					</span>
				</div>
				<label>
					Redirect URI
					<input bind:value={tidalRedirectUri} autocomplete="off" />
				</label>
				<label>
					Scope
					<input bind:value={tidalScope} autocomplete="off" />
				</label>
				<div class="actions">
					<button onclick={startTidalLogin}>Start TIDAL login</button>
					<button onclick={refreshTidalToken}>Refresh TIDAL token</button>
				</div>
				{#if tidalAuthorizationUrl}
					<a class="auth-link" href={tidalAuthorizationUrl} target="_blank" rel="noreferrer">
						Open TIDAL authorization
					</a>
					<label>
						Returned URL or code
						<input bind:value={tidalAuthorizationCode} autocomplete="off" />
					</label>
					<label>
						State
						<input bind:value={tidalAuthorizationState} autocomplete="off" />
					</label>
					<div class="actions">
						<button onclick={finishTidalLogin}>Finish TIDAL login</button>
					</div>
				{/if}
			</section>

			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>YouTube</h3>
						<p>Google OAuth</p>
					</div>
					<span class={`state ${loginStateForProvider('youtube')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('youtube')?.status ?? 'missing')}
					</span>
				</div>
				<label>
					Redirect URI
					<input bind:value={youtubeRedirectUri} autocomplete="off" />
				</label>
				<label>
					Scope
					<input bind:value={youtubeScope} autocomplete="off" />
				</label>
				<div class="actions">
					<button onclick={startYoutubeLogin}>Start YouTube login</button>
					<button onclick={refreshYoutubeToken}>Refresh YouTube token</button>
				</div>
				{#if youtubeAuthorizationUrl}
					<a class="auth-link" href={youtubeAuthorizationUrl} target="_blank" rel="noreferrer">
						Open Google authorization
					</a>
					<label>
						Returned URL or code
						<input bind:value={youtubeAuthorizationCode} autocomplete="off" />
					</label>
					<label>
						State
						<input bind:value={youtubeAuthorizationState} autocomplete="off" />
					</label>
					<div class="actions">
						<button onclick={finishYoutubeLogin}>Finish YouTube login</button>
					</div>
				{/if}
			</section>

			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>Last.fm</h3>
						<p>Desktop session</p>
					</div>
					<span class={`state ${loginStateForProvider('lastfm')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('lastfm')?.status ?? 'missing')}
					</span>
				</div>
				<div class="actions">
					<button onclick={startLastFmLogin}>Start Last.fm login</button>
				</div>
				{#if lastFmAuthorizationUrl}
					<a class="auth-link" href={lastFmAuthorizationUrl} target="_blank" rel="noreferrer">
						Open Last.fm authorization
					</a>
					<div class="actions">
						<button onclick={finishLastFmLogin}>Finish Last.fm login</button>
					</div>
				{/if}
			</section>

			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>Qobuz</h3>
						<p>App credentials</p>
					</div>
					<span class={`state ${loginStateForProvider('qobuz')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('qobuz')?.status ?? 'missing')}
					</span>
				</div>
				<p>{loginStateForProvider('qobuz')?.message ?? 'No Qobuz credentials saved'}</p>
				<div class="actions">
					<button onclick={() => selectProviderCredentials('qobuz')}>Edit Qobuz credentials</button>
				</div>
			</section>

			<section class="login-panel">
				<div class="login-panel-header">
					<div>
						<h3>Bandcamp</h3>
						<p>Link-out</p>
					</div>
					<span class={`state ${loginStateForProvider('bandcamp')?.status ?? 'missing'}`}>
						{stateLabel(loginStateForProvider('bandcamp')?.status ?? 'missing')}
					</span>
				</div>
				<p>{loginStateForProvider('bandcamp')?.message ?? 'No Bandcamp login state available'}</p>
				<div class="actions">
					<button onclick={() => selectProviderCredentials('bandcamp')}>Edit Bandcamp note</button>
				</div>
				<a class="auth-link" href="https://bandcamp.com/developer" target="_blank" rel="noreferrer">
					Open Bandcamp developer docs
				</a>
			</section>
		</div>

		{#if providerAccounts.length > 0}
			<table class="credential-table">
				<thead>
					<tr>
						<th>Service</th>
						<th>Saved fields</th>
						<th>Source</th>
					</tr>
				</thead>
				<tbody>
					{#each providerAccounts as providerAccount}
						<tr>
							<td>{providerName(providerAccount.provider_id)}</td>
							<td>{savedProviderFlags(providerAccount)}</td>
							<td>{providerAccount.source}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p>No provider credentials saved</p>
		{/if}
	</section>

	<section class="panel">
		<div>
			<h2>Last.fm Scrobbling</h2>
			<p>
				{lastFmCredentialsReady()
					? 'Last.fm scrobbling credentials are ready'
					: 'Save a Last.fm API key, API secret, and session key in Service Credentials'}
			</p>
		</div>

		<div class="scrobble-grid">
			<div>
				<span>Pending</span>
				<strong>{lastFmScrobbleStatus?.pending_count ?? 0}</strong>
			</div>
			<div>
				<span>Submitted</span>
				<strong>{lastFmScrobbleStatus?.submitted_count ?? 0}</strong>
			</div>
			<div>
				<span>Failed</span>
				<strong>{lastFmScrobbleStatus?.failed_count ?? 0}</strong>
			</div>
		</div>

		{#if lastFmScrobbleStatus?.last_error}
			<p class="inline-error">{lastFmScrobbleStatus.last_error}</p>
		{/if}

		<div class="actions">
			<button onclick={retryLastFmScrobbles} disabled={!lastFmCredentialsReady()}>
				Retry pending scrobbles
			</button>
			<button onclick={loadLastFmScrobbleStatus}>Refresh status</button>
		</div>
	</section>

	<section class="panel">
		<div>
			<h2>Jellyfin</h2>
			<p>
				{#if account}
					Stored from {account.source}; password present: {account.has_password ? 'yes' : 'no'}
				{:else}
					No Jellyfin account saved
				{/if}
			</p>
		</div>

		<label>
			Server URL
			<input bind:value={baseUrl} placeholder="https://jellyfin.example" />
		</label>
		<label>
			Username
			<input bind:value={userName} autocomplete="username" />
		</label>
		<label>
			Password
			<input bind:value={password} type="password" autocomplete="current-password" />
		</label>

		<div class="actions">
			<button onclick={saveAccount}>Save</button>
			<button onclick={clearAccount}>Clear</button>
		</div>
	</section>

	<section class="panel">
		<div>
			<h2>Audio Output</h2>
			<p>{selectedAudioOutputDescription()}</p>
		</div>

		<label>
			Output device
			<select bind:value={selectedAudioOutput} onchange={selectAudioOutput}>
				{#each audioOutputs as device}
					<option value={device.id}>{device.name}</option>
				{/each}
			</select>
		</label>

		<div class="actions">
			<button onclick={loadAudioOutputs}>Refresh devices</button>
		</div>
	</section>

	<section class="panel">
		<div>
			<h2>ReplayGain</h2>
			<p>Applied during local playback when matching ReplayGain tags are present</p>
		</div>

		<label>
			Mode
			<select bind:value={replayGainMode} onchange={setReplayGainMode}>
				<option value="off">Off</option>
				<option value="track">Track</option>
				<option value="album">Album</option>
			</select>
		</label>
	</section>
</section>

<style>
	.settings {
		display: grid;
		gap: 16px;
		max-width: 1280px;
	}

	.heading {
		position: relative;
		overflow: hidden;
		min-height: 210px;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background:
			linear-gradient(
				145deg,
				color-mix(in oklch, var(--surface) 92%, transparent),
				color-mix(in oklch, var(--surface-2) 58%, transparent)
			);
		box-shadow: var(--shadow);
		padding: clamp(22px, 5vw, 52px);
	}

	h1,
	h2,
	p {
		margin: 0;
	}

	h1 {
		font-family: var(--font-display);
		font-size: clamp(42px, 6vw, 76px);
		line-height: 0.94;
	}

	h2 {
		font-family: var(--font-display);
		font-size: clamp(22px, 2vw, 30px);
		line-height: 1.04;
	}

	h3 {
		margin: 0;
		font-size: 0.9rem;
	}

	.heading p,
	.panel p {
		color: var(--muted);
		font-size: 0.9rem;
	}

	.panel {
		display: grid;
		gap: 14px;
		padding: 18px;
		background: color-mix(in oklch, var(--surface) 90%, transparent);
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
	}

	label {
		display: grid;
		gap: 6px;
		color: var(--muted);
		font-size: 0.86rem;
	}

	input,
	select {
		width: 100%;
		box-sizing: border-box;
		border: 1px solid var(--border);
		border-radius: 999px;
		background: color-mix(in oklch, var(--surface) 88%, transparent);
		color: var(--fg);
		padding: 0.55rem 0.65rem;
	}

	.service-table {
		display: block;
		width: 100%;
		overflow-x: auto;
		border-collapse: collapse;
	}

	.service-table thead,
	.service-table tbody {
		display: table;
		width: 100%;
		min-width: 920px;
	}

	.service-table th,
	.service-table td,
	.credential-table th,
	.credential-table td {
		border-bottom: 1px solid var(--border);
		padding: 0.55rem 0.6rem;
		text-align: left;
		vertical-align: top;
	}

	.service-table th,
	.credential-table th {
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.76rem;
		text-transform: uppercase;
	}

	.service-table td,
	.credential-table td {
		font-size: 0.82rem;
	}

	.service-table td:first-child {
		min-width: 130px;
	}

	.service-table td:last-child {
		min-width: 260px;
		color: var(--muted);
	}

	.service-table a {
		display: block;
		margin-top: 2px;
		color: var(--accent);
		font-size: 0.76rem;
	}

	.state {
		display: inline-block;
		border-radius: 999px;
		border: 1px solid var(--border);
		background: color-mix(in oklch, var(--surface-2) 70%, transparent);
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.72rem;
		padding: 0.14rem 0.45rem;
		white-space: nowrap;
	}

	.state.implemented {
		border-color: color-mix(in oklch, var(--success) 44%, var(--border));
		color: var(--success);
	}

	.state.partial {
		border-color: color-mix(in oklch, var(--accent) 44%, var(--border));
		color: var(--accent);
	}

	.state.ready,
	.state.link_out_only {
		border-color: color-mix(in oklch, var(--success) 44%, var(--border));
		color: var(--success);
	}

	.state.missing {
		color: var(--muted);
	}

	.state.failed {
		border-color: color-mix(in oklch, var(--danger) 48%, var(--border));
		color: var(--danger);
	}

	.state.researched {
		border-color: color-mix(in oklch, var(--accent-2) 48%, var(--border));
		color: var(--accent-2);
	}

	.credential-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px;
	}

	.credential-table {
		width: 100%;
		border-collapse: collapse;
	}

	.login-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 12px;
	}

	.login-panel {
		display: grid;
		gap: 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 12px;
	}

	.login-panel-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 10px;
	}

	.login-panel-header p {
		margin-top: 2px;
	}

	.auth-link {
		color: var(--accent);
		font-size: 0.86rem;
	}

	.scrobble-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 10px;
	}

	.scrobble-grid div {
		display: grid;
		gap: 4px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 10px;
	}

	.scrobble-grid span {
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.78rem;
		text-transform: uppercase;
	}

	.scrobble-grid strong {
		font-size: 1.2rem;
	}

	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.error,
	.message {
		padding: 0.65rem 0.8rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
	}

	.error {
		background: color-mix(in oklch, var(--danger) 20%, var(--surface));
		color: color-mix(in oklch, var(--danger) 72%, var(--fg));
	}

	.inline-error {
		color: var(--danger);
	}

	.error-cell {
		color: var(--danger);
	}

	.message {
		background: color-mix(in oklch, var(--success) 18%, var(--surface));
		color: color-mix(in oklch, var(--success) 82%, var(--fg));
	}

	@media (max-width: 1180px) {
		.settings {
			gap: 14px;
		}

		.heading {
			min-height: 190px;
		}

		.service-table thead,
		.service-table tbody {
			min-width: 760px;
		}

		.credential-grid,
		.login-grid {
			grid-template-columns: 1fr;
		}
	}

	@media (max-width: 880px) {
		.settings {
			gap: 12px;
		}

		.heading {
			min-height: 160px;
			padding: 22px;
		}

		h1 {
			font-size: clamp(38px, 12vw, 58px);
		}

		.panel {
			padding: 14px;
		}

		.scrobble-grid {
			grid-template-columns: 1fr;
		}

		.service-table thead,
		.service-table tbody {
			min-width: 680px;
		}

		.login-panel-header {
			display: grid;
		}

		.actions button {
			width: 100%;
		}
	}
</style>
