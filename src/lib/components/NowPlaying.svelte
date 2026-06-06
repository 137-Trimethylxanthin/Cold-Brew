<script lang="ts">
	import { formatSource, playbackQualityLabel } from '$lib/playback';
	import type { PlaybackStatus, Song } from '$lib/types';
	import { Maximize, X } from '@lucide/svelte';
	import { t } from '$lib/i18n';
	import SpectrumAnalyzer from '$lib/components/SpectrumAnalyzer.svelte';

	let { song, status, children }: { song: Song; status: PlaybackStatus | null; children?: import('svelte').Snippet } = $props();

	let fullscreen = $state(false);
	let showControls = $state(true);
	let hideTimer: ReturnType<typeof setTimeout> | null = null;

	function detail() {
		if (status?.current_path) return playbackQualityLabel(status);
		const parts: string[] = [];
		if (song.source) parts.push(formatSource(song.source));
		if (song.quality) parts.push(song.quality);
		return parts.join(' / ') || '\u2014';
	}

	function toggleFullscreen() {
		fullscreen = !fullscreen;
		if (fullscreen) {
			showControls = true;
			resetHideTimer();
			document.addEventListener('keydown', handleFullscreenKey);
		} else {
			clearHideTimer();
			document.removeEventListener('keydown', handleFullscreenKey);
		}
	}

	function handleFullscreenKey(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			fullscreen = false;
			clearHideTimer();
			document.removeEventListener('keydown', handleFullscreenKey);
		}
	}

	function resetHideTimer() {
		clearHideTimer();
		showControls = true;
		hideTimer = setTimeout(() => {
			showControls = false;
		}, 5000);
	}

	function clearHideTimer() {
		if (hideTimer) {
			clearTimeout(hideTimer);
			hideTimer = null;
		}
	}

	$effect(() => {
		return () => {
			clearHideTimer();
			document.removeEventListener('keydown', handleFullscreenKey);
		};
	});
</script>

<div class="grid gap-4" data-od-id="now-playing" aria-label="{t('common.now_playing')}">
	<div class="flex items-start justify-between gap-2">
		<div class="w-full max-w-[380px] aspect-square relative overflow-hidden border border-outline/70 rounded-3xl shadow-xl" aria-hidden="true">
			{#if song.cover_art}
				<img class="object-cover w-full h-full rounded-3xl" src={song.cover_art} alt={t('album.art', { title: song.title })} />
			{:else}
				<div class="hero-cover-placeholder w-full h-full"></div>
			{/if}
		</div>
		<button
			class="fullscreen-toggle"
			onclick={toggleFullscreen}
			aria-label="{t('common.fullscreen')}"
			title="{t('common.fullscreen')}"
		>
			<Maximize class="size-5" aria-hidden="true" />
		</button>
	</div>
	<div class="grid gap-1.5">
		<h1 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(36px,6vw,64px)] leading-[0.96]">
			{song.title}
		</h1>
		<p class="m-0 text-soft font-mono text-sm">{detail()}</p>
		{#if song.artist}
			<span class="text-soft text-sm">{song.artist}</span>
		{/if}
		{#if song.album}
			<span class="text-soft text-sm">{song.album}</span>
		{/if}
	</div>
	<SpectrumAnalyzer />
</div>

<!-- M20: Fullscreen Now-Playing Overlay -->
{#if fullscreen}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div
		class="fullscreen-now-playing"
		class:fullscreen-controls-hidden={!showControls}
		onmousemove={resetHideTimer}
		role="dialog"
		aria-label="{t('common.now_playing')}"
		aria-modal="true"
	>
		<div class="fullscreen-np-gradient"></div>
		<div class="fullscreen-np-content">
			<button class="fullscreen-np-exit" onclick={toggleFullscreen} aria-label="{t('common.exit_fullscreen')}">
				<X class="size-6" aria-hidden="true" />
			</button>
			<div class="fullscreen-np-center">
				<div class="fullscreen-np-cover">
					{#if song.cover_art}
						<img class="object-cover w-full h-full rounded-3xl shadow-2xl" src={song.cover_art} alt={t('album.art', { title: song.title })} />
					{:else}
						<div class="hero-cover-placeholder w-full h-full rounded-3xl"></div>
					{/if}
				</div>
				<div class="fullscreen-np-info">
					<h1 class="fullscreen-np-title">{song.title}</h1>
					<p class="fullscreen-np-detail">{detail()}</p>
					{#if song.artist}
						<p class="fullscreen-np-artist">{song.artist}</p>
					{/if}
					{#if song.album}
						<p class="fullscreen-np-album">{song.album}</p>
					{/if}
				</div>
				<div class="fullscreen-spectrum">
					<SpectrumAnalyzer />
				</div>
			</div>
			{#if children}
				<div class="fullscreen-np-transport">
					{@render children()}
				</div>
			{/if}
		</div>
	</div>
{/if}

<style>
	.fullscreen-toggle {
		min-height: 36px;
		min-width: 36px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid var(--color-outline);
		border-radius: 999px;
		background: oklch(22% 0.026 58 / 0.86);
		color: var(--color-soft);
		cursor: pointer;
		padding: 0.4rem;
		transition: color 0.15s ease, border-color 0.15s ease;
	}

	.fullscreen-toggle:hover,
	.fullscreen-toggle:focus-visible {
		color: var(--color-fg);
		border-color: var(--color-brand);
		outline: none;
	}

	.fullscreen-now-playing {
		position: fixed;
		inset: 0;
		z-index: 100;
		display: flex;
		align-items: center;
		justify-content: center;
		background: oklch(10% 0.015 60);
	}

	.fullscreen-np-gradient {
		position: absolute;
		inset: 0;
		background:
			radial-gradient(ellipse at 50% 30%, oklch(70% 0.13 205 / 0.12), transparent 60%),
			radial-gradient(ellipse at 20% 80%, oklch(28% 0.035 58 / 0.5), transparent 50%);
		pointer-events: none;
	}

	.fullscreen-np-content {
		position: relative;
		z-index: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 2rem;
		width: 100%;
		max-width: 720px;
		padding: 2rem;
		transition: opacity 0.5s ease;
	}

	.fullscreen-controls-hidden .fullscreen-np-exit,
	.fullscreen-controls-hidden .fullscreen-np-transport {
		opacity: 0;
		pointer-events: none;
	}

	.fullscreen-np-exit {
		position: absolute;
		top: -1rem;
		right: 0;
		min-height: 40px;
		min-width: 40px;
		display: flex;
		align-items: center;
		justify-content: center;
		border: 1px solid oklch(35% 0.032 60 / 0.6);
		border-radius: 999px;
		background: oklch(15% 0.02 60 / 0.8);
		color: var(--color-soft);
		cursor: pointer;
		transition: opacity 0.4s ease;
	}

	.fullscreen-np-exit:hover {
		color: var(--color-fg);
		border-color: var(--color-outline);
	}

	.fullscreen-np-center {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 1.5rem;
		text-align: center;
	}

	.fullscreen-np-cover {
		width: min(60vw, 420px);
		aspect-ratio: 1;
	}

	.fullscreen-np-title {
		margin: 0;
		font-family: var(--font-family-display);
		font-size: clamp(28px, 4vw, 48px);
		line-height: 1.05;
		color: var(--color-fg);
	}

	.fullscreen-np-detail {
		margin: 0.25rem 0 0;
		font-family: var(--font-family-mono);
		font-size: 0.8rem;
		color: var(--color-soft);
	}

	.fullscreen-np-artist,
	.fullscreen-np-album {
		margin: 0.15rem 0 0;
		font-size: 0.9rem;
		color: var(--color-soft);
	}

	.fullscreen-np-transport {
		display: flex;
		align-items: center;
		gap: 1rem;
		transition: opacity 0.4s ease;
	}

	.fullscreen-spectrum {
		width: 100%;
	}

	.fullscreen-spectrum :global(.spectrum-canvas) {
		height: 180px;
	}
</style>
