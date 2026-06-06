<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import type { LevelData } from '$lib/types';
	import { levelData } from '$lib/stores';

	const SMOOTH_RMS = 0.25;
	const PEAK_DECAY = 0.92;

	let animFrameId: number;
	let smoothLeftRms = $state(0);
	let smoothRightRms = $state(0);
	let leftPeakHold = $state(0);
	let rightPeakHold = $state(0);
	let leftPeakTimer = $state(0);
	let rightPeakTimer = $state(0);
	let unlisten: (() => void) | undefined;

	let latest: LevelData = { left_peak: 0, right_peak: 0, left_rms: 0, right_rms: 0 };

	onMount(async () => {
		unlisten = await listen<LevelData>('level_data', (event) => {
			latest = event.payload;
		});
		drawLoop();
	});

	onDestroy(() => {
		if (unlisten) unlisten();
		if (animFrameId) cancelAnimationFrame(animFrameId);
	});

	function lerp(a: number, b: number, t: number) {
		return a + (b - a) * t;
	}

	function barColor(value: number): string {
		if (value > 0.85) return '#ef4444';
		if (value > 0.7) return '#eab308';
		return '#22c55e';
	}

	function drawLoop() {
		smoothLeftRms = lerp(smoothLeftRms, latest.left_rms, SMOOTH_RMS);
		smoothRightRms = lerp(smoothRightRms, latest.right_rms, SMOOTH_RMS);

		// Peak hold logic
		if (latest.left_peak > leftPeakHold) {
			leftPeakHold = latest.left_peak;
			leftPeakTimer = 40;
		} else if (leftPeakTimer > 0) {
			leftPeakTimer -= 1;
		} else {
			leftPeakHold *= PEAK_DECAY;
		}

		if (latest.right_peak > rightPeakHold) {
			rightPeakHold = latest.right_peak;
			rightPeakTimer = 40;
		} else if (rightPeakTimer > 0) {
			rightPeakTimer -= 1;
		} else {
			rightPeakHold *= PEAK_DECAY;
		}

		$levelData = {
			left_peak: leftPeakHold,
			right_peak: rightPeakHold,
			left_rms: smoothLeftRms,
			right_rms: smoothRightRms
		};

		animFrameId = requestAnimationFrame(drawLoop);
	}

	function barStyle(channel: 'left' | 'right', type: 'rms' | 'peak') {
		const value = channel === 'left'
			? (type === 'rms' ? smoothLeftRms : leftPeakHold)
			: (type === 'rms' ? smoothRightRms : rightPeakHold);
		const heightPct = Math.max(1, value * 100);
		const color = type === 'peak' ? '#ef4444' : barColor(value);
		return `height: ${heightPct}%; background: ${color};`;
	}
</script>

<div class="vu-meter" aria-label="VU meter" role="img">
	<div class="vu-channel">
		<span class="vu-label">L</span>
		<div class="vu-bar-container">
			<div class="vu-bar" style={barStyle('left', 'rms')}></div>
			<div class="vu-peak-dot" style="bottom: {Math.max(0, leftPeakHold * 100)}%;"></div>
		</div>
	</div>
	<div class="vu-channel">
		<span class="vu-label">R</span>
		<div class="vu-bar-container">
			<div class="vu-bar" style={barStyle('right', 'rms')}></div>
			<div class="vu-peak-dot" style="bottom: {Math.max(0, rightPeakHold * 100)}%;"></div>
		</div>
	</div>
</div>

<style>
	.vu-meter {
		display: flex;
		gap: 3px;
		align-items: flex-end;
		height: 40px;
		padding: 2px 0;
	}

	.vu-channel {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2px;
		height: 100%;
		flex: 1;
		max-width: 12px;
	}

	.vu-label {
		font-family: var(--font-mono, monospace);
		font-size: 0.5rem;
		color: var(--color-soft);
		line-height: 1;
	}

	.vu-bar-container {
		flex: 1;
		width: 100%;
		background: oklch(15% 0.015 60);
		border-radius: 3px;
		position: relative;
		overflow: hidden;
	}

	.vu-bar {
		position: absolute;
		bottom: 0;
		left: 0;
		right: 0;
		border-radius: 3px;
		transition: height 0.05s linear;
	}

	.vu-peak-dot {
		position: absolute;
		left: 0;
		right: 0;
		height: 3px;
		background: #ef4444;
		border-radius: 1px;
		transition: bottom 0.08s ease-out;
	}
</style>
