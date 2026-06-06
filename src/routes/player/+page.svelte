<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { PlaybackStatus, QueueSnapshot, Song } from '$lib/types';
	import { playbackStatus, currentSong, queueSnapshot, volume } from '$lib/stores';
	import { formatSource, formatSampleRate, emptySong } from '$lib/playback';
	import NowPlaying from '$lib/components/NowPlaying.svelte';
	import SpectrumAnalyzer from '$lib/components/SpectrumAnalyzer.svelte';
	import ProgressBar from '$lib/components/ProgressBar.svelte';
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import { SkipBack, Play, Pause, Square, SkipForward } from '@lucide/svelte';
	import { t } from '$lib/i18n';
	import { Button } from '$lib/components/ui/button';
	import { Slider } from '$lib/components/ui/slider';

	let upcomingSongs: Song[] = $state([]);
	let oldSongs: Song[] = $state([]);
	let routeError = $state('');
	let spotifyIsActive = $state(false);

	const transportButtonClass =
		'h-11 rounded-full border-outline bg-surface/86 px-4 text-fg max-xl:min-w-0 max-xl:px-1.5 max-xl:text-[0.8rem] max-md:px-0';
	const transportMainButtonClass =
		'h-11 min-w-[66px] rounded-full border-fg bg-fg px-4 text-bg max-xl:min-w-0 max-xl:px-1.5 max-xl:text-[0.8rem] max-md:px-0';
	const playerPanelClass =
		'grid h-full min-h-0 content-start gap-3 overflow-hidden rounded-3xl border border-outline bg-surface/90 p-[1.125rem] [[data-density=compact]_&]:gap-2 [[data-density=compact]_&]:p-2.5 [[data-density=spacious]_&]:gap-5 [[data-density=spacious]_&]:p-6';
	const panelTitleClass =
		'm-0 line-clamp-2 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]';
	const metricCardClass =
		'grid min-w-0 gap-1 overflow-hidden rounded-2xl border border-outline bg-surface-2/[0.42] p-2.5';

	function bottomPanelGridClass() {
		return oldSongs.length > 0
			? 'grid h-[clamp(132px,18vh,172px)] min-h-0 grid-cols-3 gap-3.5 overflow-hidden max-md:hidden'
			: 'grid h-[clamp(132px,18vh,172px)] min-h-0 grid-cols-2 gap-3.5 overflow-hidden max-md:hidden';
	}

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
			/* Tauri commands not available during browser-only dev */
		}
	}

	async function refreshPlaybackStatus() {
		try {
			const status = await invoke<PlaybackStatus>('get_playback_status');
			$playbackStatus = status;
			spotifyIsActive = false;
			routeError = '';
		} catch {
			/* Tauri commands not available during browser-only dev */
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

	function normalizedVolume() {
		return Number.isFinite($volume) ? $volume : 1;
	}

	function volumePercentLabel() {
		return Math.round(normalizedVolume() * 100);
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

<section
	class="relative grid h-full min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden max-md:grid-rows-[minmax(0,1fr)] max-md:gap-0"
	data-od-id="player-route"
>
	<section
		class="grid h-full min-h-0 grid-cols-[minmax(230px,0.42fr)_minmax(0,1fr)] items-stretch gap-6 overflow-hidden rounded-3xl border border-outline bg-[linear-gradient(145deg,oklch(22%_0.026_58_/_0.92),oklch(28%_0.035_58_/_0.58))] p-[clamp(18px,3vw,28px)] shadow-[0_26px_80px_oklch(0%_0_0_/_0.34)] max-xl:grid-cols-[minmax(210px,0.72fr)_minmax(0,1fr)] max-md:grid-cols-1 max-md:gap-3 max-md:rounded-none max-md:border-0 max-md:p-4 max-md:text-center"
		data-od-id="player-hero"
	>
		<NowPlaying song={$currentSong} status={$playbackStatus}>
			{#snippet children()}
				<div
					class="flex flex-wrap items-center gap-2 max-xl:grid max-xl:w-full max-xl:grid-cols-5 max-xl:gap-1.5"
					aria-label="Playback controls"
				>
					<Button
						class={transportButtonClass}
						onclick={() => runPlayerCommand('play_previous_queue_song')}
						disabled={oldSongs.length === 0}
						aria-label={t('transport.previous')}
					>
						<SkipBack class="size-4" aria-hidden="true" />
						<span class="max-md:sr-only">{t('transport.previous')}</span>
					</Button>
					<Button
						class={transportMainButtonClass}
						onclick={playRouteSelection}
						disabled={$currentSong.id === '' && upcomingSongs.length === 0}
						aria-label={t('transport.play')}
					>
						<Play class="size-4" aria-hidden="true" />
						<span class="max-md:sr-only">{t('transport.play')}</span>
					</Button>
					<Button
						class={transportButtonClass}
						onclick={() => runPlayerCommand('playback_pause')}
						disabled={!$playbackStatus?.playing}
						aria-label={t('transport.pause')}
					>
						<Pause class="size-4" aria-hidden="true" />
						<span class="max-md:sr-only">{t('transport.pause')}</span>
					</Button>
					<Button
						class={transportButtonClass}
						onclick={() => runPlayerCommand('playback_stop')}
						disabled={!$playbackStatus?.current_path}
						aria-label={t('transport.stop')}
					>
						<Square class="size-4" aria-hidden="true" />
						<span class="max-md:sr-only">{t('transport.stop')}</span>
					</Button>
					<Button
						class={transportButtonClass}
						onclick={() => runPlayerCommand('play_next_queue_song')}
						disabled={upcomingSongs.length === 0}
						aria-label={t('transport.next')}
					>
						<SkipForward class="size-4" aria-hidden="true" />
						<span class="max-md:sr-only">{t('transport.next')}</span>
					</Button>
				</div>
			{/snippet}
		</NowPlaying>

		<div
			class="grid h-full min-h-0 min-w-0 grid-rows-[1rem_auto_minmax(0,1fr)_auto_auto_auto] gap-3.5 overflow-hidden max-md:grid-rows-[1rem_auto_minmax(0,1fr)_auto_auto] max-md:gap-2.5"
		>
			<div class="h-4 overflow-hidden">
				{#if spotifyIsActive}
					<p class="m-0 font-mono text-xs tracking-widest text-soft uppercase">
						Spotify is playing
					</p>
				{/if}
			</div>
			<div class="flex min-h-6 flex-wrap gap-2 overflow-hidden">
				<StatusBadge>{formatSource($currentSong.source ?? 'local') || 'Local'}</StatusBadge>
				{#if $currentSong.quality}
					<StatusBadge state="partial">{$currentSong.quality}</StatusBadge>
				{/if}
				<StatusBadge state={$playbackStatus?.state ?? 'idle'}
					>{$playbackStatus?.state ?? 'Idle'}</StatusBadge
				>
			</div>

			<div class="min-h-0 min-w-0 overflow-hidden">
				<SpectrumAnalyzer showLabels={true} />
			</div>

			<div class="grid gap-2">
				<ProgressBar value={playbackProgress()} />
				<div class="flex justify-between font-mono text-xs text-soft">
					<span>{playbackTimeLabel()}</span>
					<span
						>{$playbackStatus?.output_sample_rate
							? formatSampleRate($playbackStatus.output_sample_rate)
							: 'Ready'}</span
					>
				</div>
			</div>

			<div class="grid w-full grid-cols-5 gap-1.5" aria-label="Playback controls">
				<Button
					class={transportButtonClass}
					onclick={() => runPlayerCommand('play_previous_queue_song')}
					disabled={oldSongs.length === 0}
					aria-label={t('transport.previous')}
				>
					<SkipBack class="size-4" aria-hidden="true" />
					<span class="max-md:sr-only">{t('transport.previous')}</span>
				</Button>
				<Button
					class={transportMainButtonClass}
					onclick={playRouteSelection}
					disabled={$currentSong.id === '' && upcomingSongs.length === 0}
					aria-label={t('transport.play')}
				>
					<Play class="size-4" aria-hidden="true" />
					<span class="max-md:sr-only">{t('transport.play')}</span>
				</Button>
				<Button
					class={transportButtonClass}
					onclick={() => runPlayerCommand('playback_pause')}
					disabled={!$playbackStatus?.playing}
					aria-label={t('transport.pause')}
				>
					<Pause class="size-4" aria-hidden="true" />
					<span class="max-md:sr-only">{t('transport.pause')}</span>
				</Button>
				<Button
					class={transportButtonClass}
					onclick={() => runPlayerCommand('playback_stop')}
					disabled={!$playbackStatus?.current_path}
					aria-label={t('transport.stop')}
				>
					<Square class="size-4" aria-hidden="true" />
					<span class="max-md:sr-only">{t('transport.stop')}</span>
				</Button>
				<Button
					class={transportButtonClass}
					onclick={() => runPlayerCommand('play_next_queue_song')}
					disabled={upcomingSongs.length === 0}
					aria-label={t('transport.next')}
				>
					<SkipForward class="size-4" aria-hidden="true" />
					<span class="max-md:sr-only">{t('transport.next')}</span>
				</Button>
			</div>

			<label class="grid gap-2 text-sm text-soft max-md:hidden">
				<span>{t('transport.volume')} {volumePercentLabel()}%</span>
				<Slider
					value={[volumePercentLabel()]}
					min={0}
					max={100}
					step={1}
					onValueChange={async (v: number[]) => {
						const val = v[0] / 100;
						$volume = val;
						try {
							await invoke('set_playback_volume', { volume: val });
						} catch {
							/* Browser-only dev */
						}
					}}
				/>
			</label>
		</div>
	</section>

	<div class={bottomPanelGridClass()}>
		<section class={playerPanelClass}>
			<div class="flex items-end justify-between gap-3">
				<div>
					<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">Queue</p>
					<h2 class={panelTitleClass}>Up next</h2>
				</div>
				<StatusBadge>{upcomingSongs.length} tracks</StatusBadge>
			</div>
			{#if upcomingSongs.length === 0}
				<p class="text-soft">Queue is empty</p>
			{:else}
				<ol class="m-0 grid min-h-0 list-none gap-2 overflow-hidden p-0">
					{#each upcomingSongs.slice(0, 5) as song, index}
						<li
							class="grid grid-cols-[auto_minmax(0,1fr)] items-center gap-3 border border-outline rounded-2xl p-2.5 bg-surface/78"
						>
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

		<section class={playerPanelClass}>
			<div class="flex items-end justify-between gap-3">
				<div class="min-w-0">
					<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">Output</p>
					<h2 class={panelTitleClass}>
						{$playbackStatus?.output_device_name ?? 'Default device'}
					</h2>
				</div>
				<StatusBadge state={$playbackStatus?.playing ? 'playing' : 'idle'}>
					{$playbackStatus?.playing ? 'Playing' : 'Idle'}
				</StatusBadge>
			</div>
			<div class="grid min-h-0 grid-cols-3 gap-2.5 overflow-hidden">
				<div class={metricCardClass}>
					<span class="font-mono text-xs uppercase text-soft">Source</span>
					<strong class="truncate"
						>{$currentSong.source ? formatSource($currentSong.source) : 'Local'}</strong
					>
				</div>
				<div class={metricCardClass}>
					<span class="font-mono text-xs uppercase text-soft">Quality</span>
					<strong class="truncate">{qualityDisplay()}</strong>
				</div>
				<div class={metricCardClass}>
					<span class="font-mono text-xs uppercase text-soft">ReplayGain</span>
					<strong class="truncate">{$playbackStatus?.replay_gain_mode ?? 'off'}</strong>
				</div>
			</div>
		</section>

		{#if oldSongs.length > 0}
			<section class={playerPanelClass}>
				<div class="flex items-end justify-between gap-3">
					<div>
						<p class="text-soft font-mono text-xs tracking-widest uppercase m-0">History</p>
						<h2 class={panelTitleClass}>Recent</h2>
					</div>
				</div>
				<ol class="m-0 grid min-h-0 list-none gap-2 overflow-hidden p-0">
					{#each oldSongs.slice(-4).reverse() as song}
						<li class={metricCardClass}>
							<strong class="truncate">{song.title}</strong>
							<span class="truncate text-soft">{queuedSongDetail(song)}</span>
						</li>
					{/each}
				</ol>
			</section>
		{/if}
	</div>

	{#if routeError}
		<p
			class="absolute top-4 right-4 z-10 max-w-[min(420px,calc(100%_-_2rem))] rounded-2xl border border-outline bg-danger/20 px-3 py-2 text-danger shadow-xl"
		>
			{routeError}
		</p>
	{/if}
</section>
