<script lang="ts">
  // Restricted recovery capability is never a normal account Session.
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { createAuthFormOwner } from './auth-form-owner';
  import { goto } from '$app/navigation';
  import { API, Kratos } from '$lib';
  import Session from '$lib/stores/Session';
  import * as Card from '$lib/components/ui/card';
  import * as Field from '$lib/components/ui/field';
  import Input from '$lib/components/ui/input/input.svelte';
  import { Button } from '$lib/components/ui/button';
  import { Spinner } from '$lib/components/ui/spinner';
  import RetainedRecovery from './RecoveryRequest.svelte';
  import type { SettingsFlow } from '@ory/client';
  import { AxiosError } from 'axios';
  type Capability = { session_purpose?: string; recovery?: { can_reset_password?: boolean }; identity?: unknown; follows?: unknown; preferences?: unknown; permissions?: unknown[]; roles?: unknown[] };
  let ready = $state(false), resetOnly = $state(false), busy = $state(false), success = $state(false);
  let flow = $state<SettingsFlow>(), csrf = $state(''), error = $state('');
  let form = $state<HTMLFormElement>();
  let disposed = false, generation = 0;
  const controller = new AbortController();
  const formOwner = createAuthFormOwner();
  $effect.pre(() => { page.url.href; formOwner.observe(window.location.href); });
  function restrictedFlowToken(value: SettingsFlow) {
    const nodes = value?.ui?.nodes;
    if (!Array.isArray(nodes) || nodes.some(node => !['csrf_token', 'password', 'method'].includes((node.attributes as { name?: string }).name ?? ''))) throw Error('The server did not provide a password-only recovery flow. No account details were displayed.');
    const token = nodes.find(node => (node.attributes as { name?: string }).name === 'csrf_token');
    const password = nodes.find(node => (node.attributes as { name?: string }).name === 'password');
    const valueToken = (token?.attributes as { value?: unknown })?.value;
    if (typeof valueToken !== 'string' || !valueToken || !password) throw Error('The restricted recovery flow is incomplete. Request a fresh recovery link.');
    return valueToken;
  }
  function restrictedFlow(value: SettingsFlow) {
    const token = restrictedFlowToken(value);
    flow = value; csrf = token;
  }
  function confirmResetSuccess(value: SettingsFlow, submitted: SettingsFlow, submittedToken: string) {
    // Native settings consumes the submitted flow and rotates its session,
    // returning a fresh password-only successor rather than the consumed id.
    const id = value?.id;
    if (typeof id !== 'string' || !id || id === submitted.id || value.type !== 'browser' || value.state !== 'success' || value.return_to !== submitted.return_to || value.ui?.method !== 'POST' || typeof value.ui.action !== 'string') throw Error('The password update did not return a valid recovery successor. Sign in with the new password before requesting another reset.');
    const target = new URL(value.ui.action, window.location.href);
    if (target.origin !== window.location.origin || target.pathname !== '/auth/self-service/settings' || target.search !== '?flow=' + encodeURIComponent(id) || target.hash || target.username || target.password || restrictedFlowToken(value) === submittedToken) throw Error('An unrelated recovery settings successor was discarded. Sign in with the new password before requesting another reset.');
  }
  const describe = (cause: unknown) => cause instanceof AxiosError
    ? cause.response?.data?.ui?.messages?.map((message: { text?: string }) => message.text).filter(Boolean).join(' ') || cause.response?.data?.error?.message || 'The recovery request was not confirmed. It may have completed; try signing in with the new password before requesting a new link. No password update was replayed.'
    : cause instanceof Error ? cause.message : 'The restricted recovery flow is unavailable. Request a fresh recovery link.';
  onMount(() => {
    const captured = ++generation;
    const intent = formOwner.capture();
    void (async () => {
      try {
        const result = await API.Auth.Check();
        if (disposed || captured !== generation || !intent.alive()) return;
        const capability = result?.body as unknown as Capability | undefined;
        if (result?.status === 200 && capability?.session_purpose === 'recovery' && capability.recovery?.can_reset_password === true) {
          if (capability.identity !== undefined || capability.follows !== undefined || capability.preferences !== undefined || capability.roles?.length || capability.permissions?.length) throw Error('The server returned broader account data for restricted recovery; reset was not mounted.');
          resetOnly = true;
          const response = await Kratos.createBrowserSettingsFlow({ returnTo: '/auth/recovery' }, { signal: controller.signal });
          if (!disposed && captured === generation && intent.alive()) restrictedFlow(response.data);
        }
      } catch (cause) { if (!disposed && captured === generation && intent.alive()) error = describe(cause); }
      finally { if (!disposed && captured === generation && intent.alive()) ready = true; }
    })();
    return () => { disposed = true; generation++; controller.abort(); csrf = ''; };
  });
  const resetPassword = async (event: SubmitEvent) => {
    event.preventDefault();
    if (busy || !resetOnly || !flow || !csrf || !form || disposed) return;
    const captured = generation, submitted = flow, flowId = submitted.id, csrfToken = csrf;
    const intent = formOwner.capture(flowId, () => flow?.id, false);
    if (!intent.alive()) return;
    const submittedField = form.elements.namedItem('password') as HTMLInputElement | null;
    busy = true; error = '';
    try {
      const password = submittedField!.value;
      // Do not cancel/replay a valid password write merely because its UI left.
      const response = await Kratos.updateSettingsFlow({ flow: flowId, updateSettingsFlowBody: { method: 'password', csrf_token: csrfToken, password } });
      if (disposed || captured !== generation || !intent.alive()) return;
      if (response.status !== 200 || response.data?.state !== 'success') {
        if (response.data?.id !== flowId) throw Error('An unrelated recovery settings flow was discarded. Reload the current recovery form.');
        restrictedFlow(response.data); throw Error('The password update was not accepted. Review the restricted form errors.');
      }
      // An acknowledged write must never reuse its consumed local CSRF token,
      // including when the acknowledgement's successor shape is rejected.
      csrf = '';
      confirmResetSuccess(response.data, submitted, csrfToken);
      await Session.initialize(true);
      if (disposed || captured !== generation || !intent.alive()) return;
      if (!(await Session.isValid())) throw Error('The password may have been changed, but a normal account session could not be confirmed. Sign in with the new password before trying another reset.');
      if (disposed || captured !== generation || !intent.alive()) return;
      success = true;
    } catch (cause) {
      if (!disposed && captured === generation && intent.alive()) {
        // Validation errors may provide the same CSRF-bound restricted flow.
        if (cause instanceof AxiosError && cause.response?.status === 400 && cause.response.data?.ui) {
          try {
            if (cause.response.data.id !== flowId) throw Error('Unrelated recovery validation flow');
            restrictedFlow(cause.response.data);
          } catch { flow = undefined; csrf = ''; }
        }
        error = describe(cause);
      }
    } finally {
      // Clear only this write's secret, never a newer form's replacement field.
      if (submittedField) submittedField.value = '';
      if (!disposed && captured === generation && intent.alive()) busy = false;
    }
  };
</script>

{#if !ready}
  <Spinner class="m-auto size-10" />
{:else if resetOnly}
  <Card.Root class="relative rounded" data-recovery-reset-only>
    <Card.Header class="text-center">
      <Card.Title class="text-xl">{success ? 'Password reset completed' : 'Reset Password'}</Card.Title>
      <Card.Description>{success ? 'A normal account session has been confirmed.' : 'This recovery session can only set a new password. It cannot access account details or private library data.'}</Card.Description>
    </Card.Header>
    <Card.Content>
      {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
      {#if success}
        <Button type="button" onclick={() => goto('/settings/account')}>Continue to Account Settings</Button>
      {:else if flow && csrf}
        <form onsubmit={resetPassword} bind:this={form}>
          <Field.Group>
            <Field.Field>
              <Field.Label for="recovery-new-password">New Password</Field.Label>
              <Input id="recovery-new-password" name="password" type="password" autocomplete="new-password" minlength={8} maxlength={64} required disabled={busy} />
            </Field.Field>
            <input type="hidden" name="csrf_token" value={csrf} />
            <Button type="submit" disabled={busy}>{#if busy}<Spinner />{:else}Reset Password{/if}</Button>
          </Field.Group>
        </form>
      {:else}
        <p>Request a fresh recovery link if this one has expired. No password update will be replayed.</p>
      {/if}
      <a class="hover:text-accent-foreground underline" href="/auth/login">Sign in</a>
    </Card.Content>
  </Card.Root>
{:else if error}
  <Card.Root class="relative rounded"><Card.Content><p role="alert">{error}</p><a class="hover:text-accent-foreground underline" href="/auth/recovery">Reload Account Recovery</a></Card.Content></Card.Root>
{:else}
  <RetainedRecovery />
{/if}
