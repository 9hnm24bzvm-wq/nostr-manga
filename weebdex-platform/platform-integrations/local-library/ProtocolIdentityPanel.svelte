<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Input } from '$lib/components/ui/input';
  import { Textarea } from '$lib/components/ui/textarea';
  import { message } from './library';
  import { publicIdentityKey, publicIdentityLabel, lookupPublicIdentity } from './public-identity-input';
  import { connectSigner, disconnectSigner, signerIdentity, relayPreferences, type Relay, type SourceMapping } from './publication';
  import { sourceResource, metadataForEditing, profileFields, relayLines, readSocial, reviewProfile, reviewFollows, reviewRelayList, reviewComment, reviewReaction, reviewWithdrawal, sendReviewed, publicActionRecords, deliverPublicAction, reconcilePublicAction, cancelPublicAction, removePublicAction, type PublicActionRecord, type SignedEvent, type PublicReview, type MetadataBase } from './protocol-identity';
  let { resourceKind, resourceId, assertViewCurrent = () => {}, selectedOrigin = $bindable(''), settingsView = 'profile' }: { resourceKind?: 'manga' | 'chapter'; resourceId?: string; assertViewCurrent?: () => void; selectedOrigin?: string; settingsView?: 'profile' | 'follows' | 'actions' | 'connections' | 'lookup' } = $props();
  let relays = $state<Relay[]>([]), pending = $state(false), error = $state(''), notice = $state('');
  let actions = $state<PublicActionRecord[]>([]);
  let lookupText = $state(''), lookupConsent = $state(false), lookupResult = $state<Awaited<ReturnType<typeof lookupPublicIdentity>>>();
  let profileName = $state(''), displayName = $state(''), about = $state(''), followsText = $state(''), routesText = $state('');
  let profileExtra = $state<Record<string, string>>({}), removedFields = $state<string[]>([]), loadedFields = $state<string[]>([]);
  let metadataLoaded = $state(false);
  const extraLabels: Record<string, string> = { picture: 'Profile picture URL', banner: 'Banner image URL', nip05: 'Public address (NIP-05)', lud16: 'Lightning address' };
  let review = $state<PublicReview>(), consent = $state(false), source = $state<SourceMapping>(), events = $state<SignedEvent[]>([]), metadata = $state<MetadataBase[]>([]), receipts = $state<any[]>([]);
  let sourcePublishingOpen = $state(false), conversationOpen = $state(false), inspectedAction = $state(''), inspectedMetadata = $state('');
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
      clearReview(); sourcePublishingOpen = false; source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0;
      if (activeTransportKey !== transportKey) { metadata = []; metadataContext = ''; metadataLoaded = false; lookupResult = undefined; profileName = ''; displayName = ''; about = ''; profileExtra = {}; removedFields = []; loadedFields = []; followsText = ''; routesText = ''; }
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
  async function refreshRelays() { const preferences = await relayPreferences(), saved = await publicActionRecords(); if (disposed) return; relays = preferences.relays; actions = saved; if (!relays.some(row => row.origin === selectedOrigin)) selectedOrigin = preferences.relays.find(row => row.selected)?.origin ?? (relays.length === 1 ? relays[0].origin : ''); }
  async function refreshSocial(more = false) {
    const context = currentContext(), recipient = contextRelay(context);
    const previousSource = more ? selectedSource(context) : undefined;
    const next = more ? offset + 50 : 0, previous = more ? events : [];
    const mapped = await contextResult(context, sourceResource(context.kind, context.id, () => assertContext(context)));
    if (previousSource && previousSource.event_id !== mapped.event_id) { clearReview(); sourcePublishingOpen = false; source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0; generation++; throw Error('The canonical source mapping changed. Reload this conversation before continuing.'); }
    const rows = await contextResult(context, readSocial(recipient, mapped, next));
    if (source?.event_id !== mapped.event_id) { if (source) generation++; clearReview(); replyParent = undefined; comment = ''; reaction = '+'; }
    source = mapped; sourceContext = context.key; events = [...previous, ...rows]; offset = next;
    notice = 'Showing verified public source comments and reactions from the selected instance.';
  }
  function editBase(kind: 0 | 3 | 10002) {
    currentContext();
    const base = metadata.find(row => row.kind === kind);
    if (!metadataLoaded || metadataContext !== transportKey || !base) throw Error('Load your current public information before editing.');
    return $state.snapshot(base);
  }
  function profilePatch() {
    const before = profileFields(editBase(0)), patch: Record<string, string> = {};
    // Do not turn absent or unfamiliar values into blank strings on an unrelated edit.
    for (const [key, value] of Object.entries({ name: profileName, display_name: displayName, about, ...profileExtra })) {
      if (value !== (typeof before[key] === 'string' ? before[key] : '')) patch[key] = value;
    }
    return patch;
  }
  async function refreshMetadata() {
    const context = currentContext(), recipient = contextRelay(context), owner = context.owner;
    if (!owner) throw Error('Connect your public identity before editing.');
    const current: MetadataBase[] = [];
    for (const kind of [0, 3, 10002] as const) current.push(await contextResult(context, metadataForEditing(recipient, owner, kind)));
    assertContext(context);
    if (current.some(row => row.head && !row.event)) throw Error('Your public information is still updating. Try loading it again shortly; existing data has not been changed.');
    const fields = profileFields(current[0]), text = (key: string) => typeof fields[key] === 'string' ? fields[key] as string : '';
    const nextFollows = [...new Set((current[1].event?.tags ?? []).filter(tag => tag[0] === 'p').map(tag => publicIdentityLabel(tag[1])))].join('\n');
    const nextRoutes = relayLines(current[2]).join('\n');
    // Complete every parse before replacing the current editable snapshot.
    metadata = current; metadataContext = transportKey; metadataLoaded = true;
    profileName = text('name'); displayName = text('display_name'); about = text('about');
    profileExtra = Object.fromEntries(Object.keys(extraLabels).map(key => [key, text(key)])); removedFields = []; loadedFields = Object.keys(fields);
    followsText = nextFollows; routesText = nextRoutes; clearReview();
    notice = 'Your current public information is ready to edit. Unchanged information is preserved.';
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
    if (!conversationOpen) { generation++; clearReview(); sourcePublishingOpen = false; source = undefined; sourceContext = ''; events = []; replyParent = undefined; comment = ''; reaction = '+'; offset = 0; }
  }
  $effect(() => { contextKey; currentContext(); });
  onMount(() => { void run(refreshRelays); return () => { disposed = true; generation++; }; });
</script>
{#snippet recipientControls()}
    <label>Verified recipient <select class="rounded border px-2" bind:value={selectedOrigin} disabled={pending}><option value="">Choose instance</option>{#each relays as recipient}<option value={recipient.origin}>{recipient.origin}</option>{/each}</select></label>
    <Button variant="outline" disabled={pending} onclick={() => run(refreshRelays)}>Refresh destinations</Button>
    {#if !relays.length}<p class="text-sm">No destination selected yet. <a class="underline" href="/settings/library">Add a verified site in Library → Public sync</a>, then return here. Adding a site does not publish anything.</p>{/if}
{/snippet}
{#snippet publicLookup()}
  <div class="space-y-2 rounded border p-3">
    <label class="block">Find a public profile<Input bind:value={lookupText} placeholder="npub… or reader@example.com" disabled={pending} oninput={() => { lookupResult = undefined; }} /></label>
    <p class="text-sm">Read-only: no signer, login or private key is needed. Names can be shared by different people; check the npub with the person before following.</p>
    {#if lookupText.includes('@')}<label class="block text-sm"><input type="checkbox" bind:checked={lookupConsent} /> I approve contacting this public address’s domain. It will see my network request; no account cookie is sent.</label>{/if}
    <Button variant="outline" disabled={pending || !relay || !lookupText.trim() || lookupText.includes('@') && !lookupConsent} onclick={() => run(async () => { const context = currentContext(), input = lookupText; const found = await lookupPublicIdentity(input, contextRelay(context), lookupConsent, () => assertContext(context)); assertContext(context); if (input !== lookupText) throw Error('The lookup changed. Search again.'); lookupResult = found; })}>Look up public profile</Button>
    {#if lookupResult}<p class="font-medium">{lookupResult.displayName}</p><p class="whitespace-pre-wrap">{lookupResult.about}</p><p class="break-all text-xs">{lookupResult.npub}</p>{#if lookupResult.address}<p class="text-sm">Address matches this public key: {lookupResult.address}. This identifies an account, not a trusted person.</p>{/if}
      {#if settingsView === 'follows' && metadataLoaded}<Button variant="outline" disabled={pending} onclick={() => { const key = lookupResult!.publicKey, keys = followsText.split(/\s+/).filter(Boolean).map(publicIdentityKey); if (!keys.includes(key)) followsText = [...keys, key].map(publicIdentityLabel).join('\n'); clearReview(); notice = 'Added to your unsaved follow draft. Review and approve before it becomes public.'; }}>Add this exact identity to follow draft</Button>{/if}
    {/if}
  </div>
{/snippet}
{#snippet reviewedAction()}
    {#if review}
      <div class="rounded border p-3 space-y-2">
        <p>{review.purpose}. Recipient: {review.relay.origin} · pinned service key {review.relay.serviceKey}</p>
        <p class="break-all text-xs">Author {review.owner} · kind {review.kind} · exact tags {JSON.stringify(review.tags)}</p>
        <p class="whitespace-pre-wrap break-words">{review.content || '(empty signed content)'}</p>
        {#if review.changes}<div data-public-change-summary><p>Added: {review.changes.added.join(', ') || 'None'}</p><p>Changed: {review.changes.changed.join(', ') || 'None'}</p><p class={review.changes.removed.length ? 'font-semibold text-destructive' : ''}>Removed: {review.changes.removed.join(', ') || 'None — all other information is preserved'}</p></div>{/if}
        <p>{review.kind === 5 ? 'This publishes a signed withdrawal request. Copies elsewhere may remain; source publications have a separate protected policy.' : 'These exact changes become public. They replace the current profile or list at this destination; removed fields and contacts are listed above.'}</p>
        <label><input type="checkbox" bind:checked={consent} /> I approve this exact public action and recipient.</label>
        <Button disabled={pending || !consent} onclick={() => run(sendCurrentReview)}>Sign and send reviewed public action</Button>
        <Button variant="outline" disabled={pending} onclick={clearReview}>Cancel review</Button>
      </div>
    {/if}
{/snippet}
<section oninput={(event) => { if ((event.target as HTMLElement)?.closest('[data-public-editor-fields]') && review) clearReview(); }} class={resourceId ? 'space-y-3' : 'space-y-5'} data-public-identity-tool={resourceId ? undefined : settingsView} data-source-conversation={resourceId ? true : undefined} data-resource-kind={resourceId ? resourceKind : undefined} data-resource-id={resourceId} data-verified-source-id={sourceContext === contextKey && source?.kind === target.kind && source?.id === target.id ? source.event_id : undefined}>
  {#if resourceId}
    <Button variant="ghost" size="sm" aria-expanded={conversationOpen} onclick={toggleConversation}>Comments from other sites</Button>
    {#if conversationOpen}
      <div class="space-y-3 border-t pt-4">
        <p class="text-muted-foreground text-sm">These comments were published outside this site. They are separate from the conversation above. You can read them without a signing identity.</p>
        {#if !relays.length}<p class="text-sm">To read a source site, <a class="underline" href="/settings/library">add it in Library settings</a> → Public sync. This does not publish anything.</p>{:else}{@render recipientControls()}{/if}
        <Button variant="outline" disabled={pending || !relay} onclick={() => run(() => refreshSocial())}>Load verified public conversation</Button>
        {#if source}
          <details class="text-sm"><summary class="cursor-pointer text-muted-foreground">Source details</summary><p class="break-all text-xs" data-verified-source-id={source.event_id}>Verified canonical source: {source.event_id} · publisher {source.publisher} · kind {source.source_kind}</p></details>
          <details class="space-y-3 rounded border p-3" data-source-publishing bind:open={sourcePublishingOpen}><summary class="cursor-pointer font-medium">Post to the source site (advanced)</summary>
          <p class="text-sm">Posting outside this site requires an external signing identity. Your site login does not sign messages. Nothing is sent until you review the exact public post and approve it. <a class="underline" href="/settings/identity">Advanced identity settings</a></p>
          {#if replyParent}<p class="break-all">Replying to a source comment <Button variant="ghost" onclick={() => { replyParent = undefined; clearReview(); }}>Cancel reply</Button></p>{/if}
          <label class="block" for="public-source-comment">Public comment <Textarea id="public-source-comment" class="mt-1" bind:value={comment} maxlength={4000}></Textarea></label>
          <Button disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); setReview(reviewComment(contextRelay(context), selectedSource(context), comment, replyParent)); })}>Review public {replyParent ? 'reply' : 'comment'}</Button>
          <label for="public-source-reaction">Public reaction <Input id="public-source-reaction" class="mt-1 w-24" bind:value={reaction} maxlength={64} /></label>
          <Button variant="outline" disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); setReview(reviewReaction(contextRelay(context), selectedSource(context), reaction)); })}>Review public reaction</Button>
          </details>
        {/if}
        {#each events as event}
          <article class="space-y-3 border-t pt-4"><p class="text-sm">{event.kind === 1111 ? 'Comment' : 'Reaction'} from the source site · {new Date(event.created_at * 1000).toLocaleString()}</p><p class="whitespace-pre-wrap break-words">{event.content}</p><details><summary class="cursor-pointer text-xs">Author and source details</summary><p class="break-all text-xs">Author: {event.pubkey} · Post: {event.id}</p></details>
            {#if event.kind === 1111}<Button variant="ghost" disabled={pending || !$signerIdentity.owner} onclick={() => run(async () => { const context = currentContext(); selectedSource(context); if (!events.some(row => row.id === event.id)) throw new ContextChanged(); replyParent = event; comment = ''; clearReview(); sourcePublishingOpen = true; await tick(); assertContext(context); document.getElementById('public-source-comment')?.focus(); })}>Reply to this verified comment</Button>{/if}
            {#if event.pubkey === $signerIdentity.owner}<Button variant="ghost" disabled={pending} onclick={() => run(async () => { setWithdrawal(event, 'social'); })}>Review authored event withdrawal</Button>{/if}
          </article>
        {/each}
        {#if events.length && events.length % 50 === 0 && offset < 10000}<Button variant="outline" disabled={pending} onclick={() => run(() => refreshSocial(true))}>Load more verified events</Button>{/if}
        {@render reviewedAction()}
      </div>
    {/if}
  {:else}
    {#if settingsView === 'connections' || settingsView === 'lookup'}
      <p class="text-muted-foreground text-sm">Check the signing identity and verified instance used for public posts. These are separate from your site login.</p>
      <p class="break-all text-sm">Selected public identity: {$signerIdentity.owner ? publicIdentityLabel($signerIdentity.owner) : 'none — public lookup does not need one'}</p>
      {@render recipientControls()}
      {@render publicLookup()}
      <p class="text-muted-foreground text-sm">Add or verify destinations from <a class="underline" href="/settings/library">Library settings</a> → Set up public sync.</p>
    {:else if settingsView === 'profile' || settingsView === 'follows'}
      <p class="text-muted-foreground text-sm">Changes are public only after you review and approve them. Choose the instance where you want them published.</p>
      {@render recipientControls()}
      <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner} onclick={() => { if (!metadataLoaded || confirm('Reload current public information and discard your unsaved edits?')) void run(refreshMetadata); }}>{metadataLoaded ? 'Reload current information' : 'Load my current information'}</Button>
      {#if !metadataLoaded}<p class="text-sm">Load your profile and lists first so existing information is preserved.</p>{/if}
      <div data-public-editor-fields class="space-y-3">
      {#if settingsView === 'profile'}
        <label class="block" for="public-profile-name">Public username <Input id="public-profile-name" disabled={!metadataLoaded || pending} class="mt-1" bind:value={profileName} maxlength={128} /></label>
        <label class="block" for="public-profile-display-name">Public display name <Input id="public-profile-display-name" disabled={!metadataLoaded || pending} class="mt-1" bind:value={displayName} maxlength={128} /></label>
        <label class="block" for="public-profile-bio">Public bio <Textarea id="public-profile-bio" disabled={!metadataLoaded || pending} class="mt-1" bind:value={about} maxlength={4000}></Textarea></label>
        <details class="space-y-3"><summary class="cursor-pointer">Pictures, addresses and other profile fields</summary>
          {#each Object.entries(extraLabels) as [key, label]}<label class="block">{label}<Input bind:value={profileExtra[key]} disabled={!metadataLoaded || pending} /></label>{/each}
          <p class="text-sm">Custom fields stay unchanged. To remove a field deliberately, select it below and check the removal summary before publishing.</p>
          {#each loadedFields as field}<label class="block"><input type="checkbox" value={field} bind:group={removedFields} disabled={pending} /> Remove {field}</label>{/each}
        </details>
        <Button disabled={pending || !relay || !$signerIdentity.owner || !metadataLoaded} onclick={() => run(async () => { setReview(reviewProfile(selectedRelay(), profilePatch(), editBase(0), removedFields)); })}>Review public profile</Button>
      {:else}
        {@render publicLookup()}
        <p class="text-muted-foreground text-sm">Public follows use public identities. These are people you follow, separate from your saved manga.</p>
        <label class="block" for="public-follow-keys">People you follow, one public npub per line (public hex keys also work) <Textarea id="public-follow-keys" disabled={!metadataLoaded || pending} class="mt-1" bind:value={followsText}></Textarea></label>
        <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner || !metadataLoaded} onclick={() => run(async () => { setReview(reviewFollows(selectedRelay(), followsText.split(/\s+/).filter(Boolean).map(publicIdentityKey), editBase(3))); })}>Review replacement public follow list</Button>
        <label class="block" for="public-write-relays">Public relay URLs, one WSS URL per line; optionally add read or write <Textarea id="public-write-relays" disabled={!metadataLoaded || pending} class="mt-1" bind:value={routesText}></Textarea></label>
        <Button variant="outline" disabled={pending || !relay || !$signerIdentity.owner || !metadataLoaded} onclick={() => run(async () => { setReview(reviewRelayList(selectedRelay(), routesText.split(/\n/).map(line => line.trim()).filter(Boolean), editBase(10002))); })}>Review replacement public relay list</Button>
      {/if}
      </div>
      {#each metadata.filter(item => settingsView === 'profile' ? item.kind === 0 : item.kind !== 0) as item}
        <div class="space-y-3 border-t pt-4" data-verified-metadata-kind={item.kind} data-verified-metadata-id={item.head?.id}>
          <p>{actionName(item.kind)} · {item.head?.projected ? 'Current' : item.head ? 'Updating' : 'No existing information'}</p>
          {#if item.event}<Button variant="ghost" onclick={() => { inspectedMetadata = inspectedMetadata === item.head?.id ? '' : item.head!.id; }}>Inspect current data</Button>{#if inspectedMetadata === item.head?.id}<p class="whitespace-pre-wrap break-words">{item.event.content || JSON.stringify(item.event.tags)}</p>{/if}<Button variant="outline" disabled={pending} onclick={() => run(async () => { setWithdrawal(item.event!, 'metadata'); })}>Review removal</Button>{/if}
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
