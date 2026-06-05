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
		const refreshTimer = window.setInterval(() => {
			void refreshPlayerRoute();
		}, 1500);
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
		} catch {
			// Tauri commands not available during browser-only dev
		}
	}

	async function refreshPlaybackStatus() {
		try {
			const status = await invoke<PlaybackStatus>('get_playback_status');
			$playbackStatus = status;
			spotifyIsActive = false;
			routeError = '';
		} catch {
			// Tauri commands not available during browser-only dev
		}
	}

	async function runPlayerCommand(command: string) {
		try {
			await invoke(command);
			await refreshPlayerRoute();
			routeError = '';
		} catch (error) {
			routeError = toErrorMessage(error);
		}
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
		} catch (error) {
			routeError = toErrorMessage(error);
		}
	}

	async function handleVolumeChange(event: Event) {
		const input = event.currentTarget as HTMLInputElement;
		const val = Number(input.value);
		$volume = val;
		try {
			await invoke<PlaybackStatus>('set_playback_volume', { volume: val });
		} catch {
			// Browser-only dev
		}
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
				<p class="eyebrow accent-warn">Spotify is playing</p>
			{/if}
			<div class="quality-row">
				<span class="quality-pill">{formatSource($currentSong.source ?? 'local') || 'Local'}</span>
				{#if $currentSong.quality}
					<span class="quality-pill hires">{$currentSong.quality}</span>
				{/if}
				<span class="quality-pill">{$playbackStatus?.state ?? 'Idle'}</span>
			</div>

			<div class="spectrum" aria-hidden="true">
				{#each Array.from({ length: 28 }, () => Math.floor(Math.random() * 70) + 28) as height}
					<span style={`--bar-height: ${height}%`}></span>
				{/each}
			</div>

			<div class="progress-block">
				<div class="duration-bar" style={`--progress: ${playbackProgress()}%`} aria-hidden="true"></div>
				<div class="progress-labels">
					<span>{playbackTimeLabel()}</span>
					<span>{$playbackStatus?.output_sample_rate ? formatSampleRate($playbackStatus.output_sample_rate) : 'Ready'}</span>
				</div>
			</div>

			<div class="transport" aria-label="Playback controls">
				<button onclick={() => runPlayerCommand('play_previous_queue_song')} disabled={oldSongs.length === 0}>
					<SkipBack class="size-4" /> Prev
				</button>
				<button class="main" onclick={playRouteSelection}
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

			<label class="volume-label">
				<span>Volume</span>
				<input type="range" min="0" max="1" step="0.01" value={$volume}
					oninput={handleVolumeChange} aria-label="Playback volume" />
			</label>
		</div>
	</section>

	<div class="player-grid">
		<section class="panel">
			<div class="section-title">
				<div>
					<p class="eyebrow">Queue</p>
					<h2>Up next</h2>
				</div>
				<span class="state-pill">{upcomingSongs.length} tracks</span>
			</div>
			{#if upcomingSongs.length === 0}
				<p class="muted">Queue is empty</p>
			{:else}
				<ol class="queue-list">
					{#each upcomingSongs.slice(0, 5) as song, index}
						<li class="queue-item">
							<span class="queue-index">{String(index + 1).padStart(2, '0')}</span>
							<span class="queue-main">
								<strong>{song.title}</strong>
								{#if queuedSongDetail(song)}
									<small>{queuedSongDetail(song)}</small>
								{/if}
							</span>
						</li>
					{/each}
				</ol>
			{/if}
		</section>

		<section class="panel">
			<div class="section-title">
				<div>
					<p class="eyebrow">Output</p>
					<h2>{$playbackStatus?.output_device_name ?? 'Default device'}</h2>
				</div>
				<span class="state-pill">{$playbackStatus?.playing ? 'Playing' : 'Idle'}</span>
			</div>
			<div class="stat-grid">
				<div>
					<span>Source</span>
					<strong>{$currentSong.source ? formatSource($currentSong.source) : 'Local'}</strong>
				</div>
				<div>
					<span>Quality</span>
					<strong>{qualityDisplay()}</strong>
				</div>
				<div>
					<span>ReplayGain</span>
					<strong>{$playbackStatus?.replay_gain_mode ?? 'off'}</strong>
				</div>
			</div>
		</section>
	</div>

	{#if oldSongs.length > 0}
		<section class="panel history">
			<div class="section-title">
				<div>
					<p class="eyebrow">History</p>
					<h2>Recent</h2>
				</div>
			</div>
			<ol class="history-list">
				{#each oldSongs.slice(-4).reverse() as song}
					<li>
						<strong>{song.title}</strong>
						<span>{queuedSongDetail(song)}</span>
					</li>
				{/each}
			</ol>
		</section>
	{/if}

	{#if routeError}
		<p class="error">{routeError}</p>
	{/if}
</section>

<style>
	.player-route {
		display: grid;
		gap: 16px;
	}

	h2,
	p {
		margin: 0;
	}

	h2 {
		font-family: var(--font-display);
		font-size: clamp(22px, 2vw, 30px);
		line-height: 1.04;
	}

	.player-hero,
	.panel {
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 90%, transparent);
	}

	.player-hero {
		display: grid;
		grid-template-columns: minmax(260px, 0.42fr) minmax(0, 1fr);
		gap: 24px;
		align-items: start;
		min-height: 440px;
		background:
			linear-gradient(
				145deg,
				color-mix(in oklch, var(--surface) 92%, transparent),
				color-mix(in oklch, var(--surface-2) 58%, transparent)
			);
		box-shadow: var(--shadow);
		padding: clamp(18px, 3vw, 28px);
	}

	.player-copy {
		display: grid;
		gap: 14px;
		min-width: 0;
	}

	.player-copy > p:not(.eyebrow),
	.muted,
	.progress-labels,
	.history-list span,
	.queue-main small,
	.stat-grid span {
		color: var(--muted);
	}

	.eyebrow {
		color: var(--accent);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.accent-warn {
		color: var(--danger);
	}

	.quality-row {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}

	.quality-pill,
	.state-pill {
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

	.duration-bar {
		height: 8px;
		border-radius: 999px;
		background: linear-gradient(
			90deg,
			var(--accent) 0 var(--progress, 0%),
			color-mix(in oklch, var(--surface-3) 70%, transparent) var(--progress, 0%)
		);
	}

	.progress-labels {
		display: flex;
		justify-content: space-between;
		font-family: var(--font-mono);
		font-size: 0.76rem;
	}

	.transport {
		display: flex;
		align-items: center;
		flex-wrap: wrap;
		gap: 8px;
	}

	.transport button {
		display: inline-flex;
		align-items: center;
		gap: 6px;
		min-height: var(--tap);
		border: 1px solid var(--border);
		border-radius: 999px;
		background: color-mix(in oklch, var(--surface) 86%, transparent);
		color: var(--fg);
		padding: 0 16px;
	}

	.transport button.main {
		min-width: 66px;
		border-color: var(--fg);
		background: var(--fg);
		color: var(--bg);
	}

	.volume-label {
		display: grid;
		gap: 3px;
		color: var(--muted);
		font-size: 0.78rem;
	}

	.volume-label input {
		width: 100%;
		accent-color: var(--accent);
	}

	.player-grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 14px;
	}

	.panel {
		display: grid;
		align-content: start;
		gap: 12px;
		padding: 18px;
	}

	.section-title {
		display: flex;
		align-items: end;
		justify-content: space-between;
		gap: 14px;
	}

	.queue-list,
	.history-list {
		display: grid;
		gap: 8px;
		margin: 0;
		padding: 0;
		list-style: none;
	}

	.queue-item {
		display: grid;
		grid-template-columns: auto minmax(0, 1fr);
		align-items: center;
		gap: 12px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface) 78%, transparent);
		padding: 10px;
	}

	.queue-index {
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.76rem;
	}

	.queue-main {
		display: grid;
		gap: 2px;
		min-width: 0;
	}

	.queue-main strong,
	.queue-main small,
	.history-list strong,
	.history-list span {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.stat-grid {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		gap: 10px;
	}

	.stat-grid div,
	.history-list li {
		display: grid;
		gap: 4px;
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 10px;
	}

	.stat-grid span {
		font-family: var(--font-mono);
		font-size: 0.76rem;
		text-transform: uppercase;
	}

	.error {
		padding: 0.65rem 0.8rem;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--danger) 20%, var(--surface));
		color: color-mix(in oklch, var(--danger) 72%, var(--fg));
	}

	@media (max-width: 1180px) and (min-width: 761px) {
		.player-hero {
			grid-template-columns: minmax(220px, 0.8fr) minmax(0, 1fr);
			min-height: 360px;
		}

		.transport {
			display: grid;
			grid-template-columns: repeat(5, minmax(0, 1fr));
			gap: 6px;
			width: 100%;
		}

		.transport button {
			min-width: 0;
			min-height: 44px;
			padding: 0 0.3rem;
			font-size: 0.8rem;
		}

		.transport button.main {
			min-width: 0;
		}
	}

	@media (max-width: 760px) {
		.player-route {
			gap: 12px;
		}

		.player-hero,
		.player-grid,
		.stat-grid {
			grid-template-columns: 1fr;
		}

		.player-hero {
			min-height: 0;
			gap: 12px;
			text-align: center;
			padding: 16px;
		}

		.player-copy {
			gap: 10px;
		}

		.spectrum {
			height: 44px;
			gap: 3px;
		}

		.quality-row {
			justify-content: center;
		}

		.transport {
			display: grid;
			grid-template-columns: repeat(5, minmax(0, 1fr));
			gap: 6px;
			width: 100%;
		}

		.transport button {
			min-width: 0;
			min-height: 42px;
			padding: 0 0.25rem;
			font-size: 0.76rem;
		}

		.transport button.main {
			min-width: 0;
		}

		.section-title {
			align-items: flex-start;
		}

		.panel {
			padding: 14px;
		}
	}
</style>
