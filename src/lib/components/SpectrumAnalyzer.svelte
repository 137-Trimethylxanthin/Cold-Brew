<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import { type SpectrumData } from '$lib/types';
	import { spectrumData } from '$lib/stores';

	const BAR_COUNT = 64;
	const SMOOTHING = 0.3;

	let canvas: HTMLCanvasElement;
	let animFrameId: number;
	let currentBins = $state<Float32Array>(new Float32Array(BAR_COUNT));
	let targetBins = $state<Float32Array>(new Float32Array(BAR_COUNT));
	let unlisten: (() => void) | undefined;

	onMount(async () => {
		unlisten = await listen<SpectrumData>('spectrum_data', (event) => {
			const bins = event.payload;
			if (Array.isArray(bins)) {
				targetBins = new Float32Array(bins);
			}
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

	function drawLoop() {
		if (!canvas) {
			animFrameId = requestAnimationFrame(drawLoop);
			return;
		}

		const ctx = canvas.getContext('2d');
		if (!ctx) {
			animFrameId = requestAnimationFrame(drawLoop);
			return;
		}

		const { width, height } = canvas;
		const dpr = window.devicePixelRatio || 1;
		canvas.width = width * dpr;
		canvas.height = height * dpr;
		ctx.scale(dpr, dpr);

		const newBins = new Float32Array(BAR_COUNT);
		for (let i = 0; i < BAR_COUNT; i++) {
			newBins[i] = lerp(currentBins[i], targetBins[i], SMOOTHING);
		}
		currentBins = newBins;

		ctx.clearRect(0, 0, width, height);

		const barWidth = width / BAR_COUNT;
		const barGap = barWidth * 0.18;
		const barDrawWidth = barWidth - barGap;

		// Gradient from deep blue to cyan to magenta
		const grad = ctx.createLinearGradient(0, height, 0, 0);
		grad.addColorStop(0, '#3b82f6');
		grad.addColorStop(0.5, '#06b6d4');
		grad.addColorStop(1, '#d946ef');
		ctx.fillStyle = grad;

		for (let i = 0; i < BAR_COUNT; i++) {
			const value = newBins[i];
			const barHeight = Math.max(1, value * height * 0.95);
			const x = i * barWidth + barGap / 2;
			const y = height - barHeight;

			ctx.fillRect(x, y, barDrawWidth, barHeight);
		}

		// Dim overlay when all silent
		if (newBins.every((v) => v < 0.01)) {
			ctx.fillStyle = 'rgba(0, 0, 0, 0.3)';
			ctx.fillRect(0, 0, width, height);
		}

		$spectrumData = Array.from(newBins);
		animFrameId = requestAnimationFrame(drawLoop);
	}
</script>

<canvas
	bind:this={canvas}
	class="spectrum-canvas"
	aria-label="Spectrum analyzer"
></canvas>

<style>
	.spectrum-canvas {
		display: block;
		width: 100%;
		height: 120px;
		border-radius: 0.75rem;
		background: oklch(12% 0.015 60);
	}
</style>
