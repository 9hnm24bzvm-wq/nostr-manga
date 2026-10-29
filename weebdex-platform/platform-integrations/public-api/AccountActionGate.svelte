<script lang="ts">
  import { onDestroy, tick } from 'svelte';
  import { API } from '$lib';
  import { Session } from '$lib/stores';
  import { Button, buttonVariants } from '$lib/components/ui/button';
  import type { NativeAccountIssue } from './native-account-scope';
  let { issue, onready = () => {} }: { issue?: NativeAccountIssue; onready?: () => void | Promise<void> } = $props();
  let pending = $state(false), error = $state(''), disposed = false;
  const needsVerification = $derived(issue === 'verify');
  async function refresh() {
    if (pending || disposed) return;
    pending = true; error = ''; const capturedSession = $Session;
    try {
      const result = await API.Auth.Check(); if (disposed || $Session !== capturedSession) return;
      if (result?.status !== 200 || !result.body?.identity?.id) { error = 'You are not signed in yet. Sign in, then continue here.'; return; }
      Session.set(result.body); await tick(); if (!disposed && $Session === result.body) await onready();
    } catch { if (!disposed) error = 'Your account could not be checked. Please try again.'; }
    finally { if (!disposed) pending = false; }
  }
  onDestroy(() => { disposed = true; });
</script>
{#if !$Session?.identity?.id || issue === 'signin' || needsVerification}
  <section class="space-y-3 rounded border p-3" aria-label="Account access">
    <p class="text-sm">{needsVerification ? 'Verify your email to finish setting up your account.' : 'Sign in to use your account. One account works for ratings, comments, reports and follows.'}</p>
    <div class="flex flex-wrap gap-2">
      {#if needsVerification}
        <a class={buttonVariants({variant:'default',class:'min-h-11'})} href="/auth/verification" target="_blank" rel="noopener noreferrer">Verify email</a>
      {:else}
        <a class={buttonVariants({variant:'default',class:'min-h-11'})} href="/auth/login" target="_blank" rel="noopener noreferrer">Sign in</a>
        <a class={buttonVariants({variant:'outline',class:'min-h-11'})} href="/auth/register" target="_blank" rel="noopener noreferrer">Create account</a>
      {/if}
      <Button class="min-h-11" type="button" variant="outline" disabled={pending} onclick={refresh}>{pending ? 'Checking…' : 'Continue here'}</Button>
    </div>
    <p class="text-xs text-muted-foreground">Opens a new tab so you do not lose this task. When you are done, return here and choose Continue here.</p>
    {#if error}<p role="alert" class="text-sm text-destructive">{error}</p>{/if}
  </section>
{/if}
