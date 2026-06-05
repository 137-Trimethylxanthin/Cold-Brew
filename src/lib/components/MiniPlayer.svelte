<script lang="ts">
	import { playbackStatus, currentSong, volume } from '$lib/stores';
	import { playbackQualityLabel } from '$lib/playback';
	import type { Song } from '$lib/types';
	import { SkipBack, Play, Pause, Square, SkipForward } from '@lucide/svelte';

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

<div class="miniPlayer">
	<div class="now-playing">
		<div class="cover" aria-hidden="true"></div>
		<div>
			<strong>{$currentSong.title}</strong>
			<span>{nowPlayingDetail()}</span>
		</div>
	</div>
	<span class="time">{playbackTimeLabel()}</span>
	<div class="durationBar" style={`--progress: ${playbackProgress()}%`} aria-hidden="true"></div>
	<div class="transport">
		<button onclick={onPlayPrevious} disabled={!canPrev}>
			<SkipBack class="size-4" /> Prev
		</button>
		<button onclick={onResume} disabled={!canPlay || isPlaying}>
			<Play class="size-4" /> Play
		</button>
		<button onclick={onPause} disabled={!isPauseEnabled}>
			<Pause class="size-4" /> Pause
		</button>
		<button onclick={onStop} disabled={!isStopEnabled}>
			<Square class="size-4" /> Stop
		</button>
		<button onclick={onPlayNext} disabled={!canNext}>
			<SkipForward class="size-4" /> Next
		</button>
	</div>
	<label class="volume">
		<span>Volume</span>
		<input
			type="range"
			min="0"
			max="1"
			step="0.01"
			value={$volume}
			oninput={onVolumeChange}
			aria-label="Playback volume"
		/>
	</label>
</div>

<style>
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

	.now-playing span {
		color: var(--muted);
		font-size: 0.84rem;
	}

	.cover {
		width: 48px;
		height: 48px;
		border: 1px solid var(--border);
		border-radius: 14px;
		position: relative;
		overflow: hidden;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 82%, transparent) 0 12%,
				transparent 13%
			),
			conic-gradient(from 235deg, var(--fg), var(--accent), var(--accent-2), var(--surface-2), var(--fg));
	}

	.cover::before {
		content: '';
		position: absolute;
		inset: 8%;
		border: 1px solid color-mix(in oklch, var(--surface) 52%, transparent);
		border-radius: inherit;
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
		.miniPlayer {
			grid-template-columns: minmax(190px, 280px) auto minmax(120px, 1fr) auto;
		}

		.volume {
			display: none;
		}
	}

	@media (max-width: 880px) {
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
