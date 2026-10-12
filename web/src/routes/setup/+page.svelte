<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';

	goto(resolve('/setup/accounts'), { replace: true });
</script>
