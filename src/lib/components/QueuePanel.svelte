<script lang="ts">
	import { playbackStatus, currentSong } from '$lib/stores';
	import { formatSource, playbackQualityLabel, formatSampleRate, formatDb } from '$lib/playback';
	import type { Song } from '$lib/types';
	import { invoke } from '@tauri-apps/api/core';
	import { Button } from '$lib/components/ui/button';

	let {
		upcomingSongs = [],
		oldSongs = [],
		onRemove = (_song: Song) => {},
		onMove = (_from: number, _to: number) => {}
	}: {
		upcomingSongs: Song[];
		oldSongs: Song[];
		onRemove: (song: Song) => void;
		onMove: (from: number, to: number) => void;
	} = $props();

	let draggedUpcomingIndex: number | null = $state(null);
	let dragOverUpcomingIndex: number | null = $state(null);

	const spectrumBars = [
		64, 36, 86, 48, 74, 42, 92, 58, 30, 80, 54, 70, 44, 96, 62, 34, 76, 50, 88, 46,
		68, 38, 82, 56, 72, 40, 90, 52
	] as const;

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
		if (!playbackStatus || !$playbackStatus) return durationLabel($currentSong.duration);
		const status = $playbackStatus;
		if (!status.current_path) return durationLabel($currentSong.duration);
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
		const song = $currentSong;
		return (
			songDetailLabel(song) || song.artist || song.album || '\u2014'
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

	function uniqueParts(parts: string[]) {
		return [...new Set(parts.filter(Boolean))];
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

	function dropQueuedSong(event: DragEvent, toIndex: number) {
		event.preventDefault();
		const transferIndex = Number(event.dataTransfer?.getData('text/plain'));
		const fromIndex =
			draggedUpcomingIndex ?? (Number.isInteger(transferIndex) ? transferIndex : null);
		endQueueDrag();
		if (fromIndex === null) return;
		onMove(fromIndex, toIndex);
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
</script>

<aside class="grid content-start gap-3.5 min-w-0 border border-border rounded-3xl p-[18px] overflow-auto bg-surface/92">
	<section class="grid gap-3.5 border border-border rounded-3xl p-4 bg-surface-2/[0.42]">
		<div class="relative overflow-hidden aspect-square border border-border/70 rounded-3xl shadow-lg" aria-hidden="true">
			{#if $currentSong.cover_art}
				<img class="object-cover w-full h-full" src={$currentSong.cover_art} alt={`${$currentSong.title} album art`} />
			{:else}
				<div class="cover-placeholder w-full h-full"></div>
			{/if}
		</div>
		<div class="grid gap-[5px] min-w-0">
			<h2 class="overflow-hidden m-0 font-[family-name:var(--font-family-display)] text-[clamp(24px,3vw,34px)] leading-tight truncate">
				{$currentSong.title}
			</h2>
			<p>{nowPlayingDetail()}</p>
		</div>
		<div class="flex flex-wrap gap-2">
			<span class="inline-flex items-center min-h-7 border border-border rounded-full px-2.5 text-muted font-mono text-[0.68rem] uppercase bg-surface/72">
				{formatSource($currentSong.source ?? 'local')}
			</span>
			{#if $currentSong.quality}
				<span class="inline-flex items-center min-h-7 border border-accent/40 rounded-full px-2.5 text-accent font-mono text-[0.68rem] uppercase bg-surface/72">
					{$currentSong.quality}
				</span>
			{/if}
		</div>
		<div class="grid grid-cols-[repeat(28,minmax(2px,1fr))] items-end gap-1 h-[54px]" aria-hidden="true">
			{#each spectrumBars as height}
				<span class="spectrum-bar" style="--bar-height: {height}%"></span>
			{/each}
		</div>
		<div class="grid gap-2">
			<div class="duration-bar" style="--progress: {playbackProgress()}%" aria-hidden="true"></div>
			<div class="flex justify-between font-mono text-[0.76rem] text-muted">
				<span>{playbackTimeLabel()}</span>
				<span>{$playbackStatus?.state ?? 'idle'}</span>
			</div>
		</div>
	</section>

	<section class="border border-border rounded-3xl p-4 bg-surface-2/[0.42]">
		<h3 class="m-0 mb-2.5 text-[0.78rem] uppercase text-muted">Up next</h3>
		{#if upcomingSongs.length === 0}
			<p>Queue is empty</p>
		{:else}
			<ol class="m-0 p-0 list-none">
				{#each upcomingSongs as song, index}
				{@const isDragging = draggedUpcomingIndex === index}
				{@const isDragOver = dragOverUpcomingIndex === index && draggedUpcomingIndex !== index}
				<li
				class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 mb-2 border rounded-[20px] p-2.5 cursor-grab bg-surface/76 border-border {isDragging ? 'opacity-55' : ''} {isDragOver ? 'border-accent bg-accent/14' : ''}"
				draggable="true"
				ondragstart={(event) => startQueueDrag(event, index)}
				ondragover={(event) => allowQueueDrop(event, index)}
				ondragleave={() => leaveQueueDrop(index)}
				ondrop={(event) => dropQueuedSong(event, index)}
				ondragend={endQueueDrag}
				>
						<span class="grid gap-px min-w-0">
							<strong class="truncate">{song.title}</strong>
							{#if queuedSongDetail(song)}
							<small class="truncate text-muted text-[0.76rem]">{queuedSongDetail(song)}</small>
							{/if}
							</span>
							<Button variant="ghost" size="sm" onclick={() => onRemove(song)}>Remove</Button>
							</li>
				{/each}
			</ol>
		{/if}
	</section>

	<section class="border border-border rounded-3xl p-4 bg-surface-2/[0.42]">
		<h3 class="m-0 mb-2.5 text-[0.78rem] uppercase text-muted">History</h3>
		<ol class="m-0 p-0 list-none">
			{#each oldSongs.slice(-4).reverse() as song}
				<li class="mb-2 min-w-0">
					<span class="grid gap-px min-w-0">
						<strong class="truncate">{song.title}</strong>
						{#if queuedSongDetail(song)}
							<small class="truncate text-muted text-[0.76rem]">{queuedSongDetail(song)}</small>
						{/if}
					</span>
				</li>
			{/each}
		</ol>
	</section>
</aside>
