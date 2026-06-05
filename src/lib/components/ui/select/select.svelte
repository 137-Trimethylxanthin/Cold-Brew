<script lang="ts">
	import { Select as SelectPrimitive } from "bits-ui";
	import type { SelectRootPropsWithoutHTML } from "bits-ui";

	let {
		open = $bindable(false),
		value = $bindable() as any,
		type = "single" as const,
		...restProps
	}: Omit<SelectRootPropsWithoutHTML, 'value' | 'type' | 'onValueChange'> & {
		value?: any;
		type?: "single" | "multiple";
		onValueChange?: (value: any) => void;
	} = $props();
</script>

<SelectPrimitive.Root bind:open bind:value={value as never} type={type as never} {...restProps} />
