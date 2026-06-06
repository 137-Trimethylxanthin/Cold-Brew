<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';
	import type { SpectrumData } from '$lib/types';

	const BAR_COUNT = 64;
	const SMOOTHING = 0.25;

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let currentBins = $state<Float32Array>(new Float32Array(BAR_COUNT));
	let targetBins = $state<Float32Array>(new Float32Array(BAR_COUNT));
	let isActive = $state(false);
	let unlisten: (() => void) | undefined;

	onMount(async () => {
		unlisten = await listen<SpectrumData>('spectrum_data', (event) => {
			const data = event.payload as { bins: number[] } | number[];
			const bins = Array.isArray(data) ? data : data.bins;
			if (Array.isArray(bins)) {
				targetBins = new Float32Array(bins);
				isActive = bins.some((v: number) => v > 0.01);
			}
		});
		drawLoop();
	});

	onDestroy(() => {
		unlisten?.();
		cancelAnimationFrame(animFrameId);
	});

	function lerp(a: number, b: number, t: number): number {
		return a + (b - a) * t;
	}

	function drawLoop() {
		if (!canvasEl) {
			animFrameId = requestAnimationFrame(drawLoop);
			return;
		}
		const ctx = canvasEl.getContext('2d');
		if (!ctx) {
			animFrameId = requestAnimationFrame(drawLoop);
			return;
		}

		const rect = canvasEl.getBoundingClientRect();
		if (rect.width === 0 || rect.height === 0) {
			animFrameId = requestAnimationFrame(drawLoop);
			return;
		}

		const w = rect.width * devicePixelRatio;
		const h = rect.height * devicePixelRatio;
		if (canvasEl.width !== w) canvasEl.width = w;
		if (canvasEl.height !== h) canvasEl.height = h;

		const newBins = new Float32Array(BAR_COUNT);
		const allSilent = targetBins.every((v) => v < 0.005);
		for (let i = 0; i < BAR_COUNT; i++) {
			const t = allSilent ? lerp(targetBins[i], 0.02, SMOOTHING * 0.5) : targetBins[i];
			newBins[i] = lerp(currentBins[i], t, SMOOTHING);
		}
		currentBins = newBins;
		isActive = !newBins.every((v) => v < 0.01);

		// Dark background
		ctx.fillStyle = 'oklch(13% 0.018 58)';
		ctx.fillRect(0, 0, w, h);

		const barW = (w / BAR_COUNT) * 0.78;
		const gap = (w / BAR_COUNT) * 0.22;
		const maxH = h * 0.88;
		const yBaseline = h * 0.06;

		// Gradient: brand blue → teal → warm cream
		const grad = ctx.createLinearGradient(0, h, 0, 0);
		grad.addColorStop(0.0, 'oklch(70% 0.13 205 / 0.55)');
		grad.addColorStop(0.4, 'oklch(70% 0.13 205 / 0.78)');
		grad.addColorStop(0.75, 'oklch(72% 0.09 190 / 0.85)');
		grad.addColorStop(1.0, 'oklch(93% 0.013 80 / 0.95)');
		ctx.fillStyle = grad;

		for (let i = 0; i < BAR_COUNT; i++) {
			const val = newBins[i];
			const barH = Math.max(devicePixelRatio, val * maxH);
			const x = i * (w / BAR_COUNT) + gap / 2;
			const y = h - yBaseline - barH;
			const radius = Math.min(barW / 2, 3 * devicePixelRatio);

			ctx.beginPath();
			ctx.moveTo(x + radius, y);
			ctx.lineTo(x + barW - radius, y);
			ctx.arcTo(x + barW, y, x + barW, y + radius, radius);
			ctx.lineTo(x + barW, h);
			ctx.lineTo(x, h);
			ctx.lineTo(x, y + radius);
			ctx.arcTo(x, y, x + radius, y, radius);
			ctx.fill();
		}

		// Subtle center reflection line
		ctx.fillStyle = 'oklch(93% 0.013 80 / 0.04)';
		ctx.fillRect(w * 0.25, 0, w * 0.5, h);

		animFrameId = requestAnimationFrame(drawLoop);
	}
</script>

<div class="spectrum-wrapper" class:spectrum-inactive={!isActive}>
	<span class="spectrum-label">Spectrum</span>
	<canvas
		bind:this={canvasEl}
		class="spectrum-canvas"
		role="img"
		aria-label="Real-time audio spectrum analyzer"
	></canvas>
	<span class="spectrum-freqs">
		<span>20Hz</span>
		<span>1k</span>
		<span>20kHz</span>
	</span>
</div>

<style>
	.spectrum-wrapper {
		position: relative;
		border: 1px solid var(--color-outline);
		border-radius: var(--radius-lg);
		background: oklch(13% 0.018 58);
		overflow: hidden;
		transition: opacity 350ms ease;
	}
	.spectrum-inactive {
		opacity: 0.55;
	}
	.spectrum-label {
		position: absolute;
		top: 8px;
		left: 14px;
		z-index: 2;
		font-family: var(--font-family-mono);
		font-size: 0.64rem;
		letter-spacing: 0.12em;
		text-transform: uppercase;
		color: oklch(68% 0.023 72 / 0.6);
	}
	.spectrum-canvas {
		display: block;
		width: 100%;
		height: 120px;
		image-rendering: auto;
		image-rendering: crisp-edges;
	}
	.spectrum-freqs {
		position: absolute;
		bottom: 6px;
		left: 14px;
		right: 14px;
		display: flex;
		justify-content: space-between;
		font-family: var(--font-family-mono);
		font-size: 0.58rem;
		color: oklch(68% 0.023 72 / 0.35);
		pointer-events: none;
	}
</style>
