<script lang="ts">
  import { onMount } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { message } from './library';
  import { connectSigner, disconnectSigner, signerIdentity, relayPreferences, type Relay, type SourceMapping } from './publication';
  import { sourceResource, readMetadata, readSocial, reviewProfile, reviewFollows, reviewRelayList, reviewComment, reviewReaction, reviewWithdrawal, sendReviewed, publicActionRecords, deliverPublicAction, reconcilePublicAction, cancelPublicAction, removePublicAction, type PublicActionRecord, type SignedEvent, type PublicReview } from './protocol-identity';
  let { resourceKind, resourceId, assertViewCurrent = () => {}, selectedOrigin = $bindable(''), settingsView = 'profile' }: { resourceKind?: 'manga' | 'chapter'; resourceId?: string; assertViewCurrent?: () => void; selectedOrigin?: string; settingsView?: 'profile' | 'follows' | 'actions' | 'connections' } = $props();
  let relays = $state<Relay[]>([]), pending = $state(false), error = $state(''), notice = $state('');
  let actions = $state<PublicActionRecord[]>([]);
  let profileName = $state(''), displayName = $state(''), about = $state(''), followsText = $state(''), routesText = $state('');
  let review = $state<PublicReview>(), consent = $state(false), source = $state<SourceMapping>(), events = $state<SignedEvent[]>([]), metadata = $state<{ kind: number; event: SignedEvent | null; head: any }[]>([]), receipts = $state<any[]>([]);
  let conversationOpen = $state(false), inspectedAction = $state(''), inspectedMetadata = $state('');
  const queueState = (state: string) => ({ prepared: 'Ready to send', pending: 'Waiting to send', sending: 'Sending', retryable: 'Ready to retry', refused: 'Not sent', accepted: 'Sent', superseded: 'Replaced by a newer action', outcome_unknown: 'Delivery needs checking', projection_pending: 'Updating', cancelled: 'Cancelled' }[state] ?? 'Needs attention');
  const actionName = (kind: number) => ({ 0: 'Profile update', 3: 'People you follow', 10002: 'Posting destinations', 1: 'Comment', 1111: 'Comment', 7: 'Reaction', 5: 'Removal request' }[kind] ?? 'Public action');
  let comment = $state(''), reaction = $state('+'), replyParent = $state<SignedEvent>(), offset = $state(0);
  const relay = $derived(relays.find(row => row.origin === selectedOrigin));
  const target = $derived({ kind: resourceKind ?? 'manga', id: resourceId ?? '' });
  const transportKey = $derived(JSON.stringify([selectedOrigin, relay?.serviceKey ?? '', $signerIdentity.owner, $signerIdentity.generation]));
  const contextKey = $derived(JSON.stringify([target.kind, target.id, transportKey]));
  type Context = { key: string; generation: number; kind: 'manga' | 'chapter'; id: string; relay?: Relay; owner: string | null; signerGeneration: number };
  class ContextChanged extends Error { constructor() { super('The selected target, recipient or signing identity changed. Previous results and reviews cannot attach to this selection. Any exact signed intent already saved remains bound to its original recipient; an already sent request cannot be recalled.'); } }
  let generation = 0, activeKey = '', activeTransportKey = '', disposed = false;
  let sourceContext = $state(''), metadataContext = '', reviewContext: Context | undefined;
  function clearReview() { review = undefined; reviewContext = undefined; consent = false; }
  function currentContext(): Context {
    assertViewCurrent();
    if (disposed) throw new ContextChanged();
    // This synchronous fence also covers an input change before the effect runs.
    if (activeKey !== contextKey) {
      generation++; activeKey = contextKey;
      clearReview(); source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0;
      if (activeTransportKey !== transportKey) { metadata = []; metadataContext = ''; }
      activeTransportKey = transportKey;
    }
    return { key: activeKey, generation, kind: target.kind, id: target.id, relay: relay ? { origin: relay.origin, serviceKey: relay.serviceKey, selected: relay.selected } : undefined, owner: $signerIdentity.owner, signerGeneration: $signerIdentity.generation };
  }
  function assertContext(context: Context) {
    const current = currentContext();
    if (current.key !== context.key || current.generation !== context.generation) throw new ContextChanged();
  }
  async function contextResult<T>(context: Context, operation: Promise<T>): Promise<T> {
    try { const result = await operation; assertContext(context); return result; }
    catch (error) { assertContext(context); throw error; }
  }
  function contextRelay(context: Context) { if (!context.relay) throw Error('Choose a verified instance in Library settings first'); return context.relay; }
  function selectedRelay() { return contextRelay(currentContext()); }
  function selectedSource(context: Context) {
    assertContext(context);
    if (!source || sourceContext !== context.key || source.kind !== context.kind || source.id !== context.id) throw Error('Load the verified conversation for the current target before reviewing an action');
    return $state.snapshot(source);
  }
  async function run(operation: () => Promise<void>) {
    if (pending || disposed) return; pending = true; error = '';
    try { await operation(); } catch (cause) { if (!disposed) { if (cause instanceof ContextChanged) notice = cause.message; else error = message(cause); } }
    finally { if (!disposed) { const saved = await publicActionRecords().catch(() => undefined); if (!disposed) { if (saved) actions = saved; pending = false; } } }
  }
  function setReview(value: PublicReview) {
    const context = currentContext(), recipient = contextRelay(context);
    if (value.relay.origin !== recipient.origin || value.relay.serviceKey !== recipient.serviceKey || value.owner !== context.owner || value.generation !== context.signerGeneration || (value.source && (value.source.kind !== context.kind || value.source.id !== context.id))) throw new ContextChanged();
    review = value; reviewContext = context; consent = false;
  }
  function setWithdrawal(event: SignedEvent, collection: 'metadata' | 'social') {
    const context = currentContext();
    if (collection === 'social') { selectedSource(context); if (!events.some(row => row.id === event.id)) throw new ContextChanged(); }
    else if (metadataContext !== transportKey || !metadata.some(row => row.event?.id === event.id)) throw new ContextChanged();
    setReview(reviewWithdrawal(contextRelay(context), $state.snapshot(event)));
  }
  async function refreshRelays() { const preferences = await relayPreferences(), saved = await publicActionRecords(); if (disposed) return; relays = preferences.relays; actions = saved; if (!relays.some(row => row.origin === selectedOrigin)) selectedOrigin = ''; }
  async function refreshSocial(more = false) {
    const context = currentContext(), recipient = contextRelay(context);
    const previousSource = more ? selectedSource(context) : undefined;
    const next = more ? offset + 50 : 0, previous = more ? events : [];
    const mapped = await contextResult(context, sourceResource(context.kind, context.id));
    if (previousSource && previousSource.event_id !== mapped.event_id) { clearReview(); source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0; generation++; throw Error('The canonical source mapping changed. Reload this conversation before continuing.'); }
    const rows = await contextResult(context, readSocial(recipient, mapped, next));
    if (source?.event_id !== mapped.event_id) { if (source) generation++; clearReview(); replyParent = undefined; comment = ''; reaction = '+'; }
    source = mapped; sourceContext = context.key; events = [...previous, ...rows]; offset = next;
    notice = 'Showing verified public source comments and reactions from the selected instance.';
  }
  async function refreshMetadata() {
    const context = currentContext(), recipient = contextRelay(context), owner = context.owner;
    if (!owner) throw Error('Connect a browser signer to select the public identity');
    const current = [];
    for (const kind of [0, 3, 10002] as const) {
      try { const result = await contextResult(context, readMetadata(recipient, owner, kind)); current.push({ kind, ...result }); }
      catch (cause) { assertContext(context); if ((cause as Error & { status?: number }).status !== 404) throw cause; }
    }
    assertContext(context);
    metadata = current; metadataContext = transportKey; notice = 'Verified current public metadata heads. Projection-pending heads have no verified visible event yet.';
  }
  async function sendCurrentReview() {
    const selected = review ? $state.snapshot(review) : undefined, context = reviewContext;
    if (!selected || !context || !consent) throw Error('Review and approve the current public action first');
    assertContext(context); clearReview();
    const result = await contextResult(context, sendReviewed(selected, true, () => assertContext(context)));
    if (disposed) return;
    receipts = [{ ...result, recipient: selected.relay.origin, target: selected.source?.event_id }, ...receipts].slice(0, 20);
    notice = `Exact signed event ${result.event.id} at ${selected.relay.origin}: ` + (result.receipt.status === 409 ? 'superseded by a newer public event.' : selected.kind === 5 ? 'withdrawal request accepted; verify after projection.' : result.receipt.data.projected ? 'accepted and projected; refresh its verified counterpart.' : 'accepted; projection pending.');
  }
  function toggleConversation() {
    conversationOpen = !conversationOpen;
    if (!conversationOpen) { generation++; clearReview(); source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0; }
  }
  $effect(() => { contextKey; currentContext(); });
  onMount(() => { void run(refreshRelays); return () => { disposed = true; generation++; }; });
</script>
{#snippet recipientControls()}
    <label>Verified recipient <select class="rounded border px-2" bind:value={selectedOrigin} disabled={pending}><option value="">Choose instance</option>{#each relays as recipient}<option value={recipient.origin}>{recipient.origin}</option>{/each}</select></label>
    <Button variant="outline" disabled={pending} onclick={() => run(refreshRelays)}>Refresh verified instances</Button>
{/snippet}
{#snippet reviewedAction()}
    {#if review}
      <div class="rounded border p-3 space-y-2">
        <p>{review.purpose}. Recipient: {review.relay.origin} · pinned service key {review.relay.serviceKey}</p>
        <p class="break-all text-xs">Author {review.owner} · kind {review.kind} · exact tags {JSON.stringify(review.tags)}</p>
        <p class="whitespace-pre-wrap break-words">{review.content || '(empty signed content)'}</p>
        <p>{review.kind === 5 ? 'This publishes a signed withdrawal request. Copies elsewhere may remain; source publications have a separate protected policy.' : 'The content, tags and author identity become public. Replaceable lists replace the previous public list at this instance.'}</p>
        <label><input type="checkbox" bind:checked={consent} /> I approve this exact public action and recipient.</label>
        <Button disabled={pending || !consent} onclick={() => run(sendCurrentReview)}>Sign and send reviewed public action</Button>
        <Button variant="outline" disabled={pending} onclick={clearReview}>Cancel review</Button>
      </div>
    {/if}
{/snippet}
<section class={resourceId ? 'space-y-3' : 'space-y-5'} data-public-identity-tool={resourceId ? undefined : settingsView} data-source-conversation={resourceId ? true : undefined} data-resource-kind={resourceId ? resourceKind : undefined} data-resource-id={resourceId} data-verified-source-id={sourceContext === contextKey && source?.kind === target.kind && source?.id === target.id ? source.event_id : undefined}>
  {#if resourceId}
    <Button variant="ghost" size="sm" aria-expanded={conversationOpen} onclick={toggleConversation}>Signed comments</Button>
    {#if conversationOpen}
      <div class="space-y-3 border-t pt-4">
        <p class="text-muted-foreground text-sm">Public comments and reactions for this {resourceKind === 'chapter' ? 'chapter' : 'title'} use an external signing identity. <a class="underline" href="/settings/identity">Manage public identity</a></p>
        {#if !relays.length}<p class="text-sm">To choose where to post, <a class="underline" href="/settings/library">set up public sync in Library settings</a>.</p>{:else}{@render recipientControls()}{/if}
        <Button variant="outline" disabled={pending || !relay} onclick={() => run(() => refreshSocial())}>Load verified public conversation</Button>
        {#if source}
          <details class="text-sm"><summary class="cursor-pointer text-muted-foreground">Source details</summary><p class="break-all text-xs" data-verified-source-id={source.event_id}>Verified canonical source: {source.event_id} · publisher {source.publisher} · kind {source.source_kind}</p></details>
          {#if replyParent}<p class="break-all">Replying to {replyParent.id} by {replyParent.pubkey} <Button variant="ghost" onclick={() => { replyParent = undefined; clearReview(); }}>Cancel reply</Button></p>{/if}
          <label class="block" for="public-source-comment">Public comment <Textarea id="public-source-comment" class="mt-1" bind:value={comment} maxlength={4000}></Textarea></label>
          <Button disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); setReview(reviewComment(contextRelay(context), selectedSource(context), comment, replyParent)); })}>Review public {replyParent ? 'reply' : 'comment'}</Button>
          <label for="public-source-reaction">Public reaction <Input id="public-source-reaction" class="mt-1 w-24" bind:value={reaction} maxlength={64} /></label>
          <Button variant="outline" disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); setReview(reviewReaction(contextRelay(context), selectedSource(context), reaction)); })}>Review public reaction</Button>
        {/if}
        {#each events as event}
          <article class="space-y-3 border-t pt-4"><p class="break-all text-xs">{event.kind === 1111 ? 'Comment' : 'Reaction'} · {event.pubkey} · signed at {event.created_at} · {event.id}</p><p class="whitespace-pre-wrap break-words">{event.content}</p>
            {#if event.kind === 1111}<Button variant="ghost" disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); selectedSource(context); if (!events.some(row => row.id === event.id)) throw new ContextChanged(); replyParent = event; comment = ''; clearReview(); })}>Reply to this verified comment</Button>{/if}
            {#if event.pubkey === $signerIdentity.owner}<Button variant="ghost" disabled={pending} onclick={() => run(async () => { setWithdrawal(event, 'social'); })}>Review authored event withdrawal</Button>{/if}
          </article>
        {/each}
        {#if events.length && events.length % 50 === 0 && offset < 10000}<Button variant="outline" disabled={pending} onclick={() => run(() => refreshSocial(true))}>Load more verified events</Button>{/if}
        {@render reviewedAction()}
      </div>
    {/if}
  {:else}
    {#if settingsView === 'connections'}
      <p class="text-muted-foreground text-sm">Check the signing identity and verified instance used for public posts. These are separate from your site login.</p>
      <p class="break-all text-sm">Signing public key: {$signerIdentity.owner ?? 'none connected'}</p>
      {@render recipientControls()}
      <p class="text-muted-foreground text-sm">Add or verify destinations from <a class="underline" href="/settings/library">Library settings</a> → Set up public sync.</p>
    {:else if settingsView === 'profile' || settingsView === 'follows'}
      <p class="text-muted-foreground text-sm">Changes are public only after you review and approve them. Choose the instance where you want them published.</p>
      {@render recipientControls()}
      {#if settingsView === 'profile'}
        <label class="block" for="public-profile-name">Public username <Input id="public-profile-name" class="mt-1" bind:value={profileName} maxlength={128} /></label>
        <label class="block" for="public-profile-display-name">Public display name <Input id="public-profile-display-name" class="mt-1" bind:value={displayName} maxlength={128} /></label>
        <label class="block" for="public-profile-bio">Public bio <Textarea id="public-profile-bio" class="mt-1" bind:value={about} maxlength={4000}></Textarea></label>
        <Button disabled={pending || !relay || !$signerIdentity.owner} onclick={() => run(async () => { setReview(reviewProfile(selectedRelay(), { name: profileName, display_name: displayName, about })); })}>Review public profile</Button>
      {:else}
        <p class="text-muted-foreground text-sm">Public follows use signing public keys. These are people you follow, separate from your saved manga.</p>
        <label class="block" for="public-follow-keys">Public follow keys, one 64-character hex key per line <Textarea id="public-follow-keys" class="mt-1" bind:value={followsText}></Textarea></label>
        <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner} onclick={() => run(async () => { setReview(reviewFollows(selectedRelay(), followsText.split(/\s+/).filter(Boolean))); })}>Review replacement public follow list</Button>
        <label class="block" for="public-write-relays">Public write relay URLs, one WSS URL per line <Textarea id="public-write-relays" class="mt-1" bind:value={routesText}></Textarea></label>
        <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner} onclick={() => run(async () => { setReview(reviewRelayList(selectedRelay(), routesText.split(/\s+/).filter(Boolean))); })}>Review replacement public relay list</Button>
      {/if}
      <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner} onclick={() => run(refreshMetadata)}>Read verified current public metadata</Button>
      {#each metadata.filter(item => settingsView === 'profile' ? item.kind === 0 : item.kind !== 0) as item}
        <div class="space-y-3 border-t pt-4" data-verified-metadata-kind={item.kind} data-verified-metadata-id={item.head.id}>
          <p>{actionName(item.kind)} · {item.head.projected ? 'Current' : 'Updating'}</p>
          {#if item.event}<Button variant="ghost" onclick={() => { inspectedMetadata = inspectedMetadata === item.head.id ? '' : item.head.id; }}>Inspect current data</Button>{#if inspectedMetadata === item.head.id}<p class="whitespace-pre-wrap break-words">{item.event.content || JSON.stringify(item.event.tags)}</p>{/if}<Button variant="outline" disabled={pending} onclick={() => run(async () => { setWithdrawal(item.event!, 'metadata'); })}>Review removal</Button>{/if}
        </div>
      {/each}
      {@render reviewedAction()}
    {:else}
      <p class="text-muted-foreground text-sm">Check deliveries, retry a saved action, or stop further sending. Already published copies stay public.</p>
      <Button variant="outline" disabled={pending} onclick={() => run(async () => { actions = await publicActionRecords(); })}>Refresh saved actions</Button>
      {#if !actions.length}<p class="text-muted-foreground text-sm">No saved public actions.</p>{/if}
      {#each actions as action}
        <div class="space-y-3 border-t pt-4" data-public-action-id={action.event.id} data-action-state={action.state}>
          <p class="font-medium">{actionName(action.event.kind)} · {new Date(action.event.created_at * 1000).toLocaleString()}</p>
          <p>{action.cancelled ? 'Further sending cancelled' : queueState(action.state)}: {action.message}</p>
          <p class="text-sm">To {action.relay.origin}</p>
          <div class="flex flex-wrap gap-2">
            <Button variant="outline" disabled={pending} onclick={() => run(async () => { await reconcilePublicAction(action.event.id); notice = 'Delivery checked. Review its saved status.'; })}>Check delivery</Button>
            {#if !action.cancelled && !['accepted', 'superseded'].includes(action.state)}
              <Button variant="outline" disabled={pending || $signerIdentity.owner !== action.event.pubkey} onclick={() => { if (confirm(`Retry only this previously reviewed signed event ${action.event.id} to ${action.relay.origin}? Its body is unchanged. A fresh HTTP authentication proof will be requested.`)) void run(async () => { const context = currentContext(); await deliverPublicAction(action.event.id, () => assertContext(context)); notice = 'Saved action retry finished.'; }); }}>Retry saved action</Button>
              <Button variant="outline" onclick={() => { void cancelPublicAction(action.event.id).then(async () => { if (!disposed) actions = await publicActionRecords(); }).catch(cause => { if (!disposed) error = message(cause); }); }}>Cancel further sending</Button>
            {:else}
              <Button variant="outline" disabled={pending} onclick={() => { if (confirm('Remove this local signed-action retry record? Public copies are unaffected.')) void run(async () => { await removePublicAction(action.event.id); }); }}>Remove local retry record</Button>
            {/if}
            <Button variant="ghost" onclick={() => { inspectedAction = inspectedAction === action.event.id ? '' : action.event.id; }}>Inspect saved action</Button>
          </div>
          {#if inspectedAction === action.event.id}<div class="space-y-2 text-xs"><p class="break-all">Signed event {action.event.id} · signer {action.event.pubkey} · kind {action.event.kind}</p><p class="break-all">Original tags: {JSON.stringify(action.event.tags)}</p><p class="whitespace-pre-wrap break-words">{action.event.content || '(empty signed content)'}</p></div>{/if}
        </div>
      {/each}
    {/if}
  {/if}
  {#if !resourceId || conversationOpen}
    {#each receipts as result}<p class="break-all text-xs">Signed event receipt {result.event.id} at {result.recipient}{result.target ? ' · canonical target ' + result.target : ''}: HTTP {result.receipt.status} · {JSON.stringify(result.receipt.data)}</p>{/each}
    <p role="status">{pending ? 'Waiting for the selected signer or instance…' : notice}</p>
    {#if error}<p role="alert" class="text-red-600">{error}</p>{/if}
  {/if}
</section>
