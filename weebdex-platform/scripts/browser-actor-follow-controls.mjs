// Native Group/User entity actions have exact visible labels. Their busy Spinner
// contributes aria-label="Loading" to the accessible name, so exact role names
// cannot identify the same Follow button while a genuine response is withheld.
// Keep native button/header structure, exact text, visibility and strict unique
// locator actions; never select a first match or treat a duplicate as readiness.
const escapeTextPattern = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
export const actorFollowHeader = page => page.locator('section > header:visible').filter({ has: page.locator('h1') });
export const actorFollowControl = (page, label) => actorFollowHeader(page)
  .locator('button[data-slot="button"]:visible')
  .filter({ hasText: new RegExp('^\\s*' + escapeTextPattern(label) + '\\s*$') });
