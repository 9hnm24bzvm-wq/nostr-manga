# Public group membership presentation

The public group GET bridge previously discarded all returned relationships and
requested no included member attributes. The retained group page inferred
“unclaimed” from that empty view and exposed a native report/claim dialog using a
public UUID.

The bridge now requests `includes[]=leader&includes[]=member` and retains actual
included identities, usernames and leader roles. Missing included users are not
fabricated. Duplicate identities and malformed included usernames fail closed.
Protocol publisher attribution remains its actual public-key name, with no fake
native account version or officer role. Membership is display data, not authority.

The public preview has no reviewed group-claim command. Two exact gate-on markup
guards now suppress both its native Claim dialog and its “unclaimed” assertion.
Native mode retains its original behavior. Canonical frontend bytes are unchanged;
the additional guard expressions are explicitly enumerated, not claimed as exact
rendered HTML parity. Other group administration remains unfinished.

`test-evidence/frontend/public-group-membership-20261017/result.json` records:
- One selected finite regression fails against the pinned pre-fix client/bridge.
- 70 finite tests pass, comprising 62 prior cases and 8 new cases.
- The actual composed gate-on strict check passes with 0 errors, 15 warnings and
  all 679 canonical frontend files unchanged.
- Production integration preparation validates retained markup after exactly the
  two new availability guard exceptions, alongside all earlier exceptions.
- 12 independent recipe/provenance tests pass.

These are source/transport/compiler checks, not rendered-browser or live HTTP proof.
