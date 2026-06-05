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

<div class="now-playing-hero">
	<div class="hero-cover" aria-hidden="true"></div>
	<div class="hero-info">
		<h1>{song.title}</h1>
		<p>{detail()}</p>
		{#if song.artist}
			<span class="hero-artist">{song.artist}</span>
		{/if}
		{#if song.album}
			<span class="hero-album">{song.album}</span>
		{/if}
	</div>
</div>

<style>
	.now-playing-hero {
		display: grid;
		gap: 18px;
	}

	.hero-cover {
		aspect-ratio: 1;
		max-width: 380px;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		position: relative;
		overflow: hidden;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 82%, transparent) 0 12%,
				transparent 13%
			),
			conic-gradient(from 235deg, var(--fg), var(--accent), var(--accent-2), var(--surface-2), var(--fg));
		box-shadow: var(--shadow);
	}

	.hero-cover::before {
		content: '';
		position: absolute;
		inset: 8%;
		border: 1px solid color-mix(in oklch, var(--surface) 52%, transparent);
		border-radius: inherit;
	}

	.hero-info {
		display: grid;
		gap: 6px;
	}

	.hero-info h1 {
		margin: 0;
		font-family: var(--font-display);
		font-size: clamp(36px, 6vw, 64px);
		line-height: 0.96;
	}

	.hero-info p {
		margin: 0;
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.82rem;
	}

	.hero-artist,
	.hero-album {
		color: var(--muted);
		font-size: 0.9rem;
	}
</style>
