<script lang="ts">
	import { replaceState } from "$app/navigation";
	import { page } from "$app/state";
	import { Constants, Mutex, Permission, Typography, Utils } from "$lib";
	import API from "$lib/API";
	import { Button, buttonVariants } from "$lib/components/ui/button";
	import * as InputGroup from "$lib/components/ui/input-group";
	import { useKey } from "$lib/components/ui/key";
	import * as Popover from "$lib/components/ui/popover";
	import { Separator } from "$lib/components/ui/separator";
	import * as Tabs from "$lib/components/ui/tabs";
	import { Textarea } from "$lib/components/ui/textarea";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { Session } from "$lib/stores";
	import { cn } from "$lib/Utils";
	import {
		BoldIcon,
		ChevronsRightLeft,
		EyeOffIcon,
		ImageIcon,
		ItalicIcon,
		SendIcon,
		StrikethroughIcon,
		UnderlineIcon
	} from "@lucide/svelte";
	import { StatusCodes } from "http-status-codes";
	import { toast } from "svelte-sonner";
	import { Spinner } from "../ui/spinner";
	import { useComments } from "./context.svelte";

	const mutex = new Mutex();
	const comments = useComments();
	const key = useKey();
	const html = $derived(Utils.parseComment(comments.body));

	let tab = $state("edit");
	let isEmbed = $state(false);

	const tools = [
		{ text: "Bold", icon: BoldIcon, onclick: () => formatSelection("**", "**") },
		{ text: "Italic", icon: ItalicIcon, onclick: () => formatSelection("*", "*") },
		{ text: "Underline", icon: UnderlineIcon, onclick: () => formatSelection("__", "__") },
		{ text: "Strikethrough", icon: StrikethroughIcon, onclick: () => formatSelection("~~", "~~") },
		{ text: "Spoiler", icon: EyeOffIcon, onclick: () => formatSelection("||", "||") },
		{
			text: "Collapsible",
			icon: ChevronsRightLeft,
			onclick: () => formatSelection("+++Spoiler\n", "\n+++", "Input something here...")
		}
	];

	const formatSelection = (prefix: string, suffix: string, defaultText = "") => {
		if (!comments.textAreaRef) {
			return;
		}

		const start = comments.textAreaRef.selectionStart;
		const end = comments.textAreaRef.selectionEnd;

		const selected = comments.body.slice(start, end) || defaultText;
		const isRemove = selected.startsWith(prefix) && selected.endsWith(suffix);

		let replacement: string;
		if (isRemove) {
			replacement = selected.slice(prefix.length, selected.length - suffix.length);
		} else replacement = prefix + selected + suffix;

		comments.body = comments.body.slice(0, start) + replacement + comments.body.slice(end);
		requestAnimationFrame(() => {
			if (!comments.textAreaRef) return;
			comments.textAreaRef.focus();

			if (isRemove)
				comments.textAreaRef.setSelectionRange(start, start + selected.length - prefix.length - suffix.length);
			else if (selected)
				comments.textAreaRef.setSelectionRange(start, start + selected.length + prefix.length + suffix.length);
			else comments.textAreaRef.setSelectionRange(start + prefix.length, start + prefix.length);
		});
	};

	const onSubmit = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		return mutex.exec(async () => {
			if (!comments.body.trim().length) {
				return;
			}

			try {
				const res = await API.Thread.CreateReply(comments.thread.id, { body: comments.body });
				if (res?.status === StatusCodes.CREATED && res.body) {
					toast.success("Comment posted successfully");
					replaceState(window.location.href, {
						...page.state,
						_$Replies_feed: undefined
					});

					comments.body = "";
					key.refresh();
				} else if (res?.status === StatusCodes.FORBIDDEN) {
					throw Constants.Error.Forbidden;
				} else if (res?.status === StatusCodes.TOO_MANY_REQUESTS) {
					throw Constants.Error.TooManyRequests;
				} else throw Constants.Error.Unknown;
			} catch (e) {
				if (e instanceof Constants.Error.Error) toast.error(e.title, { description: e.body });
				else toast.error(Constants.Error.Unknown.title, { description: Constants.Error.Unknown.body });
			}
		});
	};

	const onImageEmbed = (ev: SubmitEvent) => {
		ev.preventDefault();
		ev.stopPropagation();
		ev.stopImmediatePropagation();

		const value = (ev.target as any)?.url.value;
		if (!value) {
			return;
		}

		isEmbed = false;

		if (comments.body.trim() && !comments.body.endsWith("\n")) {
			comments.body += "\n";
		}
		comments.body += `![](${value})`;
		comments.textAreaRef?.focus();
	};
</script>

{#if !comments.thread.locked}
	{#if Permission.Reply.canCreate()}
		<form onsubmit={onSubmit}>
			<Tabs.Root class="bg-input/30 !gap-0 overflow-hidden rounded-md" bind:value={tab}>
				<div class="flex items-center justify-between border-b">
					<Tabs.List class="bg-transparent p-0">
						<Tabs.Trigger value="edit" disabled={$mutex}>
							{#snippet child({ props: { class: _, ...props } })}
								<Button
									class={cn("rounded-none border-none", {
										"!bg-transparent": props["aria-selected"] !== "true",
										"!bg-input/20 text-accent-foreground": props["aria-selected"] === "true"
									})}
									variant="outline"
									{...props}
								>
									Edit
								</Button>
							{/snippet}
						</Tabs.Trigger>
						<Tabs.Trigger value="preview" disabled={$mutex || !comments.body.trim().length || !html?.length}>
							{#snippet child({ props: { class: _, ...props } })}
								<Button
									class={cn("rounded-none border-none", {
										"!bg-transparent": props["aria-selected"] !== "true",
										"!bg-input/20 text-accent-foreground": props["aria-selected"] === "true"
									})}
									variant="outline"
									type="button"
									{...props}
								>
									Preview
								</Button>
							{/snippet}
						</Tabs.Trigger>
					</Tabs.List>
					<Button
						class="border-none !bg-transparent tracking-wider uppercase"
						variant="outline"
						type="submit"
						disabled={$mutex}
					>
						{#if $mutex}
							<Spinner />
						{:else}
							<SendIcon />
						{/if}
						Post
					</Button>
				</div>
				<Tabs.Content value="edit">
					<Tooltip.Provider delayDuration={0} disableHoverableContent>
						<div class="bg-input/20 flex items-center border-b">
							{#each tools as tool}
								<Tooltip.Root>
									<Tooltip.Trigger onclick={tool.onclick}>
										{#snippet child({ props })}
											<Button class="rounded-none" variant="ghost" size="icon-sm" {...props} disabled={$mutex}>
												<tool.icon />
											</Button>
										{/snippet}
									</Tooltip.Trigger>
									<Tooltip.Content>
										{tool.text}
									</Tooltip.Content>
								</Tooltip.Root>
							{/each}
							<Tooltip.Root>
								<Tooltip.Trigger>
									{#snippet child({ props })}
										<Popover.Root bind:open={isEmbed}>
											<Popover.Trigger {...props}>
												{#snippet child({ props: { class: className, ...props } })}
													<Button class="!rounded-none" variant="ghost" size="icon-sm" {...props} disabled={$mutex}>
														<ImageIcon />
													</Button>
												{/snippet}
											</Popover.Trigger>
											<Popover.Content class="rounded-md p-1">
												<form onsubmit={onImageEmbed}>
													<InputGroup.Root class="border-none !ring-0 !ring-transparent">
														<InputGroup.Input name="url" type="url" placeholder="Image URL" required />
														<InputGroup.Addon align="inline-end">
															<InputGroup.Button type="submit">OK</InputGroup.Button>
														</InputGroup.Addon>
													</InputGroup.Root>
												</form>
											</Popover.Content>
										</Popover.Root>
									{/snippet}
								</Tooltip.Trigger>
								<Tooltip.Content>Embed Image</Tooltip.Content>
							</Tooltip.Root>
							<InputGroup.Text
								class={cn("ms-auto mr-2", {
									"text-red-300": 30000 - comments.body.length < 0
								})}
							>
								{(30000 - comments.body.length).toLocaleString()}
							</InputGroup.Text>
							<Separator class="!h-4" orientation="vertical" />
						</div>
						<Textarea
							class="min-h-30 border-none !bg-transparent !ring-0 !ring-transparent"
							bind:value={comments.body}
							placeholder="Enter your thoughts..."
							required
							bind:ref={comments.textAreaRef}
							disabled={$mutex}
						/>
					</Tooltip.Provider>
				</Tabs.Content>
				<Tabs.Content class="bg-card" value="preview">
					<div class="typography comment p-3" {@attach Typography.observer}>
						{@html html}
					</div>
				</Tabs.Content>
			</Tabs.Root>
		</form>
	{:else}
		<div class="rounded-md border border-dashed border-yellow-500/50 bg-yellow-500/15 p-3 text-center text-yellow-300">
			<strong class="text-lg font-medium">You do not have permission to post a comment</strong>
			<div class="space-y-2">
				{#if $Session!.roles?.includes("user")}
					<p>Please verify your account to join the conversation</p>
					<a
						class={buttonVariants()}
						href="/auth/verification{new URLSearchParams({
							redirect: page.url.href
						})}"
					>
						Verify
					</a>
				{:else}
					<p>You have been banned from posting comments</p>
				{/if}
			</div>
		</div>
	{/if}
{/if}
