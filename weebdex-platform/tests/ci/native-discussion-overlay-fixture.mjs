// Comparison-only exact feature delta. Never imported by product/build code.
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
const hash=value=>createHash('sha256').update(JSON.stringify(value)).digest('hex');
export const discussionFeatureRows=[
  {
    "path": "src/routes/(app)/group/[id]/comments/+page.svelte",
    "sha256": "06b1eb88a80c5abd859e28c36b5233672b7f8ccc74c90b7b62785674d3f048b3",
    "count": 1,
    "before": "<Comments.Comments />",
    "after": "<Comments.Comments target={{kind:\"group\",targetId:$group.id,threadId:$group.relationships?.thread?.id??\"\"}} />",
    "approved_public_feature_exception": "public_uuid_group_signed_discussion_target",
    "label": "Group comments receive the actual typed public resource and stored native thread"
  },
  {
    "path": "src/routes/(app)/forums/threads/[id]/+page.svelte",
    "sha256": "d94a8c78e2436ac00da00241d85f5c362ffc8f1c285be8d958e127bcec99a6b7",
    "count": 1,
    "before": "<Comments.Comments />",
    "after": "<Comments.Comments resolveThread={true} />",
    "approved_public_feature_exception": "public_uuid_forum_signed_discussion_target",
    "label": "Forum signed target comes only from the bounded persisted thread-context resolver"
  }
];
export const membershipFeatureRow={
  "path": "src/routes/(app)/user/[id]/+page.svelte",
  "sha256": "eaa3d2bbcce25af67937528ee161931d406a76067692f62c6c455208315ef4b5",
  "count": 1,
  "before": "{#if $user.relationships?.groups?.length}",
  "after": "{#if $user.public_profile_readonly}\n\t{#if $user.public_account_follow_available}<PublicUserMemberships userId={$user.id} />{/if}\n{:else if $user.relationships?.groups?.length}",
  "label": "Public native-user groups use visible bounded pages without expanded members",
  "approved_public_feature_exception": "public_uuid_native_user_memberships"
};
export const profileActionsFeatureRow={
  "path": "src/routes/(app)/user/[id]/User.svelte",
  "sha256": "f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd",
  "count": 1,
  "before": "<div class=\"flex shrink-0 flex-wrap items-center gap-1\">",
  "after": "<div class=\"flex shrink-0 flex-wrap items-center gap-1\">\r\n\t\t\t\t\t<NativeProfileActions targetId={$user.id} requestedId={page.params.id??\"\"} nativeEligible={$user.public_profile_readonly === true && $user.public_account_follow_available === true}/>",
  "label": "Explicit signing-account Settings and Block use the existing native profile action row",
  "approved_public_feature_exception": "public_uuid_bound_account_profile_actions"
};
export function assertDiscussionFeatures(rows){
 assert.deepEqual(rows.filter(row=>row.approved_public_feature_exception===profileActionsFeatureRow.approved_public_feature_exception),[profileActionsFeatureRow]);
 rows=rows.filter(row=>row.approved_public_feature_exception!==profileActionsFeatureRow.approved_public_feature_exception);
 assert.deepEqual(rows.filter(row=>row.approved_public_feature_exception===membershipFeatureRow.approved_public_feature_exception),[membershipFeatureRow]);
 rows=rows.filter(row=>row.approved_public_feature_exception!==membershipFeatureRow.approved_public_feature_exception);
 const ids=discussionFeatureRows.map(row=>row.approved_public_feature_exception);
 assert.deepEqual(rows.filter(row=>ids.includes(row.approved_public_feature_exception)),discussionFeatureRows);
 const prior=rows.filter(row=>!ids.includes(row.approved_public_feature_exception));
 assert.equal(prior.length,18);assert.equal(rows.length,20);
 assert.equal(hash(prior),'07f719ddbc1c6f1b35e77637e1b465c124fd3d8bc6a2ff713a899710cc24d8ab');
}
export const catalogueMembershipManifestRows=[
  {
    "path": "src/lib/components/ChapterListFilter.svelte",
    "sha256": "2f38ac6741bb02cff665ec0885929ce721082eae1afbc05ce8a3f1bd6f3db6fa",
    "public_api_base": "/api/v1",
    "activation": "explicit staged gate; default off",
    "reconstructed_unverified": true
  },
  {
    "path": "src/routes/(app)/user/[id]/+page.svelte",
    "sha256": "eaa3d2bbcce25af67937528ee161931d406a76067692f62c6c455208315ef4b5",
    "public_api_base": "/api/v1",
    "activation": "explicit staged gate; default off",
    "reconstructed_unverified": true
  }
];
export function assertDiscussionManifest(rows){
 assert.equal(rows.length,63);
 const latest='src/lib/components/LatestUpdatesGridEntry.svelte';
 assert.deepEqual(rows.filter(row=>row.path===latest),[{path:latest,sha256:'89b8bbd1fc61d12159044baa534f6d04aea63257a109615bd0153a2cf9383688',public_api_base:'/api/v1',activation:'explicit staged gate; default off',reconstructed_unverified:true}]);
 rows=rows.filter(row=>row.path!==latest);
 assert.equal(rows.length,62);
 const addedPaths=catalogueMembershipManifestRows.map(row=>row.path);
 assert.deepEqual(rows.filter(row=>addedPaths.includes(row.path)),catalogueMembershipManifestRows);
 rows=rows.filter(row=>!addedPaths.includes(row.path));
 const added='src/routes/(app)/forums/threads/[id]/+page.svelte';
 assert.deepEqual(rows.filter(row=>row.path===added),[{path:added,sha256:'d94a8c78e2436ac00da00241d85f5c362ffc8f1c285be8d958e127bcec99a6b7',public_api_base:'/api/v1',activation:'explicit staged gate; default off',reconstructed_unverified:true}]);
 const prior=rows.filter(row=>row.path!==added).map(row=>row.path).sort();
 assert.equal(rows.length,60);assert.equal(prior.length,59);
 assert.equal(hash(prior),'e1c59722ecc5324d4fd0798f4725487d3adf891f03bf7782573e064eba08ac44');
}
