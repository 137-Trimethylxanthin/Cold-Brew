<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { PlaybackStatus, QueueSnapshot, Song } from '$lib/types';
	import { playbackStatus, currentSong, queueSnapshot, volume } from '$lib/stores';
	import { formatSource, formatSampleRate, emptySong } from '$lib/playback';
	import NowPlaying from '$lib/components/NowPlaying.svelte';
	import { SkipBack, Play, Pause, Square, SkipForward } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Slider } from '$lib/components/ui/slider';

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

<section class="player-route" data-od-id="player-route">
	<section class="player-hero" data-od-id="player-hero">
		<NowPlaying song={$currentSong} status={$playbackStatus} />

		<div class="player-copy">
			{#if spotifyIsActive}
				<p class="text-soft font-mono text-xs tracking-widest uppercase">Spotify is playing</p>
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
				<div class="flex justify-between font-mono text-xs text-soft">
					<span>{playbackTimeLabel()}</span>
					<span>{$playbackStatus?.output_sample_rate ? formatSampleRate($playbackStatus.output_sample_rate) : 'Ready'}</span>
				</div>
			</div>

			<div class="player-transport" aria-label="Playback controls">
			<Button onclick={() => runPlayerCommand('play_previous_queue_song')} disabled={oldSongs.length === 0}>
			<SkipBack class="size-4" /> Prev
			</Button>
			<Button class="player-transport-main" onclick={playRouteSelection}
			disabled={$currentSong.id === '' && upcomingSongs.length === 0}>
			<Play class="size-4" /> Play
			</Button>
			<Button onclick={() => runPlayerCommand('playback_pause')} disabled={!$playbackStatus?.playing}>
			<Pause class="size-4" /> Pause
			</Button>
			<Button onclick={() => runPlayerCommand('playback_stop')} disabled={!$playbackStatus?.current_path}>
			<Square class="size-4" /> Stop
			</Button>
			<Button onclick={() => runPlayerCommand('play_next_queue_song')} disabled={upcomingSongs.length === 0}>
			<SkipForward class="size-4" /> Next
			</Button>
			</div>

			<label class="grid gap-2 text-soft text-sm">
			<span>Volume {$volume}</span>
			<Slider value={[$volume * 100]} min={0} max={100} step={1} onValueChange={async (v: number[]) => { const val = v[0] / 100; $volume = val; try { await invoke('set_playback_volume', { volume: val }); } catch { /* Browser-only dev */ } }} />
			</label>
		</div>
	</section>

	<div class="player-panel-grid">
		<section class="player-panel">
			<div class="flex items-end justify-between gap-3">
				<div>
					<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">Queue</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Up next</h2>
				</div>
				<span class="state-pill">{upcomingSongs.length} tracks</span>
			</div>
			{#if upcomingSongs.length === 0}
				<p class="text-soft">Queue is empty</p>
			{:else}
				<ol class="grid gap-2 m-0 p-0 list-none">
					{#each upcomingSongs.slice(0, 5) as song, index}
						<li class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3 border border-outline rounded-2xl p-2.5 bg-surface/78">
							<span class="text-soft font-mono text-xs">{String(index + 1).padStart(2, '0')}</span>
							<span class="grid gap-0.5 min-w-0">
								<strong class="truncate">{song.title}</strong>
								{#if queuedSongDetail(song)}
									<small class="truncate text-soft">{queuedSongDetail(song)}</small>
								{/if}
							</span>
						</li>
					{/each}
				</ol>
			{/if}
		</section>

		<section class="player-panel">
			<div class="flex items-end justify-between gap-3">
				<div>
					<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">Output</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">{$playbackStatus?.output_device_name ?? 'Default device'}</h2>
				</div>
				<span class="state-pill">{$playbackStatus?.playing ? 'Playing' : 'Idle'}</span>
			</div>
			<div class="grid grid-cols-3 gap-2.5">
				<div class="grid gap-1 min-w-0 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-xs uppercase text-soft">Source</span>
					<strong>{$currentSong.source ? formatSource($currentSong.source) : 'Local'}</strong>
				</div>
				<div class="grid gap-1 min-w-0 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-xs uppercase text-soft">Quality</span>
					<strong>{qualityDisplay()}</strong>
				</div>
				<div class="grid gap-1 min-w-0 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]">
					<span class="font-mono text-xs uppercase text-soft">ReplayGain</span>
					<strong>{$playbackStatus?.replay_gain_mode ?? 'off'}</strong>
				</div>
			</div>
		</section>
	</div>

	{#if oldSongs.length > 0}
		<section class="player-panel">
			<div class="flex items-end justify-between gap-3">
				<div>
					<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">History</p>
					<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Recent</h2>
				</div>
			</div>
			<ol class="grid gap-2 m-0 p-0 list-none">
				{#each oldSongs.slice(-4).reverse() as song}
					<li class="grid gap-1 min-w-0 border border-outline rounded-2xl p-2.5 bg-surface-2/[0.42]">
						<strong class="truncate">{song.title}</strong>
						<span class="truncate text-soft">{queuedSongDetail(song)}</span>
					</li>
				{/each}
			</ol>
		</section>
	{/if}

	{#if routeError}
		<p class="px-3 py-2 border border-outline rounded-2xl bg-danger/20 text-danger">{routeError}</p>
	{/if}
</section>


