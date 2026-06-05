<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { PlaybackStatus, QueueSnapshot, Song } from '$lib/types';
	import { playbackStatus, currentSong, queueSnapshot, volume } from '$lib/stores';
	import { formatSource, formatSampleRate, emptySong } from '$lib/playback';
	import NowPlaying from '$lib/components/NowPlaying.svelte';
	import { SkipBack, Play, Pause, Square, SkipForward } from '@lucide/svelte';

	let upcomingSongs: Song[] = $state([]);
	let oldSongs: Song[] = $state([]);
	let routeError = $state('');
	let spotifyIsActive = $state(false);

	onMount(() => {
		void refreshPlayerRoute();
		const refreshTimer = window.setInterval(() => { void refreshPlayerRoute(); }, 1500);
		return () => window.clearInterval(refreshTimer);
	});

	async function refreshPlayerRoute() {
		await Promise.all([refreshQueue(), refreshPlaybackStatus()]);
	}

	async function refreshQueue() {
		try {
			const queue = await invoke<QueueSnapshot>('get_queue_snapshot');
			$queueSnapshot = queue;
			upcomingSongs = queue.upcoming;
			oldSongs = queue.old;
			routeError = '';
		} catch { /* Tauri commands not available during browser-only dev */ }
	}

	async function refreshPlaybackStatus() {
		try {
			const status = await invoke<PlaybackStatus>('get_playback_status');
			$playbackStatus = status;
			spotifyIsActive = false;
			routeError = '';
		} catch { /* Tauri commands not available during browser-only dev */ }
	}

	async function runPlayerCommand(command: string) {
		try {
			await invoke(command);
			await refreshPlayerRoute();
			routeError = '';
		} catch (error) { routeError = toErrorMessage(error); }
	}

	async function playRouteSelection() {
		try {
			if ($playbackStatus?.current_path) {
				await invoke<PlaybackStatus>('playback_resume');
			} else {
				await invoke('play_current_queue_song');
			}
			await refreshPlayerRoute();
			routeError = '';
		} catch (error) { routeError = toErrorMessage(error); }
	}

	async function handleVolumeChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const val = Number(input.value);
		$volume = val;
		try { await invoke<PlaybackStatus>('set_playback_volume', { volume: val }); } catch { /* Browser-only dev */ }
	}

	function nowPlayingDetail() {
		const song = $currentSong;
		const parts = [song.artist, song.album].filter(Boolean);
		if (parts.length > 0) return parts.join(' - ');
		return $playbackStatus?.state ?? 'Idle';
	}

	function queuedSongDetail(song: Song) {
		return [song.artist, formatSource(song.source ?? '')].filter(Boolean).join(' - ');
	}

	function formatDuration(durationMs: number | null) {
		if (!durationMs) return '0:00';
		const totalSeconds = Math.floor(durationMs / 1000);
		const minutes = Math.floor(totalSeconds / 60);
		const seconds = totalSeconds % 60;
		return `${minutes}:${seconds.toString().padStart(2, '0')}`;
	}

	function songDurationMs(song: Song) {
		return song.duration ? Math.round(song.duration / 10000) : null;
	}

	function playbackProgress() {
		const durationMs = $playbackStatus?.duration_ms ?? songDurationMs($currentSong);
		if (!durationMs || durationMs <= 0) return 0;
		const positionMs = $playbackStatus?.position_ms ?? 0;
		return Math.min(100, Math.max(0, (positionMs / durationMs) * 100));
	}

	function playbackTimeLabel() {
		const position = formatDuration($playbackStatus?.position_ms ?? 0);
		const duration = formatDuration($playbackStatus?.duration_ms ?? songDurationMs($currentSong));
		return `${position} / ${duration}`;
	}

	function qualityDisplay() {
		if ($currentSong.quality) return $currentSong.quality;
		return 'Pending';
	}

	function toErrorMessage(error: unknown) {
		if (typeof error === 'string') return error;
		if (error instanceof Error) return error.message;
		return 'Unexpected playback error.';
	}
</script>

<section class="player-route">
	<section class="player-hero">
		<NowPlaying song={$currentSong} status={$playbackStatus} />

		<div class="player-copy">
			{#if spotifyIsActive}
				<p class="text-danger font-mono text-[0.68rem] tracking-widest uppercase">Spotify is playing</p>
			{/if}
			<div class="flex flex-wrap gap-2">
				<span class="state-pill">{formatSource($currentSong.source ?? 'local') || 'Local'}</span>
				{#if $currentSong.quality}
					<span class="state-pill partial">{$currentSong.quality}</span>
				{/if}
				<span class="state-pill">{$playbackStatus?.state ?? 'Idle'}</span>
			</div>

			<div class="grid grid-cols-[repeat(28,minmax(2px,1fr))] items-end gap-1 h-[54px]" aria-hidden="true">
				{#each Array.from({ length: 28 }, () => Math.floor(Math.random() * 70) + 28) as height}
					<span class="spectrum-bar" style="--bar-height: {height}%"></span>
				{/each}
			</div>

			<div class="grid gap-2">
				<div class="duration-bar" style="--progress: {playbackProgress()}%" aria-hidden="true"></div>
				<div class="flex justify-between font-mono text-[0.76rem] text-muted">
					<span>{playbackTimeLabel()}</span>
					<span>{$playbackStatus?.output_sample_rate ? formatSampleRate($playbackStatus.output_sample_rate) : 'Ready'}</span>
				</div>
			</div>

			<div class="player-transport" aria-label="Playback controls">
				<button onclick={() => runPlayerCommand('play_previous_queue_song')} disabled={oldSongs.length === 0}>
					<SkipBack class="size-4" /> Prev
				</button>
				<button class="player-transport-main" onclick={playRouteSelection}
					disabled={$currentSong.id === '' && upcomingSongs.length === 0}>
					<Play class="size-4" /> Play
				</button>
				<button onclick={() => runPlayerCommand('playback_pause')} disabled={!$playbackStatus?.playing}>
					<Pause class="size-4" /> Pause
				</button>
				<button onclick={() => runPlayerCommand('playback_stop')} disabled={!$playbackStatus?.current_path}>
					<Square class="size-4" /> Stop
				</button>
				<button onclick={() => runPlayerCommand('play_next_queue_song')} disabled={upcomingSongs.length === 0}>
					<SkipForward class="size-4" /> Next
				</button>
			</div>

			<label class="grid gap-[3px] text-muted text-[0.78rem]">
				<span>Volume</span>
				<input type="range" min="0" max="1" step="0.01" value={$volume}
					oninput={handleVolumeChange} aria-label="Playback volume" class="w-full accent-accent" />
			</label>
		</div>
	</section>

	<div class="player-panel-grid">
		<section class="player-panel">
			<div class="flex items-end justify-between gap-[14px]">
				<div>
					<p class="text-accent font-mono text-[0.68rem] tracking-widest uppercase m-0">Queue</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Up next</h2>
				</div>
				<span class="state-pill">{upcomingSongs.length} tracks</span>
			</div>
			{#if upcomingSongs.length === 0}
				<p class="text-muted">Queue is empty</p>
			{:else}
				<ol class="grid gap-2 m-0 p-0 list-none">
					{#each upcomingSongs.slice(0, 5) as song, index}
						<li class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3 border border-border rounded-[20px] p-2.5 bg-surface/78">
							<span class="text-muted font-mono text-[0.76rem]">{String(index + 1).padStart(2, '0')}</span>
							<span class="grid gap-0.5 min-w-0">
								<strong class="truncate">{song.title}</strong>
								{#if queuedSongDetail(song)}
									<small class="truncate text-muted">{queuedSongDetail(song)}</small>
								{/if}
							</span>
						</li>
					{/each}
				</ol>
			{/if}
		</section>

		<section class="player-panel">
			<div class="flex items-end justify-between gap-[14px]">
				<div>
					<p class="text-accent font-mono text-[0.68rem] tracking-widest uppercase m-0">Output</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{$playbackStatus?.output_device_name ?? 'Default device'}</h2>
				</div>
				<span class="state-pill">{$playbackStatus?.playing ? 'Playing' : 'Idle'}</span>
			</div>
			<div class="grid grid-cols-3 gap-2.5">
				<div class="grid gap-1 min-w-0 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-[0.76rem] uppercase text-muted">Source</span>
					<strong>{$currentSong.source ? formatSource($currentSong.source) : 'Local'}</strong>
				</div>
				<div class="grid gap-1 min-w-0 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-[0.76rem] uppercase text-muted">Quality</span>
					<strong>{qualityDisplay()}</strong>
				</div>
				<div class="grid gap-1 min-w-0 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-[0.76rem] uppercase text-muted">ReplayGain</span>
					<strong>{$playbackStatus?.replay_gain_mode ?? 'off'}</strong>
				</div>
			</div>
		</section>
	</div>

	{#if oldSongs.length > 0}
		<section class="player-panel">
			<div class="flex items-end justify-between gap-[14px]">
				<div>
					<p class="text-accent font-mono text-[0.68rem] tracking-widest uppercase m-0">History</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Recent</h2>
				</div>
			</div>
			<ol class="grid gap-2 m-0 p-0 list-none">
				{#each oldSongs.slice(-4).reverse() as song}
					<li class="grid gap-1 min-w-0 border border-border rounded-[20px] p-2.5 bg-surface-2/[0.42]">
						<strong class="truncate">{song.title}</strong>
						<span class="truncate text-muted">{queuedSongDetail(song)}</span>
					</li>
				{/each}
			</ol>
		</section>
	{/if}

	{#if routeError}
		<p class="p-[0.65rem] px-3.5 border border-border rounded-[20px] bg-danger/20 text-danger/70">{routeError}</p>
	{/if}
</section>

<style>
	.player-route { display: grid; gap: 16px; }

	.player-hero {
		display: grid;
		grid-template-columns: minmax(260px, 0.42fr) minmax(0, 1fr);
		gap: 24px;
		align-items: start;
		min-height: 440px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		background: linear-gradient(145deg, oklch(22% 0.026 58 / 0.92), oklch(28% 0.035 58 / 0.58));
		box-shadow: 0 26px 80px oklch(0% 0 0 / 0.34);
		padding: clamp(18px, 3vw, 28px);
	}

	.player-copy { display: grid; gap: 14px; min-width: 0; }

	.player-transport {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
	}

	.player-transport button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: 48px;
		border: 1px solid var(--color-border);
		border-radius: 999px;
		background: oklch(22% 0.026 58 / 0.86);
		color: var(--color-fg);
		padding: 0 16px;
	}

	.player-transport-main {
		min-width: 66px;
		border-color: var(--color-fg);
		background: var(--color-fg);
		color: var(--color-bg);
	}

	.player-panel-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 14px;
	}

	.player-panel {
		display: grid;
		align-content: start;
		gap: 12px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-lg);
		background: oklch(22% 0.026 58 / 0.9);
		padding: 18px;
	}

	@media (max-width: 1180px) and (min-width: 761px) {
		.player-hero {
			grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1fr);
			min-height: 360px;
		}
		.player-transport {
			display: grid;
			grid-template-columns: repeat(5, minmax(0, 1fr));
			gap: 6px;
			width: 100%;
		}
		.player-transport button { min-width: 0; min-height: 44px; padding: 0 0.3rem; font-size: 0.8rem; }
		.player-transport-main { min-width: 0; }
	}

	@media (max-width: 760px) {
		.player-route { gap: 12px; }
		.player-hero, .player-panel-grid, .player-panel .grid-cols-3 { grid-template-columns: 1fr; }
		.player-hero { min-height: 0; gap: 12px; text-align: center; padding: 16px; }
		.player-copy { gap: 10px; }
		.player-transport {
			display: grid;
			grid-template-columns: repeat(5, minmax(0, 1fr));
			gap: 6px;
			width: 100%;
		}
		.player-transport button { min-width: 0; min-height: 42px; padding: 0 0.25rem; font-size: 0.76rem; }
		.player-transport-main { min-width: 0; }
	}
</style>
