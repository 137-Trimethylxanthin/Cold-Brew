<script lang="ts">
	import { playbackStatus, currentSong } from '$lib/stores';
	import { formatSource, playbackQualityLabel, formatSampleRate, formatDb } from '$lib/playback';
	import type { Song } from '$lib/types';
	import { invoke } from '@tauri-apps/api/core';

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

<aside class="queue">
	<section class="desktop-player">
		<div class="cover-art" aria-hidden="true">
			{#if $currentSong.cover_art}
				<img src={$currentSong.cover_art} alt={`${$currentSong.title} album art`} />
			{:else}
				<div class="cover-gradient"></div>
			{/if}
		</div>
		<div class="track-title">
			<h2>{$currentSong.title}</h2>
			<p>{nowPlayingDetail()}</p>
		</div>
		<div class="quality-row">
			<span class="quality-pill">{formatSource($currentSong.source ?? 'local')}</span>
			{#if $currentSong.quality}
				<span class="quality-pill hires">{$currentSong.quality}</span>
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
				<span>{$playbackStatus?.state ?? 'idle'}</span>
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
						<button onclick={() => onRemove(song)}>Remove</button>
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

<style>
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

	.queue h3 {
		margin: 0 0 10px;
		font-size: 0.78rem;
		text-transform: uppercase;
		color: var(--muted);
	}

	.cover-art {
		position: relative;
		overflow: hidden;
		aspect-ratio: 1;
		border: 1px solid color-mix(in oklch, var(--border) 72%, transparent);
		border-radius: var(--radius-lg);
	}

	.cover-art img {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.cover-gradient {
		width: 100%;
		height: 100%;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 82%, transparent) 0 12%,
				transparent 13%
			),
			conic-gradient(from 235deg, var(--fg), var(--accent), var(--accent-2), var(--surface-2), var(--fg));
		box-shadow: 0 22px 50px color-mix(in oklch, black 20%, transparent);
	}

	.cover-gradient::before {
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
		color: var(--muted);
		font-size: 0.84rem;
		font-family: var(--font-mono);
		font-size: 0.76rem;
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
</style>
