<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	const BARS = 28;
	const SMOOTH = 0.22;

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let current = $state<Float32Array>(new Float32Array(BARS));
	let target = $state<Float32Array>(new Float32Array(BARS));
	let unlisten: (() => void) | undefined;

	onMount(async () => {
		unlisten = await listen<{ bins: number[] }>('spectrum_data', (event) => {
			const raw = event.payload.bins ?? event.payload;
			if (!Array.isArray(raw) || raw.length === 0) return;
			// Downsample 64 bins → 28 bars
			const step = 64 / BARS;
			const arr = new Float32Array(BARS);
			for (let i = 0; i < BARS; i++) {
				const start = Math.floor(i * step);
				const end = Math.floor((i + 1) * step);
				let sum = 0;
				for (let j = start; j < end && j < raw.length; j++) sum += raw[j];
				arr[i] = sum / (end - start);
			}
			target = arr;
		});
		draw();
	});

	onDestroy(() => {
		unlisten?.();
		cancelAnimationFrame(animFrameId);
	});

	function lerp(a: number, b: number, t: number) { return a + (b - a) * t; }

	function draw() {
		if (!canvasEl) { animFrameId = requestAnimationFrame(draw); return; }
		const ctx = canvasEl.getContext('2d');
		if (!ctx) { animFrameId = requestAnimationFrame(draw); return; }

		const rect = canvasEl.getBoundingClientRect();
		if (rect.width < 4 || rect.height < 4) { animFrameId = requestAnimationFrame(draw); return; }

		const dpr = devicePixelRatio || 1;
		const w = rect.width * dpr;
		const h = rect.height * dpr;
		if (canvasEl.width !== w) canvasEl.width = w;
		if (canvasEl.height !== h) canvasEl.height = h;

		// Smooth interpolation
		const c = new Float32Array(BARS);
		for (let i = 0; i < BARS; i++) c[i] = lerp(current[i], target[i], SMOOTH);
		current = c;

		// Clear — transparent background so parent bg shows through
		ctx.clearRect(0, 0, w, h);

		const barW = (w / BARS) * 0.65;
		const gap = (w / BARS) * 0.35;
		const radius = barW * 0.48; // nearly round tops
		const maxH = h * 0.92;
		const bottom = h;

		ctx.fillStyle = 'oklch(70% 0.13 205 / 0.72)';

		for (let i = 0; i < BARS; i++) {
			const val = c[i];
			const barH = Math.max(dpr * 3, val * maxH);
			const x = i * (w / BARS) + gap / 2;
			const y = bottom - barH;

			ctx.beginPath();
			ctx.moveTo(x + radius, y);
			ctx.arcTo(x + barW, y, x + barW, y + radius, radius);
			ctx.lineTo(x + barW, bottom);
			ctx.lineTo(x, bottom);
			ctx.lineTo(x, y + radius);
			ctx.arcTo(x, y, x + radius, y, radius);
			ctx.fill();
		}

		animFrameId = requestAnimationFrame(draw);
	}
</script>

<canvas
	bind:this={canvasEl}
	class="spec"
	role="img"
	aria-label="Audio spectrum"
></canvas>

<style>
	.spec {
		display: block;
		width: 100%;
		height: 54px;
		image-rendering: auto;
	}
</style>
