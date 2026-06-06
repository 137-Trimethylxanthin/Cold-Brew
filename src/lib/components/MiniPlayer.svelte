<script lang="ts">
	import { playbackStatus, currentSong, volume } from '$lib/stores';
	import { playbackQualityLabel } from '$lib/playback';
	import type { Song } from '$lib/types';
	import { SkipBack, Play, Pause, Square, SkipForward } from '@lucide/svelte';
	import { Button } from '$lib/components/ui/button';
	import { Slider } from '$lib/components/ui/slider';

	let {
		onPlayPrevious = () => {},
		onResume = () => {},
		onPause = () => {},
		onStop = () => {},
		onPlayNext = () => {},
		onVolumeChange = (_event: Event) => {},
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
		canPlay: boolean;
		isPlaying: boolean;
		isPauseEnabled: boolean;
		isStopEnabled: boolean;
		canPrev: boolean;
		canNext: boolean;
	} = $props();

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
</script>

<div class="miniplayer" data-od-id="mini-player">
	<div class="flex items-center gap-3 min-w-0">
		<div class="w-12 h-12 relative overflow-hidden border border-outline rounded-[14px]" aria-hidden="true">
			{#if $currentSong.cover_art}
				<img class="object-cover w-full h-full" src={$currentSong.cover_art} alt={`${$currentSong.title} album art`} />
			{:else}
				<div class="cover-placeholder w-full h-full [&::before]:hidden"></div>
			{/if}
		</div>
		<div class="grid gap-0.5 min-w-0">
			<strong class="truncate">{$currentSong.title}</strong>
			<span class="truncate text-soft text-sm">{nowPlayingDetail()}</span>
		</div>
	</div>
	<span class="text-soft font-mono tabular-nums whitespace-nowrap">{playbackTimeLabel()}</span>
	<div class="duration-bar" style="--progress: {playbackProgress()}%" aria-hidden="true"></div>
	<div class="flex justify-center items-center gap-2">
		<Button size="icon" variant="ghost" onclick={onPlayPrevious} disabled={!canPrev} aria-label="Previous track"><SkipBack class="size-5" /></Button>
		<Button size="icon" variant="ghost" onclick={onResume} disabled={!canPlay || isPlaying} aria-label="Play"><Play class="size-5" /></Button>
		<Button size="icon" variant="ghost" onclick={onPause} disabled={!isPauseEnabled} aria-label="Pause"><Pause class="size-5" /></Button>
		<Button size="icon" variant="ghost" onclick={onStop} disabled={!isStopEnabled} aria-label="Stop"><Square class="size-5" /></Button>
		<Button size="icon" variant="ghost" onclick={onPlayNext} disabled={!canNext} aria-label="Next track"><SkipForward class="size-5" /></Button>
	</div>
	<label class="grid gap-[3px] text-soft text-[0.78rem]">
		<span>Volume {$volume}</span>
		<Slider value={[$volume * 100]} min={0} max={100} step={1} onValueChange={(v: number[]) => volume.set(v[0] / 100)} />
	</label>
</div>
