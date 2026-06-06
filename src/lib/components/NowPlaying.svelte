<script lang="ts">
	import { formatSource, playbackQualityLabel } from '$lib/playback';
	import type { PlaybackStatus, Song } from '$lib/types';
	import { Maximize, X } from '@lucide/svelte';
	import { t } from '$lib/i18n';
	import SpectrumAnalyzer from '$lib/components/SpectrumAnalyzer.svelte';
	import { Button } from '$lib/components/ui/button';

	let {
		song,
		status,
		children
	}: { song: Song; status: PlaybackStatus | null; children?: import('svelte').Snippet } = $props();

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

<div
	class="grid h-full min-h-0 grid-rows-[minmax(0,1fr)_auto] gap-4 overflow-hidden"
	data-od-id="now-playing"
	aria-label={t('common.now_playing')}
>
	<div class="relative min-h-0 overflow-hidden">
		<div
			class="mx-auto aspect-square h-full max-h-[420px] max-w-full overflow-hidden rounded-3xl border border-outline/70 shadow-xl"
			aria-hidden="true"
		>
			{#if song.cover_art}
				<img
					class="object-cover w-full h-full rounded-3xl"
					src={song.cover_art}
					alt={t('album.art', { title: song.title })}
				/>
			{:else}
				<div class="hero-cover-placeholder w-full h-full"></div>
			{/if}
		</div>
		<Button
			variant="outline"
			size="icon"
			class="absolute top-0 right-0 min-h-9 min-w-9 rounded-full border-outline bg-surface/86 p-1.5 text-soft hover:border-brand hover:text-fg"
			onclick={toggleFullscreen}
			aria-label={t('common.fullscreen')}
			title={t('common.fullscreen')}
		>
			<Maximize class="size-5" aria-hidden="true" />
		</Button>
	</div>
	<div class="grid min-h-[112px] content-start gap-1.5 overflow-hidden">
		<h1
			class="m-0 line-clamp-2 break-words font-[family-name:var(--font-family-display)] text-[clamp(34px,5vw,60px)] leading-[0.96]"
		>
			{song.title}
		</h1>
		<p class="m-0 truncate font-mono text-sm text-soft">{detail()}</p>
		{#if song.artist}
			<span class="truncate text-sm text-soft">{song.artist}</span>
		{/if}
		{#if song.album}
			<span class="truncate text-sm text-soft">{song.album}</span>
		{/if}
	</div>
</div>

<!-- M20: Fullscreen Now-Playing Overlay -->
{#if fullscreen}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div
		class="fixed inset-0 z-[100] flex items-center justify-center bg-[oklch(10%_0.015_60)]"
		onmousemove={resetHideTimer}
		role="dialog"
		aria-label={t('common.now_playing')}
		aria-modal="true"
	>
		<div
			class="pointer-events-none absolute inset-0 [background:radial-gradient(ellipse_at_50%_30%,oklch(70%_0.13_205_/_0.12),transparent_60%),radial-gradient(ellipse_at_20%_80%,oklch(28%_0.035_58_/_0.5),transparent_50%)]"
		></div>
		<div
			class="relative z-[1] flex w-full max-w-[720px] flex-col items-center gap-8 p-8 transition-opacity duration-500"
		>
			<Button
				variant="outline"
				size="icon"
				class={`absolute -top-4 right-0 min-h-10 min-w-10 rounded-full border-outline/60 bg-bg/80 text-soft transition-opacity duration-400 hover:border-outline hover:text-fg ${showControls ? '' : 'pointer-events-none opacity-0'}`}
				onclick={toggleFullscreen}
				aria-label={t('common.exit_fullscreen')}
			>
				<X class="size-6" aria-hidden="true" />
			</Button>
			<div class="flex flex-col items-center gap-6 text-center">
				<div class="aspect-square w-[min(60vw,420px)]">
					{#if song.cover_art}
						<img
							class="object-cover w-full h-full rounded-3xl shadow-2xl"
							src={song.cover_art}
							alt={t('album.art', { title: song.title })}
						/>
					{:else}
						<div class="hero-cover-placeholder w-full h-full rounded-3xl"></div>
					{/if}
				</div>
				<div>
					<h1
						class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(28px,4vw,48px)] leading-[1.05] text-fg"
					>
						{song.title}
					</h1>
					<p class="mt-1 mb-0 font-mono text-[0.8rem] text-soft">{detail()}</p>
					{#if song.artist}
						<p class="mt-1 mb-0 text-sm text-soft">{song.artist}</p>
					{/if}
					{#if song.album}
						<p class="mt-1 mb-0 text-sm text-soft">{song.album}</p>
					{/if}
				</div>
				<div class="h-[240px] w-full">
					<SpectrumAnalyzer bars={48} showLabels={true} class="h-[240px]" />
				</div>
			</div>
			{#if children}
				<div
					class={`flex items-center gap-4 transition-opacity duration-400 ${showControls ? '' : 'pointer-events-none opacity-0'}`}
				>
					{@render children()}
				</div>
			{/if}
		</div>
	</div>
{/if}
