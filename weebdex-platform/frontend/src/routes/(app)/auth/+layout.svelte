<script lang="ts">
	const { children } = $props();
</script>

<div class="flex flex-auto flex-col items-center justify-center p-5" id="auth">
	<div class="w-full max-w-md">
		{@render children()}
	</div>
</div>
