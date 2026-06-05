<script lang="ts">
	import { goto } from '$app/navigation';
	import { playbackStatus } from '$lib/stores';
	import { formatSampleRate } from '$lib/playback';
	import { Library, Play, Settings } from '@lucide/svelte';
</script>

<nav class="sidenav" aria-label="Primary">
	<div class="brand-mark">
		<span class="brand-dot" aria-hidden="true"></span>
		<span>Cold Brew</span>
	</div>
	<p class="rail-kicker">Audiophile Player</p>
	<div class="rail-nav">
		<button onclick={() => goto('/')}><Library class="size-4 mr-2" /> Library</button>
		<button onclick={() => goto('/player')}><Play class="size-4 mr-2" /> Player</button>
		<button onclick={() => goto('/settings')}><Settings class="size-4 mr-2" /> Settings</button>
	</div>
	<section class="rail-status">
		<p class="eyebrow">Output</p>
		<strong>{$playbackStatus?.output_device_name ?? 'Default device'}</strong>
		<span>
			{$playbackStatus?.output_sample_rate
				? `${formatSampleRate($playbackStatus.output_sample_rate)} output`
				: 'Waiting for playback'}
		</span>
	</section>
</nav>

<style>
	.sidenav {
		display: grid;
		grid-template-rows: auto auto 1fr auto;
		align-content: start;
		gap: 20px;
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface) 92%, transparent);
		padding: 18px;
	}

	.brand-mark {
		display: inline-flex;
		align-items: center;
		gap: 10px;
		font-family: var(--font-display);
		font-size: 20px;
		font-weight: 700;
	}

	.brand-dot {
		width: 28px;
		height: 28px;
		border-radius: 10px;
		background:
			radial-gradient(
				circle at 50% 50%,
				color-mix(in oklch, var(--surface) 78%, transparent) 0 18%,
				transparent 19%
			),
			conic-gradient(from 210deg, var(--fg), var(--accent), var(--accent-2), var(--fg));
	}

	.rail-kicker {
		color: var(--muted);
		font-size: 0.84rem;
	}

	.eyebrow {
		margin: 0 0 8px;
		color: var(--accent);
		font-family: var(--font-mono);
		font-size: 0.68rem;
		letter-spacing: 0.08em;
		text-transform: uppercase;
	}

	.rail-nav {
		display: grid;
		gap: 8px;
		align-content: start;
	}

	.rail-nav button {
		display: flex;
		align-items: center;
		justify-content: flex-start;
		gap: 9px;
		min-height: 42px;
		padding: 0 12px;
		color: var(--muted);
		text-align: left;
	}

	.rail-nav button:hover,
	.rail-nav button:focus-visible {
		background: color-mix(in oklch, var(--accent) 11%, var(--surface));
		color: var(--fg);
	}

	.rail-status {
		min-width: 0;
		border: 1px solid var(--border);
		border-radius: var(--radius-lg);
		background: color-mix(in oklch, var(--surface-2) 56%, transparent);
		padding: 16px;
	}

	.rail-status strong {
		display: block;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.rail-status span {
		color: var(--muted);
		font-size: 0.84rem;
	}
</style>
