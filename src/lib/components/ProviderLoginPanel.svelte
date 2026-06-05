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

<section class="login-panel">
	<div class="login-panel-header">
		<div>
			<h3>{providerName}</h3>
			<p>{description}</p>
		</div>
		<span class={`state ${loginState?.status ?? 'missing'}`}>
			{stateLabel(loginState?.status ?? 'missing')}
		</span>
	</div>

	{@render children?.()}
</section>

<style>
	.login-panel {
		display: grid;
		gap: 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius-md);
		background: color-mix(in oklch, var(--surface-2) 42%, transparent);
		padding: 12px;
	}

	.login-panel-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 10px;
	}

	.login-panel-header p {
		margin-top: 2px;
	}

	h3 {
		margin: 0;
		font-size: 0.9rem;
	}

	.state {
		display: inline-block;
		border-radius: 999px;
		border: 1px solid var(--border);
		background: color-mix(in oklch, var(--surface-2) 70%, transparent);
		color: var(--muted);
		font-family: var(--font-mono);
		font-size: 0.72rem;
		padding: 0.14rem 0.45rem;
		white-space: nowrap;
	}

	.state.implemented {
		border-color: color-mix(in oklch, var(--success) 44%, var(--border));
		color: var(--success);
	}

	.state.partial {
		border-color: color-mix(in oklch, var(--accent) 44%, var(--border));
		color: var(--accent);
	}

	.state.ready,
	.state.link_out_only {
		border-color: color-mix(in oklch, var(--success) 44%, var(--border));
		color: var(--success);
	}

	.state.missing {
		color: var(--muted);
	}

	.state.failed {
		border-color: color-mix(in oklch, var(--danger) 48%, var(--border));
		color: var(--danger);
	}

	.state.researched {
		border-color: color-mix(in oklch, var(--accent-2) 48%, var(--border));
		color: var(--accent-2);
	}

	@media (max-width: 880px) {
		.login-panel-header {
			display: grid;
		}
	}
</style>
