<script lang="ts">
  import { onMount } from 'svelte';
  import { registrationContinuationHref } from './account-onboarding';
  import IdentitySetup from '$localLibrary/IdentitySetup.svelte';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';

  let { returnTo, needsVerification, accountCurrent, oncontinue, current = () => accountCurrent }: {
    returnTo: string;
    needsVerification: boolean;
    accountCurrent: boolean;
    oncontinue: () => void;
    current?: () => boolean;
  } = $props();
  let heading = $state<HTMLHeadingElement>();
  const verificationHref = $derived(registrationContinuationHref('/auth/verification', returnTo));
  const loginHref = $derived(registrationContinuationHref('/auth/login', returnTo));
  onMount(() => { heading?.focus({ preventScroll: true }); });
</script>

<Card.Root class="rounded" data-account-onboarding aria-labelledby="account-welcome-title">
  <Card.Header class="text-center">
    <h1 id="account-welcome-title" bind:this={heading} tabindex="-1" class="text-xl font-semibold leading-tight">
      {accountCurrent ? 'Your account is ready' : 'Your session changed'}
    </h1>
    <Card.Description role="status">
      {accountCurrent ? 'You can start reading now. Set up an optional public identity below for ratings, uploads and some public actions.' : 'Your account was created. Sign in again to use it.'}
    </Card.Description>
  </Card.Header>
  <Card.Content class="space-y-5">
    {#if accountCurrent}
      {#if needsVerification}
        <section class="space-y-1 border-t pt-4 text-sm">
          <h2 class="font-semibold">Verify your site email</h2>
          <p class="text-muted-foreground">Check your inbox to finish verifying your email login. Public identity setup is separate.</p>
          <a href={verificationHref} class="inline-flex min-h-11 items-center text-primary underline underline-offset-4">Verify email</a>
        </section>
      {/if}
      <Button class="min-h-11 w-full" onclick={oncontinue}>Continue reading</Button>
      <details><summary class="cursor-pointer text-sm">Optional: set up public identity</summary><div class="mt-4">
      <IdentitySetup onfinish={oncontinue} assertCurrent={() => { if (!current()) throw Error('This signup is no longer current'); }} />
      </div></details>
    {:else}
      <Button class="min-h-11 w-full" href={loginHref}>Sign in</Button>
      <a href={returnTo} class="flex min-h-11 items-center justify-center text-sm underline underline-offset-4">Continue browsing</a>
    {/if}
  </Card.Content>
</Card.Root>
