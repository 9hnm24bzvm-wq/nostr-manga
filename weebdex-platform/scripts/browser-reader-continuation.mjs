// Exact native fixture controls: last-page completion must cause Continue
// Reading to choose the next unread English chapter, without completing it.
import assert from 'node:assert/strict';

export async function exerciseReaderContinuation({page, origin, manga, chapter, nextChapter, api, open, readDevice, until}) {
  for (const value of [manga, chapter, nextChapter]) assert.match(value, /^[A-Za-z0-9]{10}$/);
  assert.notEqual(chapter, nextChapter, 'Continuation needs two different fixture chapters');
  assert.equal(page.url(), origin + `/chapter/${chapter}/2`, 'Exact page-two reader must own continuation');
  const previous = await api(`/chapter/${chapter}`), next = await api(`/chapter/${nextChapter}`);
  for (const [id, response] of [[chapter, previous], [nextChapter, next]]) {
    assert.ok(response.status === 200 && response.body?.id === id && response.body.relationships?.manga?.id === manga && response.body.language === 'en', 'Recorded native continuation chapters must be actual English rows on the same title');
  }
  const lastPage = previous.body.data?.length;
  assert.ok(Number.isInteger(lastPage) && lastPage >= 3 && lastPage <= 24, 'Continuation fixture needs bounded actual page bytes');
  for (let selected = 3; selected <= lastPage; selected++) {
    await page.keyboard.press('ArrowRight');
    await page.waitForURL(url => url.origin === origin && url.pathname === `/chapter/${chapter}/${selected}`);
  }
  const images = () => page.locator('img[src^="blob:"]').evaluateAll(rows => rows.filter(image => image.complete && image.naturalWidth > 0 && image.getClientRects().length).length);
  await until(images, count => count > 0, 'Last page must decode actual visible image bytes');
  const finished = await until(readDevice, snapshot => snapshot?.records.some(row => row.family === 'chapters' && row.id === chapter && row.mangaId === manga && row.page === lastPage && row.completed === true && typeof row.readAt === 'string'), 'Last page must persist exact device completion');
  const complete = finished.records.find(row => row.family === 'chapters' && row.id === chapter);
  await open(`/title/${manga}?tlang=en`);
  const control = page.getByRole('button', {name: 'Continue Reading', exact: true});
  await control.waitFor({state: 'visible'});
  assert.equal(await control.count(), 1, 'Title must expose exactly one actual Continue Reading control');
  await control.click();
  await page.waitForURL(url => url.origin === origin && url.pathname === `/chapter/${nextChapter}/1`);
  await until(images, count => count > 0, 'Continued chapter must decode actual visible image bytes');
  const resumed = await until(readDevice, snapshot => snapshot?.records.some(row => row.family === 'chapters' && row.id === nextChapter && row.mangaId === manga && row.page === 1 && row.completed === false && row.readAt === undefined), 'Continue Reading must persist page one without inventing completion');
  const retained = resumed.records.find(row => row.family === 'chapters' && row.id === chapter);
  assert.ok(retained?.page === lastPage && retained.completed === true && retained.readAt === complete.readAt, 'Continuing must preserve the completed chapter and its real completion time');
  return {actual_last_page: lastPage, completed_locally: complete.completed, completion_time_retained: true, continue_control_used: true, exact_next_unread_chapter: true, next_page: 1, next_completed: false};
}
