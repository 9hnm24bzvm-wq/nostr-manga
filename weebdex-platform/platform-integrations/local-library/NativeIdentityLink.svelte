<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { createIdentityLinkClient, type IdentityLink, type IdentityLinkReview } from './native-identity-link';
  import { signerIdentity, connectSigner, disconnectSigner } from './publication';
  import { encodePublicKey } from './local-signing-key';
  let { account, assertViewCurrent = () => {} }: { account: string; assertViewCurrent?: () => void } = $props();
  let client = $state.raw<Awaited<ReturnType<typeof createIdentityLinkClient>>>();
  let current = $state<IdentityLink>(), review = $state<IdentityLinkReview>(), approved = $state(false), pending = $state(false), uncertain = $state(false), error = $state(''), notice = $state('');
  let alive = true, generation = 0;
  const capturedAccount = account;
  function check(epoch = generation) { assertViewCurrent(); if (!alive || capturedAccount !== account || epoch !== generation) throw Error('This account connection review was closed or changed.'); }
  async function run(operation: () => Promise<void>) { if (pending) return; pending = true; error = ''; try { await operation(); check(); } catch (cause) { if (alive) error = cause instanceof Error ? cause.message : 'The connection could not be checked.'; } finally { if (alive) { uncertain = client?.uncertain() ?? false; pending = false; } } }
  async function load() { client ??= await createIdentityLinkClient(account, () => check()); current = await client.read(); check(); }
  async function prepare(action: 'link' | 'unlink') { if (!client || !current) return; const epoch = generation; review = undefined; approved = false; const next = await client.prepare(action, current); check(epoch); review = next; }
  function cancel() { generation++; review = undefined; approved = false; notice = 'Review cancelled. No further request will be sent. If a confirmation was already sent, check the connection below.'; }
  async function confirm() { if (!client || !review || !approved) return; const selected = review, epoch = generation; const next = await client.confirm(selected, true, () => check(epoch)); check(epoch); current = next; review = undefined; approved = false; if (selected.action === 'unlink' && $signerIdentity.owner === selected.publicKey) disconnectSigner(); notice = selected.action === 'link' ? 'Identity connected to this account. Your normal login, private library and permissions are unchanged.' : 'Identity disconnected from this account. Existing public posts and separately granted publishing authority are unchanged.'; }
  onMount(() => { void run(load); return () => { alive = false; generation++; client?.dispose(); }; });
</script>
<section class="space-y-3 rounded border p-3" data-native-identity-link>
  <h3 class="font-semibold">Connected portable identity</h3>
  <p class="text-sm">Optional: connect an identity you already use elsewhere once here. Your normal account works without it. This private connection does not publish your email, library or reading history, and does not grant publishing or staff permission.</p>
  {#if current?.publicKey}
    <p class="text-sm">Your account is connected to this public identity:</p><p class="break-all text-xs">{encodePublicKey(current.publicKey)}</p>
    {#if $signerIdentity.owner === current.publicKey}<p class="text-sm">Ready for public actions in this session. Each public change still needs your approval.</p>
    {:else}<p class="text-sm">Reconnect this identity's signer to approve public actions. Signing authority is not stored in your site login.</p><Button variant="outline" disabled={pending} onclick={() => run(async () => { await connectSigner(() => check(), current!.publicKey!); notice = 'The connected identity is ready for this session.'; })}>Reconnect connected identity</Button>{/if}
    <Button variant="outline" disabled={pending} onclick={() => run(() => prepare('unlink'))}>Review disconnect</Button>
  {:else if current}
    <p class="text-sm">No portable identity connected to this account.</p>
    <Button variant="outline" disabled={pending || !$signerIdentity.owner} onclick={() => run(() => prepare('link'))}>Connect selected identity to my account</Button>
    {#if !$signerIdentity.owner}<p class="text-sm">Choose your existing identity using the connection options below, then return to this button.</p>{/if}
  {/if}
  {#if review}
    <div class="space-y-2 border-t pt-3"><p class="font-medium">{review.action === 'link' ? 'Connect this identity to your account?' : 'Disconnect this identity from your account?'}</p><p class="break-all text-xs">{encodePublicKey(review.publicKey)}</p>
    <p class="text-sm">{review.action === 'link' ? 'Your signer will approve one private proof shared only with this site and that signer. The proof includes your site account identifier. Nothing is sent to a relay or merged with another account.' : 'You can do this even if you lost the signing key. Your public posts are not deleted. Operator-granted publication permissions are separate; ask the site team to revoke those if needed.'}</p>
    <label class="block text-sm"><input type="checkbox" bind:checked={approved} disabled={pending} /> I approve this exact {review.action === 'link' ? 'connection' : 'disconnect'}.</label>
    <Button disabled={pending || !approved} onclick={() => run(confirm)}>{review.action === 'link' ? 'Approve and connect' : 'Disconnect identity'}</Button><Button variant="ghost" onclick={cancel}>Cancel</Button></div>
  {/if}
  <Button variant="ghost" disabled={pending} onclick={() => run(load)}>Check current connection</Button>
  <p class="text-sm">For account recovery, use <a class="underline" href="/auth/recovery">Recover account</a>. If asked to authenticate again, <a class="underline" href="/auth/login?redirect=%2Fsettings%2Fidentity">sign in again</a> and return here. A public key alone cannot recover a site login.</p>
  {#if uncertain}<p role="status" class="text-sm">A previous confirmation is not yet certain. Check the connection first. An explicit new review safely replaces older unfinished reviews; it never repeats their confirmation.</p>{/if}
  {#if pending}<p role="status">Checking your account or waiting for your signer…</p>{/if}{#if notice}<p role="status">{notice}</p>{/if}{#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
</section>
