<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	goto(resolve('/reports/balances'), { replace: true });
</script>
