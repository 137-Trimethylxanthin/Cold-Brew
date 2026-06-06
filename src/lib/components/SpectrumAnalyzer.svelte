<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	interface Props {
		bars?: number;
		height?: number;
	}

	let { bars = 28, height: defaultH = 54 }: Props = $props();

	const NOISE_FLOOR = 0.01;

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let current = $state<Float32Array>(new Float32Array(bars));
	let target = $state<Float32Array>(new Float32Array(bars));
	let unlisten: (() => void) | undefined;
	let ctx: CanvasRenderingContext2D | null;
	let bufW = 0;
	let bufH = 0;
	let time = 0;

	// Per-bar character — speed gets faster toward treble, bass has more weight
	const profiles = Array.from({ length: bars }, (_, i) => {
		const t = i / (bars - 1);
		const speed = 0.10 + t * 0.22;
		const boost = 3.2 - t * 2.6;
		const phase = (i / bars) * Math.PI * 2.1;
		return { speed, boost, phase };
	});

	// Build overlapping frequency windows so each bar has a WIDE range → more boom
	function buildWideFreqMap(totalBins: number, displayBars: number): [number, number][] {
		const windows: [number, number][] = [];
		for (let i = 0; i < displayBars; i++) {
			const center = Math.round(Math.pow(i / (displayBars - 1), 0.58) * (totalBins - 1));
			// Each window grabs bins around its center — wider in the middle, tighter at edges
			const width = Math.round(4 + (i / (displayBars - 1)) * 8);
			const start = Math.max(0, center - width);
			const end = Math.min(totalBins, center + width + 1);
			windows.push([start, end]);
		}
		return windows;
	}

	const freqWindows = buildWideFreqMap(64, bars);

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
				const [start, end] = freqWindows[i];
				let sum = 0;
				let count = 0;
				for (let j = start; j < end && j < raw.length; j++) {
					sum += raw[j] ?? 0;
					count++;
				}
				// Add a tiny bit of the neighbor window to avoid dead spots
				const neighborLeft = i > 0 ? raw[freqWindows[i - 1][1] - 1] ?? 0 : 0;
				sum += neighborLeft * 0.15;
				arr[i] = count > 0 ? sum / count : 0;
			}

			for (let i = 0; i < bars; i++) {
				arr[i] = Math.pow(arr[i] * profiles[i].boost, 0.50);
			}
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

		time += 0.016;

		const isSilent = target.every((v) => v < 0.004);
		const c = new Float32Array(bars);

		for (let i = 0; i < bars; i++) {
			let t = target[i];
			// Only add drift when completely silent — keep real bars clean
			if (isSilent) {
				t = 0.025 + 0.05 * Math.abs(Math.sin(time * 1.4 + profiles[i].phase));
			}
			c[i] = lerp(current[i], t, profiles[i].speed);
		}
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
