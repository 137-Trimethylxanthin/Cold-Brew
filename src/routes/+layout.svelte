<script lang="ts">
	import '../app.css';
	import { page } from '$app/stores';
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import { toast, Toaster } from 'svelte-sonner';
	import type { PlaybackStatus, QueuePlaybackResult, QueueSnapshot, Song } from '$lib/types';
	import { playbackStatus, currentSong, volume } from '$lib/stores';
	import { toErrorMessage, playbackQualityLabel, formatSource, formatSampleRate, formatDb, titleFromPath, emptySong } from '$lib/playback';
	import SideNav from '$lib/components/SideNav.svelte';
	import QueuePanel from '$lib/components/QueuePanel.svelte';
	import MiniPlayer from '$lib/components/MiniPlayer.svelte';
	import { Library, Play, MousePointer2, Settings } from '@lucide/svelte';
	import { goto } from '$app/navigation';

	let oldSongs: Song[] = [];
	let upcomingSongs: Song[] = [];
	let nativePlaybackActive = false;
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

	$effect(() => {
		$page.url.pathname;
		// re-routed – bottom tab highlights automatically via $page
	});

	let queueSheetOpen = $state(false);

	const bottomTabs = [
		{ id: 'library', icon: Library, label: 'Library', path: '/' },
		{ id: 'player', icon: Play, label: 'Player', path: '/player' },
		{ id: 'explore', icon: MousePointer2, label: 'Explore', path: '/explore' },
		{ id: 'settings', icon: Settings, label: 'Settings', path: '/settings' }
	];

	function isTabActive(path: string) {
		if (path === '/') return $page.url.pathname === '/';
		return $page.url.pathname.startsWith(path);
	}

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
				if ($playbackStatus?.playing || spotifyIsPlaying()) {
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
		} catch {}
	}

	async function refreshSpotifyState() {
		if (!spotifyPlayer || !spotifyPlaybackActive) return;
		try {
			const state = await spotifyPlayer.getCurrentState();
			if (state) syncSpotifyState(state);
		} catch (error) {
			toast.error(toErrorMessage(error));
		}
	}

	async function refreshQueue() {
		try {
			syncQueueSnapshot(await invoke<QueueSnapshot>('get_queue_snapshot'));
		} catch {}
	}

	function syncQueueSnapshot(queue: QueueSnapshot) {
		oldSongs = queue.old;
		upcomingSongs = queue.upcoming;

		if (!nativePlaybackActive && queue.current_song) {
			$currentSong = queue.current_song;
			fetchCoverArt($currentSong);
		} else if (!nativePlaybackActive && !queue.current_song && !spotifyPlaybackActive) {
			$currentSong = emptySong();
		}
	}

	async function syncQueuePlaybackResult(result: QueuePlaybackResult) {
		syncQueueSnapshot(result.queue);
		if (result.playback_status) {
			if (spotifyPlaybackActive) await pauseSpotifyPlayback(false);
			spotifyPlaybackActive = false;
			syncPlaybackStatus(result.playback_status);
			return;
		}

		const song = result.queue.current_song;
		if (song?.source === 'spotify') {
			try {
				await playSpotifySong(song, result.queue);
			} catch (error) {
				toast.error(toErrorMessage(error));
			}
			return;
		}

		if (result.message) {
			toast.error(result.message);
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

	function syncPlaybackStatus(status: PlaybackStatus) {
		$playbackStatus = status;
		$volume = status.volume;

		if (status.current_path) {
			if (spotifyPlaybackActive) void pauseSpotifyPlayback(false);
			spotifyPlaybackActive = false;
			nativePlaybackActive = true;
			$currentSong = {
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
			fetchCoverArt($currentSong);
		} else if (nativePlaybackActive) {
			nativePlaybackActive = false;
			if (!spotifyPlaybackActive) $currentSong = emptySong();
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
			return;
		}
		if ($playbackStatus?.current_path) {
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
			if (!nativePlaybackActive) $currentSong = emptySong();
			return;
		}
		syncPlaybackStatus(await invoke<PlaybackStatus>('playback_stop'));
	}

	async function updateVolume(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const vol = Number(input.value);
		$volume = vol;
		if (spotifyPlaybackActive) {
			if (spotifyPlayer && spotifyReady) {
				await spotifyPlayer.setVolume(vol);
			} else {
				await spotifyApiFetch(`/me/player/volume?volume_percent=${Math.round(vol * 100)}`, {
					method: 'PUT'
				});
			}
			return;
		}
		syncPlaybackStatus(await invoke<PlaybackStatus>('set_playback_volume', { volume: vol }));
	}

	async function playPreviousQueueSong() {
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_previous_queue_song'));
	}

	async function playNextQueueSong() {
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_next_queue_song'));
	}

	async function playSpotifySong(song: Song, queue: QueueSnapshot) {
		if (!song.uri?.startsWith('spotify:')) {
			toast.error('Queued Spotify track is missing a Spotify URI.');
			return;
		}
		if (song.playable === false) {
			toast.error('This Spotify track is not playable for the current account or market.');
			return;
		}

		if ($playbackStatus?.current_path) {
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
		$currentSong = song;
		spotifyPlaybackActive = true;
		spotifyPaused = false;
		spotifyStoppedByApp = false;
		spotifyLastQueueSyncId = song.id;
		spotifyPositionMs = 0;
		spotifyDurationMs = song.duration ? Math.round(song.duration / 10000) : null;
		spotifyStateUpdatedAt = Date.now();
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
			toast.error(`${toErrorMessage(error)} Falling back to the active Spotify device.`);
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
			volume: $volume,
			getOAuthToken: (callback) => {
				void getSpotifyToken()
					.then(callback)
					.catch((error) => {
						toast.error(toErrorMessage(error));
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
				const msg = `Spotify ${event.replaceAll('_', ' ')}: ${error.message}`;
				toast.error(msg);
				spotifyRejectDevice?.(new Error(msg));
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
			$currentSong = {
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

	async function fetchCoverArt(song: Song) {
		if (song.source === 'local' && !song.cover_art) {
			try {
				const art = await invoke<{ mime_type: string; data: string }>('get_track_cover_art', {
					path: song.id
				});
				song.cover_art = `data:${art.mime_type};base64,${art.data}`;
			} catch {
				song.cover_art = null;
			}
		}
	}

	async function playTrackNow(song: Song) {
		await syncQueuePlaybackResult(await invoke<QueuePlaybackResult>('play_track_now', { song }));
	}

	function canStartPlayback() {
		return Boolean(
			spotifyPlaybackActive ||
			$playbackStatus?.current_path ||
			$currentSong.id ||
			upcomingSongs.length > 0
		);
	}

	function isTypingTarget(target: EventTarget | null) {
		if (!(target instanceof HTMLElement)) return false;
		return ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName) || target.isContentEditable;
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="app-shell" data-od-id="app-shell" role="application">
	<SideNav />
	<div class="sidebar-icon-rail">
		<button onclick={() => goto('/')} aria-label="Library">
			<Library class="size-5" />
		</button>
		<button onclick={() => goto('/player')} aria-label="Player">
			<Play class="size-5" />
		</button>
		<button onclick={() => goto('/explore')} aria-label="Explore">
			<MousePointer2 class="size-5" />
		</button>
		<button onclick={() => goto('/settings')} aria-label="Settings">
			<Settings class="size-5" />
		</button>
	</div>
	<main class="content" data-od-id="content" id="main-content">
		<slot />
	</main>
	<div class="queue-panel-desktop" data-od-id="queue-panel-desktop">
		<QueuePanel
			{upcomingSongs}
			{oldSongs}
			onRemove={removeQueuedSong}
			onMove={moveQueuedSong}
		/>
	</div>
</div>

<!-- Mobile bottom tab bar -->
<nav class="bottom-tab-bar" data-od-id="bottom-tab-bar" aria-label="Bottom navigation">
	{#each bottomTabs as tab}
		<button
			class="bottom-tab-button"
			class:text-brand={isTabActive(tab.path)}
			onclick={() => goto(tab.path)}
			aria-label={tab.label}
			aria-current={isTabActive(tab.path) ? 'page' : undefined}
		>
			<tab.icon class="size-5" />
			<span>{tab.label}</span>
		</button>
	{/each}
</nav>

<MiniPlayer
	onPlayPrevious={playPreviousQueueSong}
	onResume={resumePlayback}
	onPause={pausePlayback}
	onStop={stopPlayback}
	onPlayNext={playNextQueueSong}
	onVolumeChange={updateVolume}
	canPlay={canStartPlayback()}
	isPlaying={Boolean($playbackStatus?.playing) || spotifyIsPlaying()}
	isPauseEnabled={Boolean($playbackStatus?.playing) || spotifyIsPlaying()}
	isStopEnabled={Boolean($playbackStatus?.current_path) || spotifyPlaybackActive}
	canPrev={oldSongs.length > 0}
	canNext={upcomingSongs.length > 0}
/>

<Toaster />
