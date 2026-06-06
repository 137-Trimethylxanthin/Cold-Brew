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
	const METER_SEGMENTS = 12;
	const meterSegments = Array.from({ length: METER_SEGMENTS }, (_, index) => index);

	let latest: LevelData = { left_peak: 0, right_peak: 0, left_rms: 0, right_rms: 0 };

	onMount(async () => {
		try {
			unlisten = await listen<LevelData>('level_data', (event) => {
				latest = event.payload;
			});
		} catch {
			/* Tauri events are unavailable during browser-only visual review. */
		}
		drawLoop();
	});

	onDestroy(() => {
		if (unlisten) unlisten();
		if (animFrameId) cancelAnimationFrame(animFrameId);
	});

	function lerp(a: number, b: number, t: number) {
		return a + (b - a) * t;
	}

	function segmentThreshold(index: number) {
		return (METER_SEGMENTS - index) / METER_SEGMENTS;
	}

	function isSegmentActive(value: number, index: number) {
		return value >= segmentThreshold(index);
	}

	function isPeakSegment(value: number, index: number) {
		const clamped = Math.max(0, Math.min(0.999, value));
		return index === METER_SEGMENTS - 1 - Math.floor(clamped * METER_SEGMENTS);
	}

	function segmentColor(index: number) {
		const threshold = segmentThreshold(index);
		if (threshold > 0.85) return 'bg-danger';
		if (threshold > 0.7) return 'bg-accent-2';
		return 'bg-success';
	}

	function segmentClass(value: number, peak: number, index: number) {
		if (isPeakSegment(peak, index)) return 'bg-danger';
		return isSegmentActive(value, index) ? segmentColor(index) : 'bg-surface-3/35';
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
</script>

<div class="flex h-10 items-end gap-[3px] py-0.5" aria-label="VU meter" role="img">
	<div class="flex h-full max-w-3 flex-1 flex-col items-center gap-0.5">
		<span class="font-mono text-[0.5rem] leading-none text-soft">L</span>
		<div class="grid w-full flex-1 grid-rows-12 gap-px overflow-hidden rounded-[3px] bg-bg p-px">
			{#each meterSegments as segment}
				<span
					class={`rounded-[1px] transition-colors duration-75 ${segmentClass(smoothLeftRms, leftPeakHold, segment)}`}
				></span>
			{/each}
		</div>
	</div>
	<div class="flex h-full max-w-3 flex-1 flex-col items-center gap-0.5">
		<span class="font-mono text-[0.5rem] leading-none text-soft">R</span>
		<div class="grid w-full flex-1 grid-rows-12 gap-px overflow-hidden rounded-[3px] bg-bg p-px">
			{#each meterSegments as segment}
				<span
					class={`rounded-[1px] transition-colors duration-75 ${segmentClass(smoothRightRms, rightPeakHold, segment)}`}
				></span>
			{/each}
		</div>
	</div>
</div>
