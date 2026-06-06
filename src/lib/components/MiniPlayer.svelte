<script lang="ts">
	import {
		playbackStatus,
		currentSong,
		volume,
		playbackSettings,
		sleepTimerState,
		abRepeatState
	} from '$lib/stores';
	import { playbackQualityLabel } from '$lib/playback';
	import type { AbRepeatState, SleepTimerState, Song } from '$lib/types';
	import {
		SkipBack,
		Play,
		Pause,
		Square,
		SkipForward,
		Minimize2,
		Maximize2,
		Timer,
		RotateCcw
	} from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Slider } from '$lib/components/ui/slider';
	import { t } from '$lib/i18n';
	import { invoke } from '@tauri-apps/api/core';
	import VuMeter from '$lib/components/VuMeter.svelte';
	import ProgressBar from '$lib/components/ProgressBar.svelte';

	let {
		onPlayPrevious = () => {},
		onResume = () => {},
		onPause = () => {},
		onStop = () => {},
		onPlayNext = () => {},
		onVolumeChange = (_event: Event) => {},
		onSpeedChange = (_speed: number) => {},
		canPlay = false,
		isPlaying = false,
		isPauseEnabled = false,
		isStopEnabled = false,
		canPrev = false,
		canNext = false
	}: {
		onPlayPrevious: () => void;
		onResume: () => void;
		onPause: () => void;
		onStop: () => void;
		onPlayNext: () => void;
		onVolumeChange: (event: Event) => void;
		onSpeedChange?: (speed: number) => void;
		canPlay: boolean;
		isPlaying: boolean;
		isPauseEnabled: boolean;
		isStopEnabled: boolean;
		canPrev: boolean;
		canNext: boolean;
	} = $props();

	const SPEED_OPTIONS = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];
	let showSpeedMenu = $state(false);
	let showSleepMenu = $state(false);

	const SLEEP_OPTIONS = [
		{ label: 'Off', value: null },
		{ label: '15 min', value: 15 },
		{ label: '30 min', value: 30 },
		{ label: '45 min', value: 45 },
		{ label: '60 min', value: 60 },
		{ label: '90 min', value: 90 },
		{ label: '120 min', value: 120 }
	];

	const COMPACT_KEY = 'coldbrew.miniplayer.compact';

	let isCompact = $state(
		typeof localStorage !== 'undefined' ? localStorage.getItem(COMPACT_KEY) === 'true' : false
	);

	function toggleCompact() {
		isCompact = !isCompact;
		if (typeof localStorage !== 'undefined') {
			localStorage.setItem(COMPACT_KEY, String(isCompact));
		}
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
		const status = $playbackStatus;
		if (!status?.current_path) return durationLabel($currentSong.duration);
		return status.duration_ms
			? `${formatMilliseconds(status.position_ms)} / ${formatMilliseconds(status.duration_ms)}`
			: formatMilliseconds(status.position_ms);
	}

	function playbackProgress() {
		const status = $playbackStatus;
		if (!status?.current_path || !status.duration_ms) return 0;
		return Math.min(100, (status.position_ms / status.duration_ms) * 100);
	}

	function nowPlayingDetail() {
		const status = $playbackStatus;
		if (status?.current_path) return playbackQualityLabel(status);
		return '\u2014';
	}

	async function setSleepTimer(minutes: number | null) {
		showSleepMenu = false;
		try {
			const state = await invoke<SleepTimerState>('set_sleep_timer', { minutes });
			sleepTimerState.set(state);
		} catch (err) {
			console.error('Sleep timer error:', err);
		}
	}

	function sleepTimerLabel() {
		const s = $sleepTimerState;
		if (!s.active || !s.remaining_seconds) return '';
		const mins = Math.floor(s.remaining_seconds / 60);
		const secs = s.remaining_seconds % 60;
		return `${mins}:${secs.toString().padStart(2, '0')}`;
	}

	async function setAbRepeatA() {
		try {
			const state = await invoke<AbRepeatState>('set_ab_repeat_a', { positionSecs: null });
			abRepeatState.set(state);
		} catch (err) {
			console.error('AB Repeat A error:', err);
		}
	}

	async function setAbRepeatB() {
		try {
			const state = await invoke<AbRepeatState>('set_ab_repeat_b', { positionSecs: null });
			abRepeatState.set(state);
		} catch (err) {
			console.error('AB Repeat B error:', err);
		}
	}

	async function clearAbRepeat() {
		try {
			const state = await invoke<AbRepeatState>('clear_ab_repeat');
			abRepeatState.set(state);
		} catch (err) {
			console.error('AB Repeat clear error:', err);
		}
	}

	function abRepeatLabel() {
		const s = $abRepeatState;
		if (!s.loop_start_secs && !s.loop_end_secs) return '';
		if (s.active) return 'AB';
		if (s.loop_start_secs && !s.loop_end_secs) return 'A...';
		return 'AB';
	}
</script>

{#if isCompact}
	<div
		class="fixed right-5 bottom-4 left-5 z-10 flex items-center gap-3 overflow-hidden rounded-2xl border border-outline bg-surface p-2 pr-4"
		data-od-id="mini-player-compact"
	>
		<div class="flex items-center gap-3 min-w-0 flex-1">
			<div
				class="w-10 h-10 relative overflow-hidden border border-outline rounded-lg shrink-0"
				aria-hidden="true"
			>
				{#if $currentSong.cover_art}
					<img
						class="object-cover w-full h-full"
						src={$currentSong.cover_art}
						alt={t('album.art', { title: $currentSong.title })}
					/>
				{:else}
					<div class="cover-placeholder w-full h-full [&::before]:hidden"></div>
				{/if}
			</div>
			<div class="grid gap-0.5 min-w-0" aria-live="polite" aria-atomic="true">
				<strong class="truncate text-sm">{$currentSong.title}</strong>
				<span class="truncate text-soft text-xs">{nowPlayingDetail()}</span>
			</div>
		</div>

		<div class="flex items-center gap-1.5">
			<Button
				size="icon-sm"
				variant="ghost"
				onclick={onPlayPrevious}
				disabled={!canPrev}
				aria-label={t('transport.previous')}><SkipBack class="size-4" /></Button
			>
			<Button
				size="icon-sm"
				variant="ghost"
				onclick={onResume}
				disabled={!canPlay || isPlaying}
				aria-label={t('transport.play')}><Play class="size-4" /></Button
			>
			<Button
				size="icon-sm"
				variant="ghost"
				onclick={onPause}
				disabled={!isPauseEnabled}
				aria-label={t('transport.pause')}><Pause class="size-4" /></Button
			>
			<Button
				size="icon-sm"
				variant="ghost"
				onclick={onStop}
				disabled={!isStopEnabled}
				aria-label={t('transport.stop')}><Square class="size-4" /></Button
			>
			<Button
				size="icon-sm"
				variant="ghost"
				onclick={onPlayNext}
				disabled={!canNext}
				aria-label={t('transport.next')}><SkipForward class="size-4" /></Button
			>
		</div>

		<ProgressBar value={playbackProgress()} class="min-w-[120px] flex-1" />

		<Button
			size="icon-sm"
			variant="ghost"
			onclick={toggleCompact}
			aria-label={t('transport.expand')}
			title={t('transport.expand')}
		>
			<Maximize2 class="size-4" />
		</Button>
	</div>
{:else}
	<div
		class="fixed right-5 bottom-4 left-5 z-10 grid grid-cols-[minmax(220px,360px)_auto_minmax(160px,1fr)_auto_minmax(130px,180px)] items-center gap-[1.125rem] rounded-3xl border border-border bg-surface/94 p-3 shadow-2xl backdrop-blur max-xl:grid-cols-[minmax(190px,280px)_auto_minmax(120px,1fr)_auto] max-md:right-0 max-md:left-0 max-md:bottom-[calc(56px+env(safe-area-inset-bottom,0px))] max-md:grid-cols-[minmax(0,1fr)_auto] max-md:gap-2.5 max-md:rounded-none max-md:border-x-0"
		data-od-id="mini-player"
	>
		<div class="flex items-center gap-3 min-w-0 max-md:col-span-full">
			<div
				class="w-12 h-12 relative overflow-hidden border border-outline rounded-xl"
				aria-hidden="true"
			>
				{#if $currentSong.cover_art}
					<img
						class="object-cover w-full h-full"
						src={$currentSong.cover_art}
						alt={t('album.art', { title: $currentSong.title })}
					/>
				{:else}
					<div class="cover-placeholder w-full h-full [&::before]:hidden"></div>
				{/if}
			</div>
			<div class="grid gap-0.5 min-w-0" aria-live="polite" aria-atomic="true">
				<strong class="truncate">{$currentSong.title}</strong>
				<span class="truncate text-soft text-sm">{nowPlayingDetail()}</span>
			</div>
		</div>
		<span class="text-soft font-mono tabular-nums whitespace-nowrap max-md:hidden"
			>{playbackTimeLabel()}</span
		>
		<ProgressBar value={playbackProgress()} class="max-md:hidden" />
		<div class="flex justify-center items-center gap-2 max-md:col-span-full max-md:justify-between">
			<Button
				size="icon"
				variant="ghost"
				onclick={onPlayPrevious}
				disabled={!canPrev}
				aria-label={t('transport.previous')}><SkipBack class="size-5" /></Button
			>
			<Button
				size="icon"
				variant="ghost"
				onclick={onResume}
				disabled={!canPlay || isPlaying}
				aria-label={t('transport.play')}><Play class="size-5" /></Button
			>
			<Button
				size="icon"
				variant="ghost"
				onclick={onPause}
				disabled={!isPauseEnabled}
				aria-label={t('transport.pause')}><Pause class="size-5" /></Button
			>
			<Button
				size="icon"
				variant="ghost"
				onclick={onStop}
				disabled={!isStopEnabled}
				aria-label={t('transport.stop')}><Square class="size-5" /></Button
			>
			<Button
				size="icon"
				variant="ghost"
				onclick={onPlayNext}
				disabled={!canNext}
				aria-label={t('transport.next')}><SkipForward class="size-5" /></Button
			>
			<div class="relative">
				<Button
					size="icon"
					variant="ghost"
					onclick={() => (showSpeedMenu = !showSpeedMenu)}
					aria-label={t('transport.speed')}
					class="font-mono text-xs"
				>
					{$playbackSettings.playback_speed}&times;
				</Button>
				{#if showSpeedMenu}
					<div
						class="absolute bottom-full mb-2 right-0 bg-surface border border-outline rounded-xl shadow-xl p-1 z-50 min-w-[80px]"
					>
						{#each SPEED_OPTIONS as speed}
							<Button
								variant="ghost"
								size="sm"
								class={`h-7 w-full justify-start rounded-lg px-3 py-1.5 text-left text-sm ${$playbackSettings.playback_speed === speed ? 'text-primary font-semibold' : 'text-soft'}`}
								onclick={() => {
									onSpeedChange(speed);
									showSpeedMenu = false;
								}}
							>
								{speed}&times;
							</Button>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<!-- AB Repeat buttons -->
		<div class="flex items-center gap-1">
			<Button
				size="icon"
				variant={$abRepeatState.loop_start_secs ? 'default' : 'ghost'}
				onclick={setAbRepeatA}
				aria-label="Set A marker"
				class="font-mono text-xs">A</Button
			>
			<Button
				size="icon"
				variant={$abRepeatState.loop_end_secs ? 'default' : 'ghost'}
				onclick={setAbRepeatB}
				aria-label="Set B marker"
				class="font-mono text-xs">B</Button
			>
			{#if $abRepeatState.active}
				<Button size="icon" variant="ghost" onclick={clearAbRepeat} aria-label="Clear AB repeat"
					><RotateCcw class="size-4" /></Button
				>
			{/if}
		</div>

		<label class="grid gap-2 text-soft text-sm max-xl:hidden">
			<span>{t('transport.volume')} {$volume}</span>
			<Slider
				value={[$volume * 100]}
				min={0}
				max={100}
				step={1}
				onValueChange={(v: number[]) => volume.set(v[0] / 100)}
			/>
		</label>
		<div class="flex items-end px-1">
			<VuMeter />
		</div>
		<Button
			size="icon"
			variant="ghost"
			onclick={toggleCompact}
			aria-label={t('transport.compact')}
			title={t('transport.compact')}
			class="self-end"
		>
			<Minimize2 class="size-4" />
		</Button>

		<!-- Sleep timer -->
		<div class="relative self-end">
			<Button
				size="icon"
				variant={$sleepTimerState.active ? 'default' : 'ghost'}
				onclick={() => (showSleepMenu = !showSleepMenu)}
				aria-label="Sleep timer"
				class="font-mono text-xs"
			>
				{#if $sleepTimerState.active && sleepTimerLabel()}
					{sleepTimerLabel()}
				{:else}
					<Timer class="size-4" />
				{/if}
			</Button>
			{#if showSleepMenu}
				<div
					class="absolute bottom-full mb-2 right-0 bg-surface border border-outline rounded-xl shadow-xl p-1 z-50 min-w-[100px]"
				>
					{#each SLEEP_OPTIONS as opt}
						<Button
							variant="ghost"
							size="sm"
							class="h-7 w-full justify-start rounded-lg px-3 py-1.5 text-left text-sm text-soft"
							onclick={() => setSleepTimer(opt.value as number | null)}
						>
							{opt.label}
						</Button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
{/if}
