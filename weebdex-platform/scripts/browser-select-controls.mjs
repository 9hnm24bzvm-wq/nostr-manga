// The retained title control is a native Bits UI button. Its always-mounted
// Spinner has aria-label="Loading" even while scale-0, so its accessible name is
// not the visible status alone. Match the exact text in the unique visible title
// scope; do not broaden to unrelated listboxes or silently choose a duplicate.
export const titleLibraryControlName = /^(?:Add to Library|Reading|Plan to Read|On Hold|Completed|Dropped|Re-reading)$/;
const escapeTextPattern = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

export function titleLibraryTrigger(page, status) {
  const text = status === undefined ? titleLibraryControlName.source.slice(1, -1) : escapeTextPattern(status);
  return page.locator('div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible')
    .filter({ hasText: new RegExp('^\\s*' + text + '\\s*$') });
}

export const titleFollowCheckSuffixes = Object.freeze(['trigger-visible-unique', 'popup-open', 'option-selected']);
export async function selectTitleLibraryStatus(page, status, diagnostics) {
  const trigger = titleLibraryTrigger(page);
  // Existing callers keep their original two actions. Only the reviewed finite
  // diagnostic callers add read-only readiness checks and fixed status receipts.
  if (!diagnostics) {
    await trigger.click();
    await page.getByRole('option', { name: status, exact: true }).click();
    return;
  }
  const { check, prefix } = diagnostics;
  if (typeof check !== 'function' || typeof prefix !== 'string' || !/^[a-z0-9]+(?:-[a-z0-9]+)+$/.test(prefix)) throw Error('Invalid fixed title-follow diagnostic contract');
  await check(prefix + '-trigger-visible-unique', () => trigger.waitFor({ state: 'visible' }));
  await check(prefix + '-popup-open', async () => {
    await trigger.click();
    await page.locator('[data-slot="select-content"][role="listbox"]:visible').waitFor({ state: 'visible' });
  });
  await check(prefix + '-option-selected', async () => {
    await page.getByRole('option', { name: status, exact: true }).click();
    await titleLibraryTrigger(page, status).waitFor({ state: 'visible' });
  });
}
// Browser-journey controls for the retained Select-based UI. These are native
// buttons opening listboxes; the retained Combobox wrapper aliases Select.Trigger.
// Importing this module starts no browser, service, listener or fixture.
export const selectListboxTrigger = scope => scope.locator('button[data-slot="select-trigger"][aria-haspopup="listbox"]:visible');
const visibleListbox = page => page.locator('[data-slot="select-content"][role="listbox"]:visible');
const optionWithin = (listbox, label) => listbox.locator('[data-slot="select-item"]').filter({
  hasText: new RegExp('^\\s*' + label.replace(/[.*+?^${}()|[\]\\]/g, '\\$&') + '\\s*$')
});
export async function clickListboxOption(page, label) {
  const listbox = visibleListbox(page);
  await listbox.waitFor({ state: 'visible' });
  await optionWithin(listbox, label).click();
}
export async function setListboxOptions(page, trigger, labels) {
  await trigger.click();
  const listbox = visibleListbox(page);
  // This strict wait requires exactly one active popup, even when clearing all
  // selections. Missing or ambiguous controls cannot become a silent success.
  await listbox.waitFor({ state: 'visible' });
  const filter = listbox.locator('[data-slot="command-input"]');
  if (await filter.count()) await filter.fill('');
  const current = listbox.locator('[data-slot="select-item"][aria-selected="true"]');
  while (await current.count()) await current.first().click();
  for (const label of labels) {
    if (await filter.count()) await filter.fill(label);
    await optionWithin(listbox, label).click();
  }
  await page.keyboard.press('Escape');
}
