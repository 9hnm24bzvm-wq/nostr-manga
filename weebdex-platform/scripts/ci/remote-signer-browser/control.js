// Separate same-origin synthetic-operator tab; no product page script injection.
const output = document.querySelector('output');
async function refresh() { const response = await fetch('/fixture/observations', { credentials: 'omit', cache: 'no-store' }); if (!response.ok) throw Error(); output.textContent = JSON.stringify(await response.json(), null, 2); }
for (const button of document.querySelectorAll('button[data-action]')) button.addEventListener('click', async () => {
  button.disabled = true;
  try { const response = await fetch('/fixture/control', { method: 'POST', credentials: 'omit', redirect: 'error', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ action: button.dataset.action }) }); if (!response.ok) throw Error(); await refresh(); }
  catch { output.textContent = 'Synthetic fixture control refused.'; }
  finally { button.disabled = false; }
});
document.querySelector('#refresh').addEventListener('click', () => { void refresh().catch(() => { output.textContent = 'Synthetic observation unavailable.'; }); });
void refresh().catch(() => { output.textContent = 'Synthetic observation unavailable.'; });
