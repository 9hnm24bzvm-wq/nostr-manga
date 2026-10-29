// Exact current feature registry with an additive source-only predecessor check.
// Historical helper/pins remain unchanged and do not qualify current execution.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import {createHash} from 'node:crypto';
import {restoreReaderUxFixpassPredecessor} from './reviewed-six-persona-source-successor.mjs';
import {assertDiscussionFeatures as assertPublishedFeatures} from './native-discussion-overlay-fixture.mjs';
export {assertDiscussionManifest,discussionFeatureRows,membershipFeatureRow} from './native-discussion-overlay-fixture.mjs';
export const profileActionsFeatureRow={"path":"src/routes/(app)/user/[id]/User.svelte","sha256":"f8232429c11dcc2dc08fb34f1c3fc31d262cc547b4becf485b884133ed6db0bd","count":1,"before":"<div class=\"flex shrink-0 flex-wrap items-center gap-1\">","after":"<div class=\"flex shrink-0 flex-wrap items-center gap-1\">\r\n\t\t\t\t\t<SessionProfileActions targetId={$user.id} requestedId={profileActionRequestedId($user, page.params.id??\"\")} nativeEligible={$user.public_profile_readonly === true && $user.public_account_follow_available === true}/>\r\n\t\t\t\t\t<details class=\"basis-full text-sm\"><summary class=\"cursor-pointer\">Advanced identity actions</summary><div class=\"mt-2\"><NativeProfileActions targetId={$user.id} requestedId={profileActionRequestedId($user, page.params.id??\"\")} nativeEligible={$user.public_profile_readonly === true && $user.public_account_follow_available === true}/></div></details>","label":"Explicit signing-account Settings and Block use the existing native profile action row","approved_public_feature_exception":"public_uuid_bound_account_profile_actions"};
export function assertDiscussionFeatures(rows){
 assert.equal(rows.length,23);
 assert.equal(createHash('sha256').update(JSON.stringify(rows)).digest('hex'),'e926b5a29422ce977c022a1a0269bea207903ca1a531eeacda296dc3f5b5087d');
 const relative='weebdex-platform/platform-integrations/public-api/anchors.json';
 const current=fs.readFileSync(new URL('../../platform-integrations/public-api/anchors.json',import.meta.url),'utf8');
 const published=JSON.parse(restoreReaderUxFixpassPredecessor(relative,current));
 assertPublishedFeatures(published.filter(row=>row.approved_public_feature_exception));
}
