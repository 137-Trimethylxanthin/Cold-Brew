<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { listen } from '@tauri-apps/api/event';

	interface Props {
		bars?: number;
		showLabels?: boolean;
		class?: string;
	}

	let { bars = 28, showLabels = false, class: className = '' }: Props = $props();

	const ACTIVE_BAR_FLOOR = 0.04;
	const ENERGY_GAMMA = 0.48;
	const DYNAMIC_GAIN_LIMIT = 6.5;
	const MUSIC_LABELS = [
		{ label: 'Sub', position: 0.08 },
		{ label: 'Bass', position: 0.27 },
		{ label: 'Body', position: 0.48 },
		{ label: 'Clear', position: 0.7 },
		{ label: 'Air', position: 0.91 }
	];

	let canvasEl: HTMLCanvasElement;
	let animFrameId: number;
	let current = $state<Float32Array>(new Float32Array());
	let target = $state<Float32Array>(new Float32Array());
	let unlisten: (() => void) | undefined;
	let ctx: CanvasRenderingContext2D | null;
	let bufW = 0;
	let bufH = 0;
	let time = 0;

	function barRatio(index: number) {
		return bars === 1 ? 0.5 : index / Math.max(1, bars - 1);
	}

	function responseSpeed(index: number, rising: boolean) {
		const ratio = barRatio(index);
		return rising ? 0.46 + ratio * 0.14 : 0.24 + ratio * 0.12;
	}

	// Backend bins are already ordered low to high, so weighted index resampling keeps every bar populated.
	function resampleBins(raw: number[]) {
		const arr = new Float32Array(bars);
		const sourceBins = raw.length;

		for (let i = 0; i < bars; i++) {
			const start = (i * sourceBins) / bars;
			const end = ((i + 1) * sourceBins) / bars;
			const first = Math.floor(start);
			const last = Math.min(sourceBins - 1, Math.ceil(end) - 1);
			let weightedSum = 0;
			let totalWeight = 0;

			for (let j = first; j <= last; j++) {
				const overlap = Math.min(end, j + 1) - Math.max(start, j);
				if (overlap <= 0) continue;
				weightedSum += (raw[j] ?? 0) * overlap;
				totalWeight += overlap;
			}

			arr[i] = totalWeight > 0 ? weightedSum / totalWeight : 0;
		}

		return arr;
	}

	function shapeBars(rawBars: Float32Array) {
		const compressed = new Float32Array(bars);
		const mixed = new Float32Array(bars);
		const shaped = new Float32Array(bars);
		let average = 0;
		let mixedPeak = 0;

		for (let i = 0; i < bars; i++) {
			const value = Math.pow(Math.max(0, rawBars[i]), ENERGY_GAMMA);
			compressed[i] = value;
			average += value;
		}

		average /= bars;

		for (let i = 0; i < bars; i++) {
			const left = compressed[Math.max(0, i - 1)];
			const right = compressed[Math.min(bars - 1, i + 1)];
			const neighbor = Math.max(left, right);
			const position = barRatio(i);
			const contour = 0.9 + 0.1 * Math.sin(position * Math.PI);
			const value = (compressed[i] * 0.72 + neighbor * 0.18 + average * 0.1) * contour;
			mixed[i] = value;
			mixedPeak = Math.max(mixedPeak, value);
		}

		if (mixedPeak < 0.001) return mixed;

		const gain = Math.min(DYNAMIC_GAIN_LIMIT, 1 / mixedPeak);
		for (let i = 0; i < bars; i++) {
			shaped[i] = Math.min(1, Math.max(ACTIVE_BAR_FLOOR, mixed[i] * gain));
		}

		return shaped;
	}

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
		current = new Float32Array(bars);
		target = new Float32Array(bars);

		listen<{ bins: number[] }>('spectrum_data', (event) => {
			if (cleanup) return;
			const raw = event.payload.bins ?? event.payload;
			if (!Array.isArray(raw) || raw.length < 64) return;

			target = shapeBars(resampleBins(raw));
		})
			.then((fn) => {
				unlisten = fn;
			})
			.catch(() => {
				/* Tauri events are unavailable during browser-only visual review. */
			});

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

	function lerp(a: number, b: number, t: number) {
		return a + (b - a) * t;
	}

	function drawLabels(width: number, drawHeight: number, dpr: number) {
		if (!ctx) return;

		const labelBandH = Math.min(drawHeight * 0.28, 20 * dpr);
		const labelTop = drawHeight - labelBandH;
		const fade = ctx.createLinearGradient(0, labelTop, 0, drawHeight);
		fade.addColorStop(0, 'oklch(15% 0.02 60 / 0)');
		fade.addColorStop(0.55, 'oklch(15% 0.02 60 / 0.42)');
		fade.addColorStop(1, 'oklch(15% 0.02 60 / 0.72)');
		ctx.fillStyle = fade;
		ctx.fillRect(0, labelTop, width, labelBandH);

		ctx.font = `${Math.round(9.5 * dpr)}px "JetBrains Mono", monospace`;
		ctx.textAlign = 'center';
		ctx.textBaseline = 'bottom';
		ctx.lineWidth = Math.max(1, dpr);

		for (const item of MUSIC_LABELS) {
			const x = Math.round(item.position * width);
			ctx.strokeStyle = 'oklch(70% 0.13 205 / 0.28)';
			ctx.beginPath();
			ctx.moveTo(x, labelTop + 3 * dpr);
			ctx.lineTo(x, drawHeight - 13 * dpr);
			ctx.stroke();

			ctx.fillStyle = 'oklch(93% 0.013 80 / 0.68)';
			ctx.fillText(item.label, x, drawHeight - 2 * dpr);
		}
	}

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
			c[i] = lerp(current[i], t, responseSpeed(i, t > current[i]));
		}
		current = c;

		const w = bufW;
		const h = bufH;
		const dpr = devicePixelRatio || 1;

		const drawH = h;

		ctx.clearRect(0, 0, w, h);

		const slotW = w / bars;
		const barW = slotW * 0.86;
		const gap = slotW - barW;
		const radius = Math.min(barW * 0.5, dpr * 4);
		const maxH = drawH;

		const grad = ctx.createLinearGradient(0, drawH, 0, 0);
		grad.addColorStop(0.0, 'oklch(70% 0.13 205 / 0.28)');
		grad.addColorStop(0.25, 'oklch(70% 0.13 205 / 0.48)');
		grad.addColorStop(0.5, 'oklch(70% 0.13 205 / 0.70)');
		grad.addColorStop(0.75, 'oklch(72% 0.10 195 / 0.88)');
		grad.addColorStop(0.92, 'oklch(93% 0.013 80 / 0.95)');
		ctx.fillStyle = grad;

		for (let i = 0; i < bars; i++) {
			const val = isSilent ? c[i] : Math.max(c[i], ACTIVE_BAR_FLOOR);
			const barH = Math.max(dpr * 2.5, val * maxH);
			const x = i * slotW + gap / 2;
			const y = drawH - barH;

			ctx.beginPath();
			ctx.moveTo(x + radius, y);
			ctx.arcTo(x + barW, y, x + barW, y + radius, radius);
			ctx.lineTo(x + barW, drawH);
			ctx.lineTo(x, drawH);
			ctx.lineTo(x, y + radius);
			ctx.arcTo(x, y, x + radius, y, radius);
			ctx.fill();

			if (!isSilent && barH > dpr * 10) {
				ctx.strokeStyle = `oklch(93% 0.013 80 / ${Math.min(0.62, 0.16 + val * 0.38)})`;
				ctx.lineWidth = Math.max(dpr * 1.2, Math.min(barW * 0.18, dpr * 2.5));
				ctx.lineCap = 'round';
				ctx.beginPath();
				ctx.moveTo(x + radius, y + ctx.lineWidth / 2);
				ctx.lineTo(x + barW - radius, y + ctx.lineWidth / 2);
				ctx.stroke();
			}
		}

		if (showLabels) drawLabels(w, drawH, dpr);

		animFrameId = requestAnimationFrame(draw);
	}
</script>

<canvas
	bind:this={canvasEl}
	class={`block h-full min-h-[72px] w-full ${className}`}
	aria-hidden="true"
></canvas>
