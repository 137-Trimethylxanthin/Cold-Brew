<script lang="ts">
	import { formatSource, playbackQualityLabel } from '$lib/playback';
	import type { PlaybackStatus, Song } from '$lib/types';

	let { song, status }: { song: Song; status: PlaybackStatus | null } = $props();

	function detail() {
		if (status?.current_path) return playbackQualityLabel(status);
		const parts: string[] = [];
		if (song.source) parts.push(formatSource(song.source));
		if (song.quality) parts.push(song.quality);
		return parts.join(' / ') || '\u2014';
	}
</script>

<div class="grid gap-4" data-od-id="now-playing">
	<div class="w-full max-w-[380px] aspect-square relative overflow-hidden border border-outline/70 rounded-3xl shadow-xl" aria-hidden="true">
		{#if song.cover_art}
			<img class="object-cover w-full h-full rounded-3xl" src={song.cover_art} alt={`${song.title} album art`} />
		{:else}
			<div class="hero-cover-placeholder w-full h-full"></div>
		{/if}
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
</div>
