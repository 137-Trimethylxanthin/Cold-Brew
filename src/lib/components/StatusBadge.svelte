<script lang="ts">
	import { Badge } from '$lib/components/ui/badge';
	import { cn } from '$lib/utils';

	interface Props {
		state?: string | null;
		label?: string;
		class?: string;
		children?: import('svelte').Snippet;
	}

	let { state = 'default', label, class: className, children }: Props = $props();

	function normalizedState() {
		return (state ?? 'default').toLowerCase();
	}

	function stateClass() {
		const value = normalizedState();
		if (
			['implemented', 'ready', 'link_out_only', 'valid', 'connected', 'playing'].includes(value)
		) {
			return 'border-success/45 text-success';
		}
		if (['partial', 'pending', 'paused'].includes(value)) {
			return 'border-brand/45 text-brand';
		}
		if (['failed', 'error', 'invalid'].includes(value)) {
			return 'border-danger/50 text-danger';
		}
		if (['researched', 'stale'].includes(value)) {
			return 'border-accent-2/50 text-accent-2';
		}
		return 'border-outline text-soft';
	}

	function stateLabel() {
		return normalizedState()
			.split('_')
			.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
			.join(' ');
	}
</script>

<Badge
	variant="outline"
	class={cn(
		'h-auto min-h-5 bg-surface-2/70 px-[0.45rem] py-[0.14rem] font-mono text-[0.72rem] leading-tight font-normal',
		stateClass(),
		className
	)}
>
	{#if children}
		{@render children()}
	{:else}
		{label ?? stateLabel()}
	{/if}
</Badge>
