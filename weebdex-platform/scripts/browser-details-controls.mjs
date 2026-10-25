export async function expandDetailsPanel(target) {
  await target.waitFor({ state: 'visible' });
  // Native boolean attributes are commonly present as the empty string.
  // Only null means the attribute is absent and the panel is closed.
  if (await target.getAttribute('open') === null) await target.locator(':scope > summary').click();
}
