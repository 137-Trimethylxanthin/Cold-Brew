<script lang="ts">
	import * as Dialog from '$lib/components/ui/dialog';
	import { Separator } from '$lib/components/ui/separator';
	import * as Table from '$lib/components/ui/table';

	let { open = $bindable(false) }: { open: boolean } = $props();

	const shortcuts = [
		{ key: 'Space', action: 'Play / Pause' },
		{ key: '← / →', action: 'Seek backward / forward' },
		{ key: 'N', action: 'Next track' },
		{ key: 'P', action: 'Previous track' },
		{ key: 'M', action: 'Mute / Unmute' },
		{ key: '↑ / ↓', action: 'Volume up / down' },
		{ key: 'S', action: 'Toggle shuffle' },
		{ key: 'R', action: 'Toggle repeat' },
		{ key: '[ / ]', action: 'AB Repeat (set A / set B)' },
		{ key: 'L', action: 'Toggle sidebar' }
	];
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		<Dialog.Header>
			<Dialog.Title>Keyboard Shortcuts</Dialog.Title>
			<Dialog.Description>
				Press <kbd
					class="px-1.5 py-0.5 rounded bg-surface-2 border border-outline font-mono text-xs">?</kbd
				> anywhere to open this reference.
			</Dialog.Description>
		</Dialog.Header>
		<Separator />
		<div class="max-h-[60vh] overflow-y-auto">
			<Table.Root>
				<Table.Header>
					<Table.Row>
						<Table.Head class="py-2 pr-4 text-soft font-medium">Key</Table.Head>
						<Table.Head class="py-2 text-soft font-medium">Action</Table.Head>
					</Table.Row>
				</Table.Header>
				<Table.Body>
					{#each shortcuts as shortcut}
						<Table.Row class="border-outline/50">
							<Table.Cell class="py-2 pr-4">
								<kbd
									class="inline-flex items-center px-1.5 py-0.5 rounded bg-surface-2 border border-outline font-mono text-xs whitespace-nowrap"
								>
									{shortcut.key}
								</kbd>
							</Table.Cell>
							<Table.Cell class="py-2">{shortcut.action}</Table.Cell>
						</Table.Row>
					{/each}
				</Table.Body>
			</Table.Root>
		</div>
	</Dialog.Content>
</Dialog.Root>
