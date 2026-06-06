<script lang="ts">
	interface Props {
		value: number;
		class?: string;
	}

	let { value, class: className = '' }: Props = $props();
	const SEGMENT_COUNT = 64;
	const segments = Array.from({ length: SEGMENT_COUNT }, (_, index) => index);

	function clampedScale() {
		return Math.min(1, Math.max(0, value / 100));
	}

	function filledSegments() {
		return Math.round(clampedScale() * SEGMENT_COUNT);
	}
</script>

<div
	class={`grid h-2 grid-cols-[repeat(64,minmax(0,1fr))] overflow-hidden rounded-full bg-surface-3/70 ${className}`}
	role="progressbar"
	aria-valuemin="0"
	aria-valuemax="100"
	aria-valuenow={Math.round(Math.min(100, Math.max(0, value)))}
>
	{#each segments as segment}
		<span class={segment < filledSegments() ? 'bg-brand' : 'bg-transparent'}></span>
	{/each}
</div>
