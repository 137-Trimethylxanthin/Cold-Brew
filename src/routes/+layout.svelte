<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { PlaybackStatus, QueuePlaybackResult, QueueSnapshot, Song } from '$lib/types';

	let currentSong: Song = {
		title: 'Nothing playing',
		artist: '',
		album: '',
		duration: 0,
		id: ''
	};
	let oldSongs: Song[] = [];
	let upcomingSongs: Song[] = [];
	let playbackStatus: PlaybackStatus | null = null;
	let nativePlaybackActive = false;
	let draggedUpcomingIndex: number | null = null;
	let dragOverUpcomingIndex: number | null = null;
	let volume = 1;
	let playerError = '';
	let spotifySdkPromise: Promise<void> | null = null;
	let spotifyPlayer: SpotifyPlayer | null = null;
	let spotifyDeviceId: string | null = null;
	let spotifyReady = false;
	let spotifyConnecting: Promise<string> | null = null;
	let spotifyResolveDevice: ((deviceId: string) => void) | null = null;
	let spotifyRejectDevice: ((error: Error) => void) | null = null;
	let spotifyPlaybackActive = false;
	let spotifyPaused = true;
	let spotifyStoppedByApp = false;
	let spotifyPositionMs = 0;
	let spotifyDurationMs: number | null = null;
	let spotifyStateUpdatedAt = 0;
	let spotifyLastQueueSyncId: string | null = null;
	const spectrumBars = [
		64, 36, 86, 48, 74, 42, 92, 58, 30, 80, 54, 70, 44, 96, 62, 34, 76, 50, 88, 46,
		68, 38, 82, 56, 72, 40, 90, 52
	] as const;

	onMount(() => {
		void refreshQueue();
		void refreshPlaybackStatus();
		const refreshTimer = window.setInterval(() => {
			void refreshQueue();
			void refreshPlaybackStatus();
			void refreshSpotifyState();
		}, 1000);
		const onKeyDown = (event: KeyboardEvent) => {
			if (isTypingTarget(event.target)) return;
			if (event.code === 'Space') {
				event.preventDefault();
				if (playbackStatus?.playing || spotifyIsPlaying()) {
					void pausePlayback();
				} else if (canStartPlayback()) {
					void resumePlayback();
				}
			}
			if (event.code === 'ArrowLeft') {
				event.preventDefault();
				void playPreviousQueueSong();
			}
			if (event.code === 'ArrowRight') {
				event.preventDefault();
				void playNextQueueSong();
			}
		};
		window.addEventListener('keydown', onKeyDown);

		return () => {
			window.clearInterval(refreshTimer);
			window.removeEventListener('keydown', onKeyDown);
			spotifyPlayer?.disconnect();
		};
	});

	async function refreshPlaybackStatus() {
		try {
			syncPlaybackStatus(await invoke<PlaybackStatus>('get_playback_status'));
		} catch {
			// Tauri commands are unavailable during browser-only development.
		}
	}

	async function refreshSpotifyState() {
		if (!spotifyPlayer || !spotifyPlaybackActive) return;
		try {
			const state = await spotifyPlayer.getCurrentState();
			if (state) syncSpotifyState(state);
		} catch (error) {
			playerError = toErrorMessage(error);
		}
	}

	async function refreshQueue() {
		try {
			syncQueueSnapshot(await invoke<QueueSnapshot>('get_queue_snapshot'));
		} catch {
			// Tauri commands are unavailable during browser-only development.
		}
	}

	function syncQueueSnapshot(queue: QueueSnapshot) {
		oldSongs = queue.old;
		upcomingSongs = queue.upcoming;

		if (!nativePlaybackActive && queue.current_song) {
			currentSong = queue.current_song;
		} else if (!nativePlaybackActive && !queue.current_song && !spotifyPlaybackActive) {
			currentSong = emptySong();
		}
	}

	async function syncQueuePlaybackResult(result: QueuePlaybackResult) {
		syncQueueSnapshot(result.queue);
		if (result.playback_status) {
			if (spotifyPlaybackActive) await pauseSpotifyPlayback(false);
			spotifyPlaybackActive = false;
			syncPlaybackStatus(result.playback_status);
			playerError = '';
			return;
		}

		const song = result.queue.current_song;
		if (song?.source === 'spotify') {
			try {
				await playSpotifySong(song, result.queue);
			} catch (error) {
				playerError = toErrorMessage(error);
			}
			return;
		}

		if (result.message) {
			playerError = result.message;
		}
	}

	async function removeQueuedSong(song: Song) {
		syncQueueSnapshot(await invoke<QueueSnapshot>('remove_queued_song', { song }));
	}

	async function moveQueuedSong(fromIndex: number, toIndex: number) {
		if (fromIndex === toIndex) return;
		syncQueueSnapshot(
			await invoke<QueueSnapshot>('move_queued_song', {
				from_index: fromIndex,
				to_index: toIndex
			})
		);
	}

	function startQueueDrag(event: DragEvent, index: number) {
		draggedUpcomingIndex = index;
		dragOverUpcomingIndex = index;
		if (event.dataTransfer) {
			event.dataTransfer.effectAllowed = 'move';
			event.dataTransfer.setData('text/plain', String(index));
		}
	}

	function allowQueueDrop(event: DragEvent, index: number) {
		event.preventDefault();
		dragOverUpcomingIndex = index;
		if (event.dataTransfer) {
			event.dataTransfer.dropEffect = 'move';
		}
	}

	async function dropQueuedSong(event: DragEvent, toIndex: number) {
		event.preventDefault();
		const transferIndex = Number(event.dataTransfer?.getData('text/plain'));
		const fromIndex =
			draggedUpcomingIndex ?? (Number.isInteger(transferIndex) ? transferIndex : null);
		endQueueDrag();
		if (fromIndex === null) return;
		await moveQueuedSong(fromIndex, toIndex);
	}

	function leaveQueueDrop(index: number) {
		if (dragOverUpcomingIndex === index) {
			dragOverUpcomingIndex = null;
		}
	}

	function endQueueDrag() {
		draggedUpcomingIndex = null;
		dragOverUpcomingIndex = null;
	}

	function syncPlaybackStatus(status: PlaybackStatus) {
		playbackStatus = status;
		volume = status.volume;

		if (status.current_path) {
			if (spotifyPlaybackActive) void pauseSpotifyPlayback(false);
			spotifyPlaybackActive = false;
			nativePlaybackActive = true;
			currentSong = {
				id: status.current_path,
				title: status.current_title ?? titleFromPath(status.current_path),
				artist: playbackQualityLabel(status),
				album: 'Local file',
				duration: Math.round((status.duration_ms ?? 0) * 10000),
				source: 'local',
				uri: status.current_path,
				quality: playbackQualityLabel(status),
				playable: true
			};
		} else if (nativePlaybackActive) {
			nativePlaybackActive = false;
			if (!spotifyPlaybackActive) currentSong = emptySong();
		}
	}

	async function pausePlayback() {
		if (spotifyIsPlaying()) {
			await pauseSpotifyPlayback(true);
			return;
		}
		syncPlaybackStatus(await invoke<PlaybackStatus>('playback_pause'));
	}

	async function resumePlayback() {
		if (spotifyPlaybackActive && spotifyPaused) {
			if (spotifyPlayer && spotifyReady) {
				await spotifyPlayer.resume();
			} else {
				await spotifyApiFetch('/me/player/play', { method: 'PUT' });
			}
			spotifyPaused = false;
			spotifyStateUpdatedAt = Date.now();
			playerError = '';
			return;
		}
		if (playbackStatus?.current_path) {
			syncPlaybackStatus(await invoke<PlaybackStatus>('playback_resume'));
			return;
		}
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_current_queue_song'));
	}

	async function stopPlayback() {
		if (spotifyPlaybackActive) {
			await pauseSpotifyPlayback(false);
			if (spotifyPlayer && spotifyReady) {
				await spotifyPlayer.seek(0);
			} else {
				await spotifyApiFetch('/me/player/seek?position_ms=0', { method: 'PUT' });
			}
			spotifyPlaybackActive = false;
			spotifyPaused = true;
			spotifyPositionMs = 0;
			if (!nativePlaybackActive) currentSong = emptySong();
			return;
		}
		syncPlaybackStatus(await invoke<PlaybackStatus>('playback_stop'));
	}

	async function updateVolume(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		volume = Number(input.value);
		if (spotifyPlaybackActive) {
			if (spotifyPlayer && spotifyReady) {
				await spotifyPlayer.setVolume(volume);
			} else {
				await spotifyApiFetch(`/me/player/volume?volume_percent=${Math.round(volume * 100)}`, {
					method: 'PUT'
				});
			}
			return;
		}
		syncPlaybackStatus(await invoke<PlaybackStatus>('set_playback_volume', { volume }));
	}

	async function playPreviousQueueSong() {
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_previous_queue_song'));
	}

	async function playNextQueueSong() {
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_next_queue_song'));
	}

	async function playSpotifySong(song: Song, queue: QueueSnapshot) {
		if (!song.uri?.startsWith('spotify:')) {
			playerError = 'Queued Spotify track is missing a Spotify URI.';
			return;
		}
		if (song.playable === false) {
			playerError = 'This Spotify track is not playable for the current account or market.';
			return;
		}

		if (playbackStatus?.current_path) {
			syncPlaybackStatus(await invoke<PlaybackStatus>('playback_stop'));
		}

		const deviceId = await spotifyPlaybackDeviceId();
		const playPath = deviceId
			? `/me/player/play?device_id=${encodeURIComponent(deviceId)}`
			: '/me/player/play';
		await spotifyApiFetch(playPath, {
			method: 'PUT',
			body: JSON.stringify({ uris: spotifyQueueUris(queue), position_ms: 0 })
		});
		currentSong = song;
		spotifyPlaybackActive = true;
		spotifyPaused = false;
		spotifyStoppedByApp = false;
		spotifyLastQueueSyncId = song.id;
		spotifyPositionMs = 0;
		spotifyDurationMs = song.duration ? Math.round(song.duration / 10000) : null;
		spotifyStateUpdatedAt = Date.now();
		playerError = '';
	}

	async function spotifyPlaybackDeviceId() {
		try {
			const deviceId = await ensureSpotifyDevice();
			await spotifyPlayer?.activateElement?.();
			await spotifyApiFetch('/me/player', {
				method: 'PUT',
				body: JSON.stringify({ device_ids: [deviceId], play: false })
			});
			return deviceId;
		} catch (error) {
			playerError = `${toErrorMessage(error)} Falling back to the active Spotify device.`;
			return null;
		}
	}

	function spotifyQueueUris(queue: QueueSnapshot) {
		const songs = [queue.current_song, ...queue.upcoming].filter((song): song is Song =>
			Boolean(song)
		);
		const uris: string[] = [];
		for (const song of songs) {
			if (
				song.source !== 'spotify' ||
				song.playable === false ||
				!song.uri?.startsWith('spotify:')
			) {
				break;
			}
			uris.push(song.uri);
		}
		return uris.length > 0 ? uris : queue.current_song?.uri ? [queue.current_song.uri] : [];
	}

	async function ensureSpotifyDevice() {
		if (spotifyReady && spotifyDeviceId) return spotifyDeviceId;
		if (spotifyConnecting) return spotifyConnecting;

		await loadSpotifySdk();
		const player = createSpotifyPlayer();
		spotifyConnecting = new Promise<string>((resolve, reject) => {
			const timeoutId = window.setTimeout(() => {
				spotifyResolveDevice = null;
				spotifyRejectDevice = null;
				reject(new Error('Spotify Web Playback device did not become ready in time.'));
			}, 15000);
			spotifyResolveDevice = (deviceId) => {
				window.clearTimeout(timeoutId);
				spotifyResolveDevice = null;
				spotifyRejectDevice = null;
				resolve(deviceId);
			};
			spotifyRejectDevice = (error) => {
				window.clearTimeout(timeoutId);
				spotifyResolveDevice = null;
				spotifyRejectDevice = null;
				reject(error);
			};
			void player.connect().then((connected) => {
				if (!connected) {
					spotifyRejectDevice?.(new Error('Spotify Web Playback SDK refused the connection.'));
				}
			});
		}).finally(() => {
			spotifyConnecting = null;
		});

		return spotifyConnecting;
	}

	function createSpotifyPlayer() {
		if (spotifyPlayer) return spotifyPlayer;
		if (!window.Spotify?.Player) {
			throw new Error('Spotify Web Playback SDK did not load.');
		}

		const player = new window.Spotify.Player({
			name: 'Cold-Brew',
			volume,
			getOAuthToken: (callback) => {
				void getSpotifyToken()
					.then(callback)
					.catch((error) => {
						playerError = toErrorMessage(error);
					});
			}
		});

		player.addListener('ready', ({ device_id }) => {
			spotifyDeviceId = device_id;
			spotifyReady = true;
			spotifyResolveDevice?.(device_id);
		});
		player.addListener('not_ready', ({ device_id }) => {
			if (spotifyDeviceId === device_id) {
				spotifyReady = false;
				spotifyDeviceId = null;
			}
		});
		player.addListener('player_state_changed', (state) => {
			if (state) syncSpotifyState(state);
		});
		for (const event of [
			'initialization_error',
			'authentication_error',
			'account_error',
			'playback_error'
		] as const) {
			player.addListener(event, (error) => {
				playerError = `Spotify ${event.replaceAll('_', ' ')}: ${error.message}`;
				spotifyRejectDevice?.(new Error(playerError));
				if (event === 'authentication_error') void refreshSpotifyToken();
			});
		}

		spotifyPlayer = player;
		return player;
	}

	function loadSpotifySdk() {
		if (window.Spotify?.Player) return Promise.resolve();
		if (spotifySdkPromise) return spotifySdkPromise;

		spotifySdkPromise = new Promise<void>((resolve, reject) => {
			const existingScript = document.querySelector<HTMLScriptElement>(
				'script[src="https://sdk.scdn.co/spotify-player.js"]'
			);
			window.onSpotifyWebPlaybackSDKReady = () => resolve();
			if (existingScript) return;
			const script = document.createElement('script');
			script.src = 'https://sdk.scdn.co/spotify-player.js';
			script.async = true;
			script.onerror = () => reject(new Error('Could not load Spotify Web Playback SDK.'));
			document.body.appendChild(script);
		});

		return spotifySdkPromise;
	}

	async function spotifyApiFetch(path: string, options: RequestInit) {
		let token = await getSpotifyToken();
		let response = await fetch(`https://api.spotify.com/v1${path}`, spotifyRequest(options, token));
		if (response.status === 401) {
			await refreshSpotifyToken();
			token = await getSpotifyToken();
			response = await fetch(`https://api.spotify.com/v1${path}`, spotifyRequest(options, token));
		}
		if (!response.ok) {
			const body = await response.text();
			throw new Error(`Spotify playback request failed with HTTP ${response.status}: ${body}`);
		}
		return response;
	}

	function spotifyRequest(options: RequestInit, token: string): RequestInit {
		const headers = new Headers(options.headers);
		headers.set('Content-Type', 'application/json');
		headers.set('Authorization', `Bearer ${token}`);
		return {
			...options,
			headers
		};
	}

	async function getSpotifyToken() {
		return invoke<string>('get_spotify_web_playback_token');
	}

	async function refreshSpotifyToken() {
		await invoke('refresh_spotify_access_token');
	}

	async function pauseSpotifyPlayback(keepActive: boolean) {
		if (spotifyPlayer && spotifyReady) {
			await spotifyPlayer.pause();
		} else {
			await spotifyApiFetch('/me/player/pause', { method: 'PUT' });
		}
		spotifyPaused = true;
		spotifyStateUpdatedAt = Date.now();
		if (!keepActive) {
			spotifyStoppedByApp = true;
			spotifyPlaybackActive = false;
		}
	}

	function syncSpotifyState(state: SpotifyWebPlaybackState) {
		if (nativePlaybackActive) {
			spotifyPaused = state.paused;
			return;
		}
		if (spotifyStoppedByApp && state.paused) {
			spotifyPaused = true;
			spotifyPositionMs = state.position;
			spotifyDurationMs = state.duration;
			spotifyStateUpdatedAt = Date.now();
			return;
		}
		spotifyStoppedByApp = false;
		spotifyPlaybackActive = true;
		spotifyPaused = state.paused;
		spotifyPositionMs = state.position;
		spotifyDurationMs = state.duration;
		spotifyStateUpdatedAt = Date.now();
		if (!nativePlaybackActive && state.track_window.current_track?.uri) {
			const queueId = spotifyQueueIdFromUri(state.track_window.current_track.uri);
			if (queueId !== spotifyLastQueueSyncId) {
				spotifyLastQueueSyncId = queueId;
				void invoke<QueueSnapshot>('advance_queue_to_song_id', { song_id: queueId }).then(
					syncQueueSnapshot
				);
			}
			currentSong = {
				id: queueId,
				title: state.track_window.current_track.name,
				artist: state.track_window.current_track.artists.map((artist) => artist.name).join(', '),
				album: state.track_window.current_track.album.name,
				duration: Math.round(state.duration * 10000),
				source: 'spotify',
				uri: state.track_window.current_track.uri,
				quality: 'Spotify Connect',
				playable: true
			};
		}
	}

	function spotifyQueueIdFromUri(uri: string) {
		const parts = uri.split(':');
		if (parts.length === 3 && parts[0] === 'spotify' && parts[1] === 'track') {
			return `spotify:${parts[2]}`;
		}
		return uri;
	}

	function spotifyIsPlaying() {
		return spotifyPlaybackActive && !spotifyPaused;
	}

	function currentSpotifyPositionMs() {
		if (!spotifyPlaybackActive) return 0;
		if (spotifyPaused) return spotifyPositionMs;
		const elapsedMs = Date.now() - spotifyStateUpdatedAt;
		return Math.min(
			spotifyDurationMs ?? spotifyPositionMs + elapsedMs,
			spotifyPositionMs + elapsedMs
		);
	}

	function durationLabel(duration: number) {
		if (!duration) return '0:00';
		const totalSeconds = Math.floor(duration / 10000000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return `${minutes}:${seconds.toString().padStart(2, '0')}`;
	}

	function formatMilliseconds(durationMs: number) {
		const totalSeconds = Math.floor(durationMs / 1000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return `${minutes}:${seconds.toString().padStart(2, '0')}`;
	}

	function playbackTimeLabel() {
		if (spotifyPlaybackActive && spotifyDurationMs) {
			return `${formatMilliseconds(currentSpotifyPositionMs())} / ${formatMilliseconds(
				spotifyDurationMs
			)}`;
		}
		if (!playbackStatus?.current_path) return durationLabel(currentSong.duration);
		if (!playbackStatus.duration_ms) return formatMilliseconds(playbackStatus.position_ms);
		return `${formatMilliseconds(playbackStatus.position_ms)} / ${formatMilliseconds(
			playbackStatus.duration_ms
		)}`;
	}

	function playbackProgress() {
		if (spotifyPlaybackActive && spotifyDurationMs) {
			return Math.min(100, (currentSpotifyPositionMs() / spotifyDurationMs) * 100);
		}
		if (!playbackStatus?.current_path || !playbackStatus.duration_ms) return 0;
		return Math.min(100, (playbackStatus.position_ms / playbackStatus.duration_ms) * 100);
	}

	function canStartPlayback() {
		return Boolean(
			spotifyPlaybackActive ||
			playbackStatus?.current_path ||
			currentSong.id ||
			upcomingSongs.length > 0
		);
	}

	function playbackQualityLabel(status: PlaybackStatus) {
		const parts: string[] = [];
		if (status.source_format) parts.push(status.source_format);
		if (status.source_is_lossless !== null)
			parts.push(status.source_is_lossless ? 'lossless' : 'lossy');
		if (status.source_sample_rate) parts.push(formatSampleRate(status.source_sample_rate));
		if (status.source_channels) parts.push(`${status.source_channels} ch`);
		if (status.output_sample_rate) parts.push(`out ${formatSampleRate(status.output_sample_rate)}`);
		if (status.replay_gain_db !== null) {
			parts.push(
				`RG ${status.replay_gain_source ?? status.replay_gain_mode} ${formatDb(status.replay_gain_db)}`
			);
		}
		if (status.quality_warnings[0]) parts.push(status.quality_warnings[0]);
		return parts.join(' / ') || 'Local file';
	}

	function nowPlayingDetail() {
		if (playerError) return playerError;
		if (playbackStatus?.current_path) return playbackQualityLabel(playbackStatus);
		if (spotifyPlaybackActive) return spotifyPaused ? 'Spotify paused' : 'Spotify playing';
		return (
			songDetailLabel(currentSong) || currentSong.artist || currentSong.album || 'No active track'
		);
	}

	function queuedSongDetail(song: Song) {
		return songDetailLabel(song) || song.artist || song.album;
	}

	function songDetailLabel(song: Song) {
		const parts: string[] = [];
		if (song.source) parts.push(formatSource(song.source));
		if (song.quality) parts.push(song.quality);
		if (song.playable === false) parts.push('metadata only');
		if (song.external_url) parts.push('link out');
		return uniqueParts(parts).join(' / ');
	}

	function formatSource(source: string) {
		if (source === 'lastfm') return 'Last.fm';
		if (source === 'qobuz') return 'Qobuz';
		if (source === 'tidal') return 'TIDAL';
		if (source === 'youtube') return 'YouTube';
		if (source === 'spotify') return 'Spotify';
		if (source === 'jellyfin') return 'Jellyfin';
		if (source === 'local') return 'Local';
		return source;
	}

	function uniqueParts(parts: string[]) {
		return [...new Set(parts.filter(Boolean))];
	}

	function formatSampleRate(sampleRate: number) {
		const value = sampleRate / 1000;
		return `${Number.isInteger(value) ? value : value.toFixed(1)} kHz`;
	}

	function formatDb(value: number) {
		return `${value > 0 ? '+' : ''}${value.toFixed(1)} dB`;
	}

	function titleFromPath(path: string) {
		const fileName = path.split(/[\\/]/).pop() ?? 'Untitled';
		return fileName.replace(/\.[^.]+$/, '') || 'Untitled';
	}

	function emptySong(): Song {
		return {
			title: 'Nothing playing',
			artist: '',
			album: '',
			duration: 0,
			id: ''
		};
	}

	function toErrorMessage(error: unknown) {
		if (typeof error === 'string') return error;
		if (error instanceof Error) return error.message;
		return 'Unexpected playback error.';
	}

	function isTypingTarget(target: EventTarget | null) {
		if (!(target instanceof HTMLElement)) return false;
		return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) || target.isContentEditable;
	}
</script>

<div class={`app-shell ${$page.url.pathname === '/player' ? 'player-shell' : ''}`}>
	<nav class="sidenav" aria-label="Primary">
		<div class="brand-mark">
			<span class="brand-dot" aria-hidden="true"></span>
			<span>Cold Brew</span>
		</div>
		<p class="rail-kicker">Audiophile Player</p>
		<div class="rail-nav">
			<button onclick={() => goto('/')}>Library</button>
			<button onclick={() => goto('/player')}>Player</button>
			<button onclick={() => goto('/settings')}>Audio</button>
		</div>
		<section class="rail-status">
			<p class="eyebrow">Output</p>
			<strong>{playbackStatus?.output_device_name ?? 'Default device'}</strong>
			<span>
				{playbackStatus?.output_sample_rate
					? `${formatSampleRate(playbackStatus.output_sample_rate)} output`
					: 'Waiting for playback'}
			</span>
		</section>
	</nav>

	<main class="content">
		<slot />
	</main>

	<aside class="queue">
		<section class="desktop-player">
			<div class="cover-art" aria-hidden="true"></div>
			<div class="track-title">
				<h2>{currentSong.title}</h2>
				<p>{nowPlayingDetail()}</p>
			</div>
			<div class="quality-row">
				<span class="quality-pill">{formatSource(currentSong.source ?? 'local')}</span>
				{#if currentSong.quality}
					<span class="quality-pill hires">{currentSong.quality}</span>
				{/if}
			</div>
			<div class="spectrum" aria-hidden="true">
				{#each spectrumBars as height}
					<span style={`--bar-height: ${height}%`}></span>
				{/each}
			</div>
			<div class="progress-block">
				<div class="durationBar" style={`--progress: ${playbackProgress()}%`} aria-hidden="true"></div>
				<div class="progress-labels">
					<span>{playbackTimeLabel()}</span>
					<span>{playbackStatus?.state ?? (spotifyPlaybackActive ? 'spotify' : 'idle')}</span>
				</div>
			</div>
		</section>

		<section class="queue-panel">
			<h3>Up next</h3>
			{#if upcomingSongs.length === 0}
				<p>Queue is empty</p>
			{:else}
				<ol>
					{#each upcomingSongs as song, index}
						<li
							class={`queue-item ${draggedUpcomingIndex === index ? 'dragging' : ''} ${
								dragOverUpcomingIndex === index && draggedUpcomingIndex !== index ? 'drag-over' : ''
							}`}
							draggable="true"
							ondragstart={(event) => startQueueDrag(event, index)}
							ondragover={(event) => allowQueueDrop(event, index)}
							ondragleave={() => leaveQueueDrop(index)}
							ondrop={(event) => dropQueuedSong(event, index)}
							ondragend={endQueueDrag}
						>
							<span class="queue-track">
								<strong>{song.title}</strong>
								{#if queuedSongDetail(song)}
									<small>{queuedSongDetail(song)}</small>
								{/if}
							</span>
							<button onclick={() => removeQueuedSong(song)}>Remove</button>
						</li>
					{/each}
				</ol>
			{/if}
		</section>

		<section class="history-panel">
			<h3>History</h3>
			<ol>
				{#each oldSongs.slice(-4).reverse() as song}
					<li>
						<span class="queue-track">
							<strong>{song.title}</strong>
							{#if queuedSongDetail(song)}
								<small>{queuedSongDetail(song)}</small>
							{/if}
						</span>
					</li>
				{/each}
			</ol>
		</section>
	</aside>
</div>

<div class="miniPlayer">
	<div class="now-playing">
		<div class="cover" aria-hidden="true"></div>
		<div>
			<strong>{currentSong.title}</strong>
			<span>{nowPlayingDetail()}</span>
		</div>
	</div>
	<span class="time">{playbackTimeLabel()}</span>
	<div class="durationBar" style={`--progress: ${playbackProgress()}%`} aria-hidden="true"></div>
	<div class="transport">
		<button onclick={playPreviousQueueSong} disabled={oldSongs.length === 0}>Prev</button>
		<button
			onclick={resumePlayback}
			disabled={!canStartPlayback() || Boolean(playbackStatus?.playing) || spotifyIsPlaying()}
			>Play</button
		>
		<button onclick={pausePlayback} disabled={!playbackStatus?.playing && !spotifyIsPlaying()}
			>Pause</button
		>
		<button
			onclick={stopPlayback}
			disabled={!playbackStatus?.current_path && !spotifyPlaybackActive}>Stop</button
		>
		<button onclick={playNextQueueSong} disabled={upcomingSongs.length === 0}>Next</button>
	</div>
	<label class="volume">
		<span>Volume</span>
		<input
			type="range"
			min="0"
			max="1"
			step="0.01"
			value={volume}
			oninput={updateVolume}
			aria-label="Playback volume"
		/>
	</label>
</div>

<style>
	:global(:root) {
		--bg: oklch(15% 0.02 60);
		--surface: oklch(22% 0.026 58);
		--surface-2: oklch(28% 0.035 58);
		--surface-3: oklch(34% 0.04 58);
		--fg: oklch(93% 0.013 80);
		--muted: oklch(68% 0.023 72);
		--border: oklch(35% 0.032 60);
		--accent: oklch(70% 0.13 205);
		--accent-2: oklch(74% 0.14 78);
		--success: oklch(70% 0.13 150);
		--danger: oklch(68% 0.14 30);
		--shadow: 0 26px 80px color-mix(in oklch, black 34%, transparent);
		--font-display: 'Iowan Old Style', Charter, Georgia, serif;
		--font-body:
			-apple-system, BlinkMacSystemFont, 'SF Pro Text', 'Segoe UI', system-ui, sans-serif;
		--font-mono: 'JetBrains Mono', 'IBM Plex Mono', ui-monospace, Menlo, monospace;
		--radius-sm: 12px;
		--radius-md: 20px;
		--radius-lg: 32px;
		--tap: 48px;
	}

	:global(body) {
		margin: 0;
		background: var(--bg);
		color: var(--fg);
		font-family: var(--font-body);
		line-height: 1.45;
		text-rendering: optimizeLegibility;
		-webkit-font-smoothing: antialiased;
	}

	:global(*) {
		box-sizing: border-box;
		scrollbar-color: color-mix(in oklch, var(--accent) 55%, var(--border)) transparent;
	}

	:global(button),
	:global(input),
	:global(select),
	:global(textarea) {
		font: inherit;
	}

	:global(button) {
		min-height: 38px;
		border: 1px solid var(--border);
		background: color-mix(in oklch, var(--surface) 88%, transparent);
		color: var(--fg);
		border-radius: 999px;
		padding: 0.45rem 0.8rem;
		cursor: pointer;
	}

	:global(button:hover),
	:global(button:focus-visible) {
		border-color: var(--accent);
		outline: none;
	}

	:global(button:disabled) {
		cursor: default;
		opacity: 0.55;
	}

	.app-shell {
		display: grid;
		grid-template-columns: 230px minmax(0, 1fr) 330px;
		gap: 18px;
		min-height: 100vh;
		padding: 18px 18px 112px;
	}

	.sidenav {
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		align-content: start;
		gap: 20px;
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 92%, transparent);
		padding: 18px;
	}

	.brand-mark {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		font-family: var(--font-display);
		font-size: 20px;
		font-weight: 700;
	}

	.brand-dot {
		width: 28px;
		height: 28px;
		border-radius: 10px;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 78%, transparent) 0 18%,
				transparent 19%
			),
			conic-gradient(from 210deg, var(--fg), var(--accent), var(--accent-2), var(--fg));
	}

	.rail-kicker,
	.queue p,
	.now-playing span,
	.progress-labels,
	.rail-status span {
		color: var(--muted);
		font-size: 0.84rem;
	}

	.eyebrow {
		margin: 0 0 8px;
		color: var(--accent);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.rail-nav {
		display: grid;
		gap: 8px;
		align-content: start;
	}

	.rail-nav button {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 9px;
		min-height: 42px;
		padding: 0 12px;
		color: var(--muted);
		text-align: left;
	}

	.rail-nav button:hover,
	.rail-nav button:focus-visible {
		background: color-mix(in oklch, var(--accent) 11%, var(--surface));
		color: var(--fg);
	}

	.rail-status {
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface-2) 56%, transparent);
		padding: 16px;
	}

	.rail-status strong {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.content {
		min-width: 0;
		overflow: hidden;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 92%, transparent);
		padding: 20px;
	}

	.queue {
		display: grid;
		align-content: start;
		gap: 14px;
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 92%, transparent);
		padding: 18px;
		overflow: auto;
	}

	.desktop-player,
	.queue-panel,
	.history-panel {
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 16px;
	}

	.desktop-player {
		display: grid;
		gap: 14px;
	}

	.app-shell.player-shell .desktop-player {
		display: none;
	}

	.queue h3 {
		margin: 0 0 10px;
		font-size: 0.78rem;
		text-transform: uppercase;
		color: var(--muted);
	}

	.cover-art,
	.cover {
		position: relative;
		overflow: hidden;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 82%, transparent) 0 12%,
				transparent 13%
			),
			conic-gradient(from 235deg, var(--fg), var(--accent), var(--accent-2), var(--surface-2), var(--fg));
		box-shadow: 0 22px 50px color-mix(in oklch, black 20%, transparent);
	}

	.cover-art {
		aspect-ratio: 1;
		border: 1px solid color-mix(in oklch, var(--border) 72%, transparent);
		border-radius: var(--radius-lg);
	}

	.cover-art::before,
	.cover::before {
		content: '';
		position: absolute;
		inset: 8%;
		border: 1px solid color-mix(in oklch, var(--surface) 52%, transparent);
		border-radius: inherit;
	}

	.track-title {
		display: grid;
		gap: 5px;
		min-width: 0;
	}

	.track-title h2 {
		overflow: hidden;
		margin: 0;
		font-family: var(--font-display);
		font-size: clamp(24px, 3vw, 34px);
		line-height: 1.02;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.quality-row {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.quality-pill {
		display: inline-flex;
		align-items: center;
		min-height: 28px;
		border: 1px solid var(--border);
		border-radius: 999px;
		background: color-mix(in oklch, var(--surface) 72%, transparent);
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		padding: 0 10px;
		text-transform: uppercase;
	}

	.quality-pill.hires {
		border-color: color-mix(in oklch, var(--accent) 44%, var(--border));
		color: var(--accent);
	}

	.spectrum {
		display: grid;
		grid-template-columns: repeat(28, minmax(2px, 1fr));
		align-items: end;
		gap: 4px;
		height: 54px;
	}

	.spectrum span {
		height: var(--bar-height);
		min-height: 7px;
		border-radius: 999px;
		background: color-mix(in oklch, var(--accent) 72%, var(--surface));
	}

	.progress-block {
		display: grid;
		gap: 8px;
	}

	.progress-labels {
		display: flex;
		justify-content: space-between;
		font-family: var(--font-mono);
		font-size: 0.76rem;
	}

	.queue ol {
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.queue li.queue-item {
		display: grid;
		grid-template-columns: minmax(0, 1fr) auto;
		align-items: center;
		gap: 8px;
		margin-bottom: 8px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface) 76%, transparent);
		cursor: grab;
		padding: 10px;
	}

	.queue li.queue-item.dragging {
		opacity: 0.55;
	}

	.queue li.queue-item.drag-over {
		border-color: var(--accent);
		background: color-mix(in oklch, var(--accent) 14%, var(--surface));
	}

	.queue-track {
		display: grid;
		gap: 1px;
		min-width: 0;
	}

	.queue-track strong,
	.queue-track small {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.queue-track small {
		color: var(--muted);
		font-size: 0.76rem;
	}

	.queue li button {
		min-height: 32px;
		padding: 0 0.65rem;
		font-size: 0.76rem;
	}

	.history-panel li {
		margin-bottom: 8px;
		min-width: 0;
	}

	.miniPlayer {
		position: fixed;
		z-index: 10;
		right: 20px;
		bottom: 16px;
		left: 20px;
		display: grid;
		grid-template-columns: minmax(220px, 360px) auto minmax(160px, 1fr) auto minmax(130px, 180px);
		align-items: center;
		gap: 18px;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 94%, transparent);
		backdrop-filter: blur(18px);
		box-shadow: var(--shadow);
		padding: 12px 18px;
		box-sizing: border-box;
	}

	.now-playing {
		display: flex;
		align-items: center;
		gap: 12px;
		min-width: 0;
	}

	.now-playing div:last-child {
		display: grid;
		gap: 2px;
		min-width: 0;
	}

	.now-playing strong,
	.now-playing span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.cover {
		width: 48px;
		height: 48px;
		border: 1px solid var(--border);
		border-radius: 14px;
		box-shadow: none;
	}

	.durationBar {
		height: 8px;
		border-radius: 999px;
		background: linear-gradient(
			90deg,
			var(--accent) 0 var(--progress, 0%),
			color-mix(in oklch, var(--surface-3) 70%, transparent) var(--progress, 0%)
		);
	}

	.transport {
		display: flex;
		gap: 8px;
	}

	.time {
		color: var(--muted);
		font-family: var(--font-mono);
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}

	.volume {
		display: grid;
		gap: 3px;
		color: var(--muted);
		font-size: 0.78rem;
	}

	.volume input {
		width: 100%;
		accent-color: var(--accent);
	}

	@media (max-width: 1180px) {
		.app-shell {
			grid-template-columns: minmax(250px, 0.42fr) minmax(0, 1fr);
			gap: 18px;
			min-height: calc(100vh - 44px);
			padding: 22px 22px 118px;
		}

		.sidenav {
			grid-column: 1;
			grid-row: 1 / span 2;
			align-content: start;
			gap: 14px;
			padding: 16px;
		}

		.content {
			grid-column: 2;
			grid-row: 1;
		}

		.rail-status {
			display: none;
		}

		.queue {
			grid-column: 2;
			grid-row: 2;
			grid-template-columns: repeat(2, minmax(0, 1fr));
			align-items: start;
			overflow: visible;
		}

		.desktop-player {
			grid-column: 1 / -1;
			grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1fr);
			gap: 20px;
			align-items: center;
		}

		.desktop-player .cover-art {
			grid-row: 1 / span 4;
		}

		.desktop-player .spectrum {
			margin: 4px 0;
		}

		.miniPlayer {
			grid-template-columns: minmax(190px, 280px) auto minmax(120px, 1fr) auto;
		}

		.volume {
			display: none;
		}
	}

	@media (max-width: 880px) {
		.app-shell {
			width: min(430px, calc(100% - 24px));
			min-height: min(880px, calc(100vh - 24px));
			grid-template-columns: 1fr;
			grid-template-rows: auto minmax(0, 1fr);
			gap: 0;
			margin: 12px auto 132px;
			overflow: hidden;
			border: 1px solid var(--border);
			border-radius: 38px;
			background: var(--bg);
			box-shadow: var(--shadow);
			padding: 0;
		}

		.sidenav {
			grid-column: 1;
			grid-row: 1;
			grid-template-columns: 1fr auto;
			grid-template-rows: auto auto;
			align-items: center;
			gap: 12px;
			border: 0;
			border-bottom: 1px solid var(--border);
			border-radius: 0;
			background: color-mix(in oklch, var(--surface) 88%, transparent);
			padding: 18px 18px 10px;
		}

		.brand-mark {
			font-size: 18px;
		}

		.rail-kicker {
			display: none;
		}

		.rail-nav {
			grid-column: 1 / -1;
			grid-template-columns: repeat(3, minmax(0, 1fr));
			gap: 4px;
			border: 1px solid var(--border);
			border-radius: 999px;
			background: color-mix(in oklch, var(--surface) 72%, transparent);
			padding: 4px;
		}

		.rail-nav button {
			justify-content: center;
			min-height: 40px;
			border: 0;
			background: transparent;
			font-size: 0.78rem;
			white-space: nowrap;
			padding: 0 8px;
		}

		.content {
			grid-column: 1;
			grid-row: 2;
			overflow: auto;
			border: 0;
			border-radius: 0;
			background: transparent;
			padding: 16px;
		}

		.rail-status,
		.queue {
			display: none;
		}

		.miniPlayer {
			right: 0;
			left: 0;
			bottom: 12px;
			width: min(430px, calc(100% - 24px));
			margin: 0 auto;
			grid-template-columns: minmax(0, 1fr) auto;
			gap: 10px;
			border-radius: 28px;
			padding: 12px;
		}

		.app-shell.player-shell {
			margin-bottom: 12px;
		}

		.app-shell.player-shell + .miniPlayer {
			display: none;
		}

		.now-playing {
			grid-column: 1 / -1;
		}

		.transport {
			grid-column: 1 / -1;
			justify-content: space-between;
		}

		.transport button {
			flex: 1 1 0;
			min-width: 0;
			padding: 0 0.45rem;
			font-size: 0.78rem;
		}

		.miniPlayer > .durationBar,
		.miniPlayer > .time,
		.volume {
			display: none;
		}
	}
</style>
