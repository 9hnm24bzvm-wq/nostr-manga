// Own one action and its corresponding finite page event together. Failed clicks,
// missing/cancelled choosers and prerequisite exceptions release the observer;
// neither action nor observer can leave a detached rejecting promise behind.
export async function actAndWaitForPageEvent(page, event, action, predicate = () => true, timeout = 7000, consume = value => value) {
  if (!['response', 'download', 'filechooser', 'dialog'].includes(event) || typeof action !== 'function' || typeof predicate !== 'function' || typeof consume !== 'function' || !Number.isFinite(timeout) || timeout <= 0) throw Error('Invalid owned page-event contract');
  let finish, settled = false, timer;
  const observed = new Promise((resolve, reject) => {
    finish = (error, value) => { if (settled) return; settled = true; error ? reject(error) : resolve(value); };
  });
  const receive = value => { try { if (predicate(value)) finish(undefined, value); } catch (error) { finish(error); } };
  const closed = () => finish(Error('Page closed before the expected ' + event));
  const crashed = () => finish(Error('Page crashed before the expected ' + event));
  page.on(event, receive); page.on('close', closed); page.on('crash', crashed);
  timer = setTimeout(() => finish(Error('Timed out waiting for the expected ' + event)), timeout);
  try {
    // Install both promise owners before invoking even a synchronously throwing
    // action. Missing events still fail at the original seven-second budget.
    const performed = Promise.resolve().then(action).catch(error => { finish(error); throw error; });
    const [observation, operation] = await Promise.allSettled([observed.then(consume), performed]);
    // Do not advance the journey while an already-issued native click/confirm
    // is still pending. Its own Playwright deadline remains unchanged.
    if (operation.status === 'rejected') {
      if (observation.status === 'rejected' && observation.reason !== operation.reason) throw new AggregateError([observation.reason, operation.reason], 'Expected page event and its action both failed');
      throw operation.reason;
    }
    if (observation.status === 'rejected') throw observation.reason;
    return observation.value;
  } finally {
    clearTimeout(timer);
    page.off(event, receive); page.off('close', closed); page.off('crash', crashed);
    finish(Error('Page-event observer cancelled because its action did not complete'));
  }
}
