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

	// ── Per-bar reactivity profile ──
	// Each bar has its own smoothing speed, boost, and phase offset
	// so bars move at different rates → organic "funky" look
	const profiles = Array.from({ length: bars }, (_, i) => {
		const t = i / (bars - 1);
		// Bass bars (left): fast response, high boost, big movement
		// Treble bars (right): slower response, less boost, shimmer
		const speed = 0.08 + t * 0.32;          // 0.08 (fast bass) → 0.40 (slow treble)
		const boost = 2.8 - t * 2.0;             // 2.8× (bass) → 0.8× (treble)
		const phase = (i / bars) * Math.PI * 1.7; // phase offset for staggered movement
		return { speed, boost, phase };
	});

	function buildFreqMap(totalBins: number, displayBars: number): number[] {
		const map = new Array<number>(displayBars);
		for (let i = 0; i < displayBars; i++) {
			// Aggressive exponential — first 8 bars cover bass alone
			map[i] = Math.round(Math.pow(i / (displayBars - 1), 0.35) * (totalBins - 1));
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

			// Apply per-bar boost + heavy compression
			for (let i = 0; i < bars; i++) {
				arr[i] = Math.pow(arr[i] * profiles[i].boost, 0.55);
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

		const c = new Float32Array(bars);
		const anyActive = target.some((v) => v > 0.005);

		for (let i = 0; i < bars; i++) {
			let t = target[i];
			// When silent, add subtle ambient drift so bars don't sit dead
			if (!anyActive) {
				t = 0.03 + 0.04 * Math.abs(Math.sin(time * 1.3 + profiles[i].phase));
			}
			c[i] = lerp(current[i], t, profiles[i].speed);
		}
		current = c;

		const w = bufW;
		const h = bufH;
		const dpr = devicePixelRatio || 1;

		ctx.clearRect(0, 0, w, h);

		const barW = (w / bars) * 0.7;
		const gap = (w / bars) * 0.3;
		const radius = Math.min(barW * 0.5, dpr * 4);
		const maxH = h * 0.94;

		// Vivid gradient: brand blue → bright cyan → warm cream peaks
		const grad = ctx.createLinearGradient(0, h, 0, 0);
		grad.addColorStop(0.0, 'oklch(70% 0.13 205 / 0.30)');
		grad.addColorStop(0.25, 'oklch(70% 0.13 205 / 0.50)');
		grad.addColorStop(0.5, 'oklch(70% 0.13 205 / 0.72)');
		grad.addColorStop(0.75, 'oklch(72% 0.10 195 / 0.88)');
		grad.addColorStop(0.92, 'oklch(93% 0.013 80 / 0.95)');
		ctx.fillStyle = grad;

		for (let i = 0; i < bars; i++) {
			const val = c[i];
			if (val < NOISE_FLOOR && anyActive) continue;
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
