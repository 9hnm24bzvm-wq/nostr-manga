<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { get } from 'svelte/store';
  import { goto } from '$app/navigation';
  import { page } from '$app/state';
  import { PUBLIC_API_URL, PUBLIC_KRATOS_PUBLIC_API, PUBLIC_SITE_NAME } from '$env/static/public';
  import { Session } from '$lib/stores';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import { createAuthFormOwner } from './auth-form-owner';
  import { safeRegistrationReturnTo } from './account-onboarding';
  import { checkNativeLogout, performNativeLogout } from './native-logout';
  const owner = createAuthFormOwner();
  $effect.pre(() => { page.url.href; owner.observe(window.location.href); });
  let pending = $state(false), settled = $state(false), signedOut = $state(false), message = $state('');
  let disposed = false, generation = 0, abort: AbortController | undefined;
  async function attempt(checkOnly = false) {
    if (pending || disposed || signedOut) return;
    const epoch = ++generation, intent = owner.capture(), session = get(Session), controller = abort = new AbortController();
    const target = safeRegistrationReturnTo(page.url.searchParams.get('redirect'));
    const current = () => { if (disposed || epoch !== generation || !intent.alive() || get(Session) !== session) throw Error('This sign-out view or account changed.'); };
    pending = true; settled = false; message = '';
    try {
      const result = await (checkOnly ? checkNativeLogout : performNativeLogout)({ fetch: (...args) => fetch(...args), origin: window.location.origin, authBase: PUBLIC_KRATOS_PUBLIC_API, apiBase: PUBLIC_API_URL, account: session?.identity?.id ?? null, assertCurrent: current, signal: controller.signal });
      current(); pending = false; settled = true; message = result.message;
      if (result.state === 'signed-out') {
        signedOut = true; abort = undefined; Session.set(undefined);
        // Clearing native Session also disposes the selected remote signer.
        // This happens only after the server check proves this browser signed out.
        try { await goto(target); } catch { /* Settled confirmation and Continue remain usable. */ }
      }
    } catch {
      if (!disposed && epoch === generation && intent.alive()) { pending = false; settled = true; message = 'Your sign-in or this page changed. Reload before trying to sign out again.'; }
    } finally { if (!disposed && epoch === generation) { pending = false; abort = undefined; } }
  }
  onMount(() => { void attempt(); });
  onDestroy(() => { disposed = true; generation++; abort?.abort(); });
</script>
<svelte:head><title>Sign out - {PUBLIC_SITE_NAME}</title></svelte:head>
<section class="mx-auto max-w-lg space-y-4 rounded border p-6" data-native-logout aria-busy={pending}>
  <h1 class="text-xl font-semibold">{pending ? 'Signing out…' : signedOut ? 'You are signed out' : 'Sign out not confirmed'}</h1>
  {#if pending}<Spinner /><p role="status">Checking this browser session and signing out securely. Please wait.</p>{/if}
  {#if settled}<p role={signedOut ? 'status' : 'alert'}>{message}</p>{/if}
  {#if settled && !signedOut}<div class="flex flex-wrap gap-2"><Button onclick={() => attempt()}>Try signing out again</Button><Button variant="outline" onclick={() => attempt(true)}>Check sign-out status</Button><Button variant="outline" onclick={() => window.location.reload()}>Reload account state</Button><Button variant="ghost" href="/settings/identity">Back to Account</Button></div>{/if}
  {#if signedOut}<Button href={safeRegistrationReturnTo(page.url.searchParams.get('redirect'))}>Continue</Button>{/if}
</section>
