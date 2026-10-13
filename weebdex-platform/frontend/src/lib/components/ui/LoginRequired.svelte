<script lang="ts">
	import { page } from "$app/state";
	import { buttonVariants } from "$lib/components/ui/button";
	import * as Empty from "$lib/components/ui/empty";

	const { isAction }: { isAction?: boolean } = $props();
</script>

<Empty.Root>
	<Empty.Header>
		<Empty.Title>Login required</Empty.Title>
		<Empty.Description>
			{isAction ? "Please sign in to perform this action." : "Please sign in to access this page."}
		</Empty.Description>
	</Empty.Header>
	<Empty.Content>
		<div class="flex flex-wrap items-center justify-center gap-2">
			<a class={buttonVariants({ variant: "default" })} href="/auth/login{page.url.searchParams.toString()}"> Login </a>
			<a
				class={buttonVariants({ class: "border-none", variant: "outline" })}
				href="/auth/register{new URLSearchParams({
					redirect: page.url.pathname + page.url.search
				}).toString()}"
			>
				Create an account
			</a>
		</div>
	</Empty.Content>
</Empty.Root>
