// Transfer focus with an inert retained settings label before exercising a
// reader shortcut. The real keyboard handler correctly yields to buttons.
export async function focusReaderShortcut(page, insist) {
  await page.getByText("Direction", { exact: true }).click();
  insist(await page.evaluate(() => !document.activeElement?.closest("input, textarea, select, button, a[href], [contenteditable]:not([contenteditable=false]), [role=combobox], [role=listbox], [role=slider], [role=textbox], [role=button]")), "Reader shortcut focus is still owned by an interactive control");
}
