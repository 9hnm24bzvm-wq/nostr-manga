<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { prepareRemoteSigner, selectRemoteSigner, cancelRemoteSigner, remoteSignerStatus } from './remote-signer-session';
  import { forgetBunkerReview, type BunkerReview } from './remote-bunker';
  import { publicIdentityLabel } from './public-identity-input';
  let { assertCurrent = () => {} }: { assertCurrent?: () => void } = $props();
  let input = $state(''), review = $state.raw<BunkerReview>(), approved = $state(false), pending = $state(false), error = $state(''), connected = $state(false);
  let alive = true, epoch = 0;
  function check(captured = epoch) { assertCurrent(); if (!alive || captured !== epoch) throw Error('This signer review was closed or changed.'); }
  function cancel() { epoch++; if (pending) cancelRemoteSigner(); if (review) forgetBunkerReview(review); review = undefined; input = ''; approved = false; pending = false; }
  async function inspect() { if (pending) return; const captured = epoch; pending = true; error = ''; try { const next = await prepareRemoteSigner(input, () => check(captured)); check(captured); if (review) forgetBunkerReview(review); review = next; input = ''; approved = false; } catch (cause) { if (alive) error = cause instanceof Error ? cause.message : 'The signer link could not be reviewed.'; } finally { if (alive && captured === epoch) pending = false; } }
  async function connect() { if (!review || !approved || pending) return; const captured = epoch; pending = true; error = ''; try { await selectRemoteSigner(review, true, () => check(captured)); check(captured); connected = true; review = undefined; approved = false; } catch (cause) { if (alive) error = cause instanceof Error ? cause.message : 'The remote signer could not be connected.'; } finally { if (alive && captured === epoch) pending = false; } }
  onMount(() => () => { alive = false; if (pending) cancelRemoteSigner(); if (review) forgetBunkerReview(review); input = ''; });
</script>
<section class="space-y-4" data-remote-signer-panel>
  <p>Connect an existing remote signer using its bunker link. Your normal site account does not need this. No private recovery key is imported or created.</p>
  <p class="text-sm">Supports direct bunker connections using NIP-44 v2 encryption and one reviewed relay. Older NIP-04-only signers are not supported. This does not grant site publishing or staff permission.</p>
  <label class="block" for="remote-bunker-link">Connection link from your trusted signer<Input id="remote-bunker-link" type="password" maxlength={4096} autocomplete="off" spellcheck={false} bind:value={input} disabled={pending} oninput={() => { if (review) forgetBunkerReview(review); review = undefined; approved = false; }} /></label>
  <Button variant="outline" disabled={pending || !input} onclick={inspect}>Review remote signer</Button>
  {#if review}<div class="space-y-3 rounded border p-3"><strong>Check the signer and relay</strong><p class="break-all text-xs">Signer service: {publicIdentityLabel(review.bunkerKey)}</p><p class="break-all text-sm">Relay: {review.relay}</p><p class="text-sm">The relay can see your IP address, this site’s origin, timing and temporary connection identity. Your browser may also send cookies already belonging to that relay host. Encrypted requests are readable by your signer. A temporary transport key stays in memory only and is discarded on disconnect or sign-out; it is not a new user identity.</p><p class="text-sm">Permissions requested: approve public profile, follows, relay list, comments, reactions, withdrawals, public library snapshots and request proofs. No private-message decryption or key export is requested. Each site action still requires its own review.</p><p class="text-sm">If this account already has a connected portable identity, the returned identity must match it exactly. This step does not link or merge accounts.</p><label class="block"><input type="checkbox" bind:checked={approved} disabled={pending} /> I trust this signer and approve contact with this exact relay.</label><Button disabled={pending || !approved} onclick={connect}>Connect reviewed remote signer</Button></div>{/if}
  {#if review || pending}<Button variant="ghost" onclick={cancel}>Cancel signer connection</Button>{/if}
  {#if pending}<p role="status">Waiting for the reviewed signer. Approval requests appear below; nothing is retried automatically.</p>{/if}
  {#if connected}<p role="status">{$remoteSignerStatus.message} Return to Account settings to connect this identity privately to your account, or review a public action.</p>{/if}
  {#if error}<p role="alert" class="text-destructive">{error}</p>{/if}
</section>
