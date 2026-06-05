<script lang="ts">
	import { goto } from '$app/navigation';
	import { playbackStatus } from '$lib/stores';
	import { formatSampleRate } from '$lib/playback';
	import { Library, Play, Settings } from '@lucide/svelte';
</script>

<nav
	class="grid grid-rows-[auto_auto_1fr_auto] content-start gap-5 min-w-0 border border-border rounded-3xl p-[18px] bg-surface/92"
	aria-label="Primary"
>
	<div class="inline-flex items-center gap-2.5 font-[family-name:var(--font-family-display)] text-xl font-bold">
		<span class="brand-dot-bg w-7 h-7 rounded-[10px]" aria-hidden="true"></span>
		<span>Cold Brew</span>
	</div>
	<p class="text-muted text-sm">Audiophile Player</p>
	<div class="grid gap-2 content-start">
		<button
			onclick={() => goto('/')}
			class="flex items-center justify-start gap-[9px] min-h-[42px] px-3 text-muted text-left hover:bg-accent/10 hover:text-fg focus-visible:bg-accent/10 focus-visible:text-fg"
		>
			<Library class="size-4 mr-2" /> Library
		</button>
		<button
			onclick={() => goto('/player')}
			class="flex items-center justify-start gap-[9px] min-h-[42px] px-3 text-muted text-left hover:bg-accent/10 hover:text-fg focus-visible:bg-accent/10 focus-visible:text-fg"
		>
			<Play class="size-4 mr-2" /> Player
		</button>
		<button
			onclick={() => goto('/settings')}
			class="flex items-center justify-start gap-[9px] min-h-[42px] px-3 text-muted text-left hover:bg-accent/10 hover:text-fg focus-visible:bg-accent/10 focus-visible:text-fg"
		>
			<Settings class="size-4 mr-2" /> Settings
		</button>
	</div>
	<section class="min-w-0 border border-border rounded-3xl p-4 bg-surface-2/56">
		<p class="m-0 mb-2 text-accent font-mono text-[0.68rem] tracking-widest uppercase">Output</p>
		<strong class="block truncate">{$playbackStatus?.output_device_name ?? 'Default device'}</strong>
		<span class="text-muted text-sm">
			{$playbackStatus?.output_sample_rate
				? `${formatSampleRate($playbackStatus.output_sample_rate)} output`
				: 'Waiting for playback'}
		</span>
	</section>
</nav>
