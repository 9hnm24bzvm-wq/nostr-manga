// Native labeled contributor fields only; no global Search-text fallback.
export function contributorSearchTrigger(page, family) {
  if (!['authors', 'artists'].includes(family)) throw Error('Unknown contributor field');
  return page.locator(`label[for="${family}"]`).locator('..').getByText('Search...', { exact: true });
}
