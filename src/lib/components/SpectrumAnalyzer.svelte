<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	interface Props {
		bars?: number;
		height?: number;
	}

	let { bars = 28, height: defaultH = 54 }: Props = $props();

	const NOISE_FLOOR = 0.012;

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let current = $state<Float32Array>(new Float32Array(bars));
	let target = $state<Float32Array>(new Float32Array(bars));
	let unlisten: (() => void) | undefined;
	let ctx: CanvasRenderingContext2D | null;
	let bufW = 0;
	let bufH = 0;
	let time = 0;

	// ── Log frequency ranges per bar (Cava-style) ──
	const freqRanges = Array.from({ length: bars }, (_, i) => {
		const fLo = Math.round(20 * Math.pow(20000 / 20, i / bars));
		const fHi = Math.round(20 * Math.pow(20000 / 20, (i + 1) / bars));
		return fLo < 1000 ? `${fLo}` : `${(fLo / 1000).toFixed(fLo >= 10000 ? 0 : 1)}k`;
	});

	// Map 64 FFT bins → display bars using same log scale
	const freqMap: number[] = [];
	for (let i = 0; i <= bars; i++) {
		const f = 20 * Math.pow(20000 / 20, i / bars);
		freqMap.push(Math.round(((f - 20) / (20000 - 20)) * 63));
	}
	freqMap[0] = 0;
	freqMap[bars] = 64;

	const speeds = Array.from({ length: bars }, (_, i) => 0.08 + (i / (bars - 1)) * 0.14);

	function sizeCanvas() {
		if (!canvasEl) return;
		const rect = canvasEl.getBoundingClientRect();
		const dpr = devicePixelRatio || 1;
		const w = Math.round(rect.width * dpr);
		const h = Math.round(rect.height * dpr);
		if (w < 4 || h < 4) return;
		if (bufW === w && bufH === h) return;
		bufW = w;
		bufH = h;
		canvasEl.width = w;
		canvasEl.height = h;
	}

	onMount(() => {
		let cleanup = false;

		listen<{ bins: number[] }>('spectrum_data', (event) => {
			if (cleanup) return;
			const raw = event.payload.bins ?? event.payload;
			if (!Array.isArray(raw) || raw.length < 64) return;

			const arr = new Float32Array(bars);
			for (let i = 0; i < bars; i++) {
				const start = freqMap[i];
				const end = freqMap[i + 1];
				let sum = 0;
				let count = 0;
				for (let j = start; j < end && j < raw.length; j++) {
					sum += raw[j] ?? 0;
					count++;
				}
				arr[i] = count > 0 ? sum / count : 0;
			}
			for (let i = 0; i < bars; i++) arr[i] = Math.pow(arr[i], 0.55);
			target = arr;
		}).then((fn) => { unlisten = fn; });

		ctx = canvasEl?.getContext('2d') ?? null;
		sizeCanvas();

		const ro = new ResizeObserver(() => sizeCanvas());
		if (canvasEl?.parentElement) ro.observe(canvasEl.parentElement);

		draw();

		return () => { cleanup = true; ro.disconnect(); };
	});

	onDestroy(() => {
		unlisten?.();
		cancelAnimationFrame(animFrameId);
	});

	function lerp(a: number, b: number, t: number) { return a + (b - a) * t; }

	function draw() {
		if (!ctx || bufW < 4 || bufH < 4) {
			animFrameId = requestAnimationFrame(draw);
			return;
		}

		time += 0.016;

		const isSilent = target.every((v) => v < 0.004);
		const c = new Float32Array(bars);

		for (let i = 0; i < bars; i++) {
			let t = target[i];
			if (isSilent) t = 0.02 + 0.06 * Math.abs(Math.sin(time * 1.5 + i * 0.35));
			c[i] = lerp(current[i], t, speeds[i]);
		}
		current = c;

		const w = bufW;
		const h = bufH;
		const dpr = devicePixelRatio || 1;

		const labelH = Math.round(14 * dpr);
		const drawH = h - labelH;

		ctx.clearRect(0, 0, w, h);

		// Frequency labels
		ctx.font = `${Math.round(9 * dpr)}px "JetBrains Mono", monospace`;
		ctx.textAlign = 'center';
		ctx.textBaseline = 'top';
		ctx.fillStyle = 'oklch(68% 0.023 72 / 0.42)';

		for (const idx of [0, Math.floor(bars / 4), Math.floor(bars / 2), Math.floor(bars * 3 / 4), bars - 1]) {
			ctx.fillText(freqRanges[idx], ((idx + 0.5) / bars) * w, drawH + Math.round(2 * dpr));
		}

		const barW = (w / bars) * 0.70;
		const gap = (w / bars) * 0.30;
		const radius = Math.min(barW * 0.5, dpr * 4);
		const maxH = drawH * 0.92;

		const grad = ctx.createLinearGradient(0, drawH, 0, 0);
		grad.addColorStop(0.0, 'oklch(70% 0.13 205 / 0.28)');
		grad.addColorStop(0.25, 'oklch(70% 0.13 205 / 0.48)');
		grad.addColorStop(0.5, 'oklch(70% 0.13 205 / 0.70)');
		grad.addColorStop(0.75, 'oklch(72% 0.10 195 / 0.88)');
		grad.addColorStop(0.92, 'oklch(93% 0.013 80 / 0.95)');
		ctx.fillStyle = grad;

		for (let i = 0; i < bars; i++) {
			const val = c[i];
			if (val < NOISE_FLOOR && !isSilent) continue;
			const barH = Math.max(dpr * 2.5, val * maxH);
			const x = i * (w / bars) + gap / 2;
			const y = drawH - barH;

			ctx.beginPath();
			ctx.moveTo(x + radius, y);
			ctx.arcTo(x + barW, y, x + barW, y + radius, radius);
			ctx.lineTo(x + barW, drawH);
			ctx.lineTo(x, drawH);
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
		height: 100%;
	}
</style>
