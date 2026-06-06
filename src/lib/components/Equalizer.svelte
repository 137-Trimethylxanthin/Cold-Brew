<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { onMount } from 'svelte';
	import type { EqPreset, EqState } from '$lib/types';
	import { eqState } from '$lib/stores';
	import { Button } from '$lib/components/ui/button';
	import { Slider } from '$lib/components/ui/slider';

	const EQ_BAND_LABELS = ['32 Hz', '64 Hz', '125 Hz', '250 Hz', '500 Hz', '1 kHz', '2 kHz', '4 kHz', '8 kHz', '16 kHz'];

	let presets = $state<EqPreset[]>([]);
	let message = $state('');
	let error = $state('');

	onMount(async () => {
		await loadEqState();
		await loadPresets();
	});

	async function loadEqState() {
		try {
			const state = await invoke<EqState>('get_eq_state');
			eqState.set(state);
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function loadPresets() {
		try {
			presets = await invoke<EqPreset[]>('list_eq_presets');
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function setBand(index: number, value: number) {
		error = ''; message = '';
		try {
			const state = await invoke<EqState>('set_eq_band', { index, gainDb: value });
			eqState.set(state);
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	async function applyPreset(name: string) {
		error = ''; message = '';
		try {
			const state = await invoke<EqState>('set_eq_preset', { presetName: name });
			eqState.set(state);
			message = `EQ preset "${name}" applied.`;
		} catch (err) {
			error = toErrorMessage(err);
		}
	}

	function bandLabel(value: number) {
		if (value === 0) return '0 dB';
		return `${value > 0 ? '+' : ''}${value.toFixed(1)} dB`;
	}

	function toErrorMessage(err: unknown) {
		const msg = typeof err === 'string' ? err : err instanceof Error ? err.message : null;
		if (msg?.includes('__TAURI_INTERNALS__')) return '';
		return msg ?? 'Unexpected error';
	}
</script>

<div class="eq-panel">
	<div>
		<h2 class="m-0 font-[family-name:var(--font-family-display)] text-[clamp(22px,2vw,30px)] leading-[1.04]">Equalizer</h2>
		<p class="text-soft text-sm">10-band graphic equalizer — EQ state is stored but DSP processing is not yet active</p>
	</div>

	{#if error}<p class="text-danger text-sm">{error}</p>{/if}
	{#if message}<p class="text-success text-sm">{message}</p>{/if}

	<div class="flex flex-wrap gap-2 mb-4">
		{#each presets as preset}
			<Button
				size="sm"
				variant={$eqState.preset_name === preset.name ? 'default' : 'outline'}
				onclick={() => applyPreset(preset.name)}
			>
				{preset.name}
			</Button>
		{/each}
	</div>

	<div class="eq-bands">
		{#each $eqState.bands as band, i}
			<label class="eq-band">
				<span class="eq-band-label">{EQ_BAND_LABELS[i]}</span>
				<div class="eq-slider-container">
					<Slider
						value={[band]}
						min={-12}
						max={12}
						step={0.5}
						orientation="vertical"
						onValueChange={(v: number[]) => {
							eqState.update((s) => {
								const b = [...s.bands];
								b[i] = v[0];
								return { ...s, bands: b, preset_name: 'Custom' };
							});
						}}
						onValueCommit={(v: number[]) => setBand(i, v[0])}
					/>
				</div>
				<span class="eq-band-value">{bandLabel(band)}</span>
			</label>
		{/each}
	</div>

	<div class="flex gap-2 mt-4">
		<Button variant="secondary" size="sm" onclick={loadEqState}>Refresh</Button>
	</div>
</div>

<style>
	.eq-panel {
		display: flex;
		flex-direction: column;
		gap: 0.75rem;
		padding: 1.25rem;
		border: 1px solid var(--color-outline, #e5e7eb);
		border-radius: 1rem;
		background: var(--color-surface-2);
	}

	.eq-bands {
		display: flex;
		justify-content: center;
		gap: 0.75rem;
		height: 200px;
		padding: 0.5rem 0;
	}

	.eq-band {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: 0.25rem;
		flex: 1;
		max-width: 60px;
	}

	.eq-band-label {
		font-family: var(--font-mono, monospace);
		font-size: 0.625rem;
		color: var(--color-soft);
		text-transform: uppercase;
	}

	.eq-slider-container {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		width: 100%;
	}

	.eq-band-value {
		font-family: var(--font-mono, monospace);
		font-size: 0.625rem;
		color: var(--color-soft);
		white-space: nowrap;
	}

	.eq-slider-container :global([data-orientation="vertical"]) {
		height: 100%;
	}
</style>
