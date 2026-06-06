<script lang="ts">
	import StatusBadge from '$lib/components/StatusBadge.svelte';
	import type { ProviderLoginState } from '$lib/types';

	interface Props {
		providerId: string;
		providerName: string;
		description: string;
		loginState: ProviderLoginState | null;
		children: import('svelte').Snippet;
	}

	let { providerId, providerName, description, loginState, children }: Props = $props();
</script>

<section class="grid gap-2.5 border border-outline rounded-2xl p-3 bg-surface-2/[0.42]">
	<div class="flex max-lg:grid items-start justify-between gap-2.5">
		<div>
			<h3 class="m-0 text-sm">{providerName}</h3>
			<p class="mt-0.5">{description}</p>
		</div>
		<StatusBadge state={loginState?.status ?? 'missing'} />
	</div>

	{@render children?.()}
</section>
