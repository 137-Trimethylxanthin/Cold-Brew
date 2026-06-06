<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	interface Props {
		bars?: number;
		height?: number;
	}

	let { bars = 28, height: defaultH = 54 }: Props = $props();

	const SMOOTH = 0.18;
	const NOISE_FLOOR = 0.008;

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let current = $state<Float32Array>(new Float32Array(bars));
	let target = $state<Float32Array>(new Float32Array(bars));
	let unlisten: (() => void) | undefined;
	let ctx: CanvasRenderingContext2D | null;
	let bufW = 0;
	let bufH = 0;

	function buildFreqMap(totalBins: number, displayBars: number): number[] {
		const map = new Array<number>(displayBars);
		for (let i = 0; i < displayBars; i++) {
			map[i] = Math.round(Math.pow(i / (displayBars - 1), 0.48) * (totalBins - 1));
		}
		return map;
	}

	const freqMap = buildFreqMap(64, bars);

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
			for (let i = 0; i < bars - 1; i++) {
				const start = freqMap[i];
				const end = freqMap[i + 1] ?? 64;
				let sum = 0;
				const count = end - start;
				for (let j = start; j < end && j < raw.length; j++) sum += raw[j] ?? 0;
				arr[i] = count > 0 ? sum / count : 0;
			}
			const lastStart = freqMap[bars - 1] ?? 56;
			let lastSum = 0;
			for (let j = lastStart; j < raw.length; j++) lastSum += raw[j] ?? 0;
			arr[bars - 1] = (raw.length - lastStart) > 0 ? lastSum / (raw.length - lastStart) : 0;

			for (let i = 0; i < bars; i++) arr[i] = Math.pow(arr[i], 0.62);
			target = arr;
		}).then((fn) => { unlisten = fn; });

		ctx = canvasEl?.getContext('2d') ?? null;
		sizeCanvas();

		const ro = new ResizeObserver(() => sizeCanvas());
		if (canvasEl?.parentElement) ro.observe(canvasEl.parentElement);

		draw();

		return () => {
			cleanup = true;
			ro.disconnect();
		};
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

		const c = new Float32Array(bars);
		for (let i = 0; i < bars; i++) c[i] = lerp(current[i], target[i], SMOOTH);
		current = c;

		const w = bufW;
		const h = bufH;
		const dpr = devicePixelRatio || 1;

		ctx.clearRect(0, 0, w, h);

		const barW = (w / bars) * 0.72;
		const gap = (w / bars) * 0.28;
		const radius = Math.min(barW * 0.5, dpr * 4);
		const maxH = h * 0.94;

		const grad = ctx.createLinearGradient(0, h, 0, 0);
		grad.addColorStop(0.0, 'oklch(70% 0.13 205 / 0.36)');
		grad.addColorStop(0.3, 'oklch(70% 0.13 205 / 0.58)');
		grad.addColorStop(0.6, 'oklch(70% 0.13 205 / 0.74)');
		grad.addColorStop(0.85, 'oklch(70% 0.13 205 / 0.88)');
		grad.addColorStop(1.0, 'oklch(93% 0.013 80 / 0.9)');
		ctx.fillStyle = grad;

		for (let i = 0; i < bars; i++) {
			const val = c[i];
			if (val < NOISE_FLOOR) continue;
			const barH = Math.max(dpr * 2, val * maxH);
			const x = i * (w / bars) + gap / 2;
			const y = h - barH;

			ctx.beginPath();
			ctx.moveTo(x + radius, y);
			ctx.arcTo(x + barW, y, x + barW, y + radius, radius);
			ctx.lineTo(x + barW, h);
			ctx.lineTo(x, h);
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
