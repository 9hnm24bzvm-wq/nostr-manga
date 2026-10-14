// The retained title control uses Bits UI Select.Trigger: a native button with
// aria-haspopup=listbox. Select.Input has role=combobox; Trigger does not.
export const titleLibraryControlName = /^(?:Add to Library|Reading|Plan to Read|On Hold|Completed|Dropped|Re-reading)$/;

export function titleLibraryTrigger(page, status) {
  return page.getByRole('button', status === undefined ? { name: titleLibraryControlName } : { name: status, exact: true })
    .and(page.locator('div.min-w-45 button[data-slot="select-trigger"][aria-haspopup="listbox"]'));
}

export async function selectTitleLibraryStatus(page, status) {
  await titleLibraryTrigger(page).click();
  await page.getByRole('option', { name: status, exact: true }).click();
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
