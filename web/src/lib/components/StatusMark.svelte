<script lang="ts">
	import type { Status } from '../views/settlement';
	import Mark, { type Tone } from './Mark.svelte';

	let { status }: { status: Status } = $props();

	const marks: Record<Status, { tone: Tone; label: string }> = {
		open: { tone: 'open', label: 'Open' },
		settled: { tone: 'ok', label: 'Settled' },
		'paying-down': { tone: 'progress', label: 'Paying down a past difference' },
		difference: { tone: 'warning', label: 'Closed with a difference' }
	};
</script>

<Mark {...marks[status]} />
