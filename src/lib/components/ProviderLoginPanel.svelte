<script lang="ts">
	import type { ProviderLoginState } from '$lib/types';

	interface Props {
		providerId: string;
		providerName: string;
		description: string;
		loginState: ProviderLoginState | null;
		children: import('svelte').Snippet;
	}

	let { providerId, providerName, description, loginState, children }: Props = $props();

	function stateLabel(value: string) {
		return value
			.split('_')
			.map((part) => part.charAt(0).toUpperCase() + part.slice(1))
			.join(' ');
	}
</script>

<section class="grid gap-2.5 border border-outline rounded-[20px] p-3 bg-surface-2/[0.42]">
	<div class="flex max-lg:grid items-start justify-between gap-2.5">
		<div>
			<h3 class="m-0 text-[0.9rem]">{providerName}</h3>
			<p class="mt-0.5">{description}</p>
		</div>
		<span class="state-pill {loginState?.status ?? 'missing'}">
			{stateLabel(loginState?.status ?? 'missing')}
		</span>
	</div>

	{@render children?.()}
</section>
