# Native staff workspace

The normal account session is the only credential used by `/staff`. Signing in
with a portable identity, linking a public key, owning a report, or knowing a
public identifier does **not** confer staff authority. Native staff do not gain
original-service admission, publisher, peer, signing or repair authority.

## Navigation and authority

| Workspace | Required current role and explicit grant |
| --- | --- |
| My reports, own history and appeal | The report owner; submitting an appeal also requires `report.create` |
| Moderator queue, claim, progress, priority | Moderator/staff/sysop and `report.update` |
| Assign another reviewer or unclaim someone else's work | Same role, `report.update` and `report.assign` |
| Site-wide decision history | Moderator/staff/sysop and `report.history` |
| Accept a group leadership claim | `report.update` plus `scanlation_group.update` |
| Accept a local title/chapter removal | `report.update` plus `manga.delete` / `chapter.remove` |
| Remove a report from report lists | Moderator/staff/sysop and `report.delete`; the subject is unchanged |
| Reopen a report | Staff/sysop and `report.correct`; previous effects are unchanged |
| Correct local hiding | Staff/sysop, `report.correct`, and `manga.restore` / `chapter.restore` |
| Correct claimed group leadership | Staff/sysop, `report.correct`, and `group.leadership.revoke` |
| Edit a foreign public profile | Staff/sysop and `user.update` |
| Operations overview | Staff/sysop and `operations.read` |

These are capabilities, not a recommendation to grant every staff member all
of them. The administrator workflow owns reviewed fixed bundles. Ordinary
registration grants are not expanded by these changes. Recovery sessions and
API clients cannot exercise these browser-workspace operations.

Private reads resolve the exact captured native account and normal cookie in
the same transaction as their read. Writes additionally validate the browser
origin and hold the current actor/session authority through commit. Target and
report revisions protect reviews from concurrent changes. Role revocation,
session revocation, account restrictions and target changes fail closed.

## Review, cancellation and unknown outcomes

1. Open a report from **Moderator queue**, not My reports.
2. Read the exact subject, evidence, claimant (if applicable), and consequence.
3. Provide an explanation. Cancel closes the local review and sends no command.
4. Confirm once. A committed command has an actor-bound durable receipt and
   append-only report history.
5. If the response is lost, choose **Check action receipt**. The client persists
   a receipt reference and does not repeat the command automatically. A missing
   receipt is not evidence that an in-flight operation never committed.

Claiming, priority and in-progress state use the same report revision as the
decision. Two reviewers cannot silently overwrite one another's claim or
review. Only an eligible current reviewer can receive assignment.

## Corrections and appeals

Report owners can submit a reasoned appeal and see the appropriate decision
timeline, but cannot see another person's report, reviewer identity, assignment
history or recorded membership prestate. Global history is a separate grant.

Reopening only changes report workflow; it never silently reverses an old
effect. **Review local correction** is a separate, explicitly authorized
operation. It requires the exact recorded post-decision target revision,
unchanged current membership/visibility, a complete before/after record, no
unresolved publication, and currently available original source. It appends a
new correction and preserves the original decision. Group restoration preserves
the prior membership and officer flags exactly; it does not guess them.

Legacy decisions without sufficient recorded prestate are not automatically
reversible. A newer target change, lost grant, unavailable source or pending
publication produces an explicit refusal. Obtain a separately authorized review
of current state instead of editing the database or replaying an old command.
Local restoration cannot restore withdrawn originals, peer copies or deleted
storage; those require the original operator's supported independent procedures.

## Read-only operations and operator handoff

`/staff/operations` shows bounded state and reference lists, configuration
presence (never secret contents), and the recovery steps. `prepared` and
`starting` mean that the original outcome needs inspection; they do not mean
"safe to retry". `source_accepted` must be reconciled as the original command,
not recreated. The page has no replay, repair or source-authority action.

Before handoff: verify the deployed configuration/schema with the supported
read-only `weebdex-platform verify` command; inspect pending receipts together;
test restoration from a readable backup in isolation; use the reviewed native
People and access workflow for a second administrator; and update the instance
operator/contact/policies. Do not send or paste keys through the native UI.

## Instance identity and policies

The `instance` object in `PLATFORM_CONFIG` is public configuration:

```json
{
  "instance": {
    "operatorName": "Example community operator",
    "contactUrl": "mailto:help@example.invalid",
    "terms": "The reviewed terms for this instance.",
    "privacy": "The reviewed privacy policy for this instance.",
    "guidelines": "The community rules for this instance.",
    "termsUrl": null,
    "privacyUrl": null,
    "guidelinesUrl": null
  }
}
```

The example is not legal policy. Supply reviewed text or an HTTP(S) policy URL
for each policy, plus the actual responsible operator and contact. Contact may
also use `mailto:`. URLs containing credentials, fragments, unsupported schemes
or control characters are rejected. Policy text is displayed as text, not HTML.
The visitor and upload-consent links remain `/terms`, `/privacy`, `/guidelines`
and `/about`, each identifying this deployment rather than an upstream contact.
Unconfigured deployments say so explicitly; the operations checklist flags the
incomplete setup. Review this configuration before opening registration/uploads.

## Validation scope

The implementation has separate real-router/disposable-PostgreSQL HTTP tests
and mounted production-component tests. Consult their terminal receipts before
treating a source change as passed. Synthetic grants are test prerequisites,
not production role policy; no real account or operator setting is changed by
the tests.
