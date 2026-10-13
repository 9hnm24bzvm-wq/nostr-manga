<script lang="ts">
	import { page } from "$app/state";
	import * as Sidebar from "$lib/components/ui/sidebar";
	import { useSidebar } from "$lib/components/ui/sidebar";
	import * as Tooltip from "$lib/components/ui/tooltip";
	import { isShowSidebar, Session } from "$lib/stores";
	import {
		BadgeAlertIcon,
		BadgePlusIcon,
		FlagIcon,
		HandshakeIcon,
		HatGlassesIcon,
		HistoryIcon,
		HomeIcon,
		InfoIcon,
		LibraryIcon,
		LogInIcon,
		Megaphone,
		PlusIcon,
		RssIcon,
		ScaleIcon,
		SearchIcon,
		SettingsIcon,
		ShuffleIcon,
		TrendingUpIcon,
		UserIcon,
		UsersIcon
	} from "@lucide/svelte";
	import { onMount } from "svelte";

	const sidebar = useSidebar();
	const menu: {
		label?: string;
		action?: {
			name: string;
			url: string;
			icon: any;
		};
		items: {
			name: string;
			url: string;
			icon: any;
			hidden?: boolean;
			action?: {
				name: string;
				url: string;
				icon: any;
			};
		}[];
	}[] = $derived([
		{
			items: [{ name: "Home", url: "/", icon: HomeIcon }]
		},
		{
			label: "Follows",
			items: [
				{
					name: "My Library",
					url: "/library",
					icon: LibraryIcon,
					action: $Session ? { name: "Settings", url: "/settings/library", icon: SettingsIcon } : undefined
				},
				{ name: "My Updates", url: "/library/updates", icon: BadgeAlertIcon },
				{ name: "My History", url: "/history", icon: HistoryIcon }
			]
		},
		{
			label: "Titles",
			action: { name: "Add Title", url: "/drafts/title", icon: PlusIcon },
			items: [
				{ name: "Advanced Search", url: "/search", icon: SearchIcon },
				{ name: "Recently Added", url: "/search?sort=createdAt", icon: BadgePlusIcon },
				{ name: "Latest Updates", url: "/updates", icon: RssIcon },
				{ name: "Popular", url: "/popular", icon: TrendingUpIcon },
				{ name: "Random", url: "/title/random", icon: ShuffleIcon }
			]
		},
		{
			label: "Community",
			items: [
				{
					name: "Groups",
					url: "/groups",
					icon: UsersIcon,
					action: { name: "Create Group", url: "/create/group", icon: PlusIcon }
				},
				{ name: "Users", url: "/users", icon: UserIcon }
			]
		},
		{
			label: "WeebDex",
			items: [
				{ name: "About Us", url: "/about", icon: InfoIcon },
				{ name: "Announcements", url: "/announcements", icon: Megaphone },
				{ name: "Guidelines", url: "/guidelines", icon: ScaleIcon },
				{ name: "Privacy Policy", url: "/privacy", icon: HatGlassesIcon },
				{ name: "Terms of Service", url: "/terms", icon: HandshakeIcon }
			]
		}
	]);

	let isMounted = $state(false);
	onMount(() => {
		sidebar.setOpen($isShowSidebar);
		isMounted = true;
	});

	$effect(() => {
		if (isMounted) {
			$isShowSidebar = sidebar.open;
		}
	});
</script>

{#if isMounted}
	<Sidebar.Root class="whitespace-nowrap" collapsible="icon">
		<Tooltip.Provider delayDuration={0} disabled={sidebar.open || sidebar.openMobile}>
			<Sidebar.Content class="py-2">
				{#each menu as group}
					<Sidebar.Group class="relative py-0">
						{#if group.label}
							<Sidebar.GroupLabel class="pointer-events-none">{group.label}</Sidebar.GroupLabel>
						{/if}
						{#if group.action}
							<Sidebar.GroupAction class="top-1	right-2 size-6" title="Add Title">
								{#snippet child({ props })}
									{#if group.action}
										<a href={group.action.url} {...props}>
											<group.action.icon class="!size-5" /> <span class="sr-only">{group.action.name}</span>
										</a>
									{/if}
								{/snippet}
							</Sidebar.GroupAction>
						{/if}
						<Sidebar.GroupContent>
							<Sidebar.Menu class="gap-0.5">
								{#each group.items as item}
									{#if !item.hidden}
										<Sidebar.MenuItem>
											<Sidebar.MenuButton isActive={page.url.pathname === item.url}>
												{#snippet child({ props })}
													<Tooltip.Root>
														<Tooltip.Trigger>
															{#snippet child({ props: props2 })}
																<a href={item.url} {...props} {...props2}>
																	<item.icon />
																	<strong class="font-medium">{item.name}</strong>
																</a>
															{/snippet}
														</Tooltip.Trigger>
														<Tooltip.Content side="right">
															<strong class="text-sm font-medium">{item.name}</strong>
														</Tooltip.Content>
													</Tooltip.Root>
												{/snippet}
											</Sidebar.MenuButton>
											{#if item.action}
												<Sidebar.MenuAction class="!top-1 !right-1 size-6" title="Create Group">
													{#snippet child({ props })}
														{#if item.action}
															<a href={item.action.url} {...props}>
																<item.action.icon class="!size-4" /> <span class="sr-only">{item.action.name}</span>
															</a>
														{/if}
													{/snippet}
												</Sidebar.MenuAction>
											{/if}
										</Sidebar.MenuItem>
									{/if}
								{/each}
							</Sidebar.Menu>
						</Sidebar.GroupContent>
					</Sidebar.Group>
				{/each}
			</Sidebar.Content>
			<Sidebar.Separator />
			<Sidebar.Footer>
				<Sidebar.Menu class="gap-0.5">
					{#if $Session}
						<Sidebar.MenuItem>
							<Sidebar.MenuButton>
								{#snippet child({ props })}
									<Tooltip.Root>
										<Tooltip.Trigger>
											{#snippet child({ props: props2 })}
												<a href="/user/{$Session.identity.id}" {...props} {...props2}>
													<UserIcon />
													<strong class="font-medium">My Profile</strong>
												</a>
											{/snippet}
										</Tooltip.Trigger>
										<Tooltip.Content side="right">
											<strong class="text-sm font-medium">My Profile</strong>
										</Tooltip.Content>
									</Tooltip.Root>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
						<Sidebar.MenuItem>
							<Sidebar.MenuButton>
								{#snippet child({ props })}
									<Tooltip.Root>
										<Tooltip.Trigger>
											{#snippet child({ props: props2 })}
												<a href="/my/reports" {...props} {...props2}>
													<FlagIcon />
													<strong class="font-medium">My Reports</strong>
												</a>
											{/snippet}
										</Tooltip.Trigger>
										<Tooltip.Content side="right">
											<strong class="text-sm font-medium">My Reports</strong>
										</Tooltip.Content>
									</Tooltip.Root>
								{/snippet}
							</Sidebar.MenuButton>
						</Sidebar.MenuItem>
					{/if}
					<Sidebar.MenuItem>
						<Sidebar.MenuButton>
							{#snippet child({ props })}
								<Tooltip.Root>
									<Tooltip.Trigger>
										{#snippet child({ props: props2 })}
											<a href="/settings" {...props} {...props2}>
												<SettingsIcon />
												<strong class="font-medium">Settings</strong>
											</a>
										{/snippet}
									</Tooltip.Trigger>
									<Tooltip.Content side="right">
										<strong class="text-sm font-medium">Settings</strong>
									</Tooltip.Content>
								</Tooltip.Root>
							{/snippet}
						</Sidebar.MenuButton>
					</Sidebar.MenuItem>
					<Sidebar.MenuItem>
						<Sidebar.MenuButton>
							{#snippet child({ props })}
								{#if $Session}
									<Tooltip.Root>
										<Tooltip.Trigger>
											{#snippet child({ props: props2 })}
												<a href="/auth/logout" {...props} {...props2}>
													<LogInIcon />
													<strong class="font-medium">Logout</strong>
												</a>
											{/snippet}
										</Tooltip.Trigger>
										<Tooltip.Content side="right">
											<strong class="text-sm font-medium">Logout</strong>
										</Tooltip.Content>
									</Tooltip.Root>
								{:else}
									<Tooltip.Root>
										<Tooltip.Trigger>
											{#snippet child({ props: props2 })}
												<a href="/auth/login{page.url.searchParams.toString()}" {...props} {...props2}>
													<LogInIcon />
													<strong class="font-medium">Login</strong>
												</a>
											{/snippet}
										</Tooltip.Trigger>
										<Tooltip.Content side="right">
											<strong class="text-sm font-medium">Login</strong>
										</Tooltip.Content>
									</Tooltip.Root>
								{/if}
							{/snippet}
						</Sidebar.MenuButton>
					</Sidebar.MenuItem>
				</Sidebar.Menu>
			</Sidebar.Footer>
		</Tooltip.Provider>
	</Sidebar.Root>
{/if}
