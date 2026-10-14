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
