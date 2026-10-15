<script lang="ts">
  import { onMount } from 'svelte';
  import { registrationContinuationHref } from './account-onboarding';
  import { Button } from '$lib/components/ui/button';
  import * as Card from '$lib/components/ui/card';

  let { returnTo, needsVerification, accountCurrent, oncontinue }: {
    returnTo: string;
    needsVerification: boolean;
    accountCurrent: boolean;
    oncontinue: () => void;
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
      {accountCurrent ? 'A quick guide before you start.' : 'Your account was created. Sign in again to use it.'}
    </Card.Description>
  </Card.Header>
  <Card.Content class="space-y-5">
    {#if accountCurrent}
      <div class="space-y-4 text-sm">
        <section class="space-y-1">
          <h2 class="font-semibold">Your saved manga</h2>
          <p class="text-muted-foreground">Any titles saved in this browser stay here. Choose where to save your reading in Settings → Library.</p>
        </section>
        <section class="space-y-1">
          <h2 class="font-semibold">Public sharing is optional</h2>
          <p class="text-muted-foreground">You can set up a separate public identity later in Settings → Public identity. You’ll review and approve anything you publish.</p>
        </section>
        {#if needsVerification}
          <section class="space-y-1">
            <h2 class="font-semibold">Verify your email</h2>
            <p class="text-muted-foreground">Some account features need email verification.</p>
            <a href={verificationHref} class="inline-flex min-h-11 items-center text-primary underline underline-offset-4">Verify email</a>
          </section>
        {/if}
      </div>
      <Button class="min-h-11 w-full" onclick={oncontinue}>Continue</Button>
    {:else}
      <Button class="min-h-11 w-full" href={loginHref}>Sign in</Button>
      <a href={returnTo} class="flex min-h-11 items-center justify-center text-sm underline underline-offset-4">Continue browsing</a>
    {/if}
  </Card.Content>
</Card.Root>
