// One writer per mounted reader. A newer admitted page replaces only queued work,
// never an in-flight write. Failed/uncertain outcomes stop automatic work entirely.
export function createReaderProgressQueue<T>(options: {
  assert: (item: T, afterClose: boolean) => void;
  save: (item: T, assertCurrent: () => void) => Promise<unknown>;
  onError: (error: unknown) => void;
}) {
  let queued: T | undefined, running: Promise<void> | undefined, closed = false, stopped = false;
  const flush = (): Promise<void> => {
    if (running) return running;
    if (stopped || queued === undefined) return Promise.resolve();
    running = (async () => {
      while (!stopped && queued !== undefined) {
        const item = queued; queued = undefined;
        const assert = () => options.assert(item, closed);
        try { assert(); await options.save(item, assert); assert(); }
        catch (error) { stopped = true; queued = undefined; options.onError(error); }
      }
    })().finally(() => { running = undefined; });
    return running;
  };
  return {
    enqueue(item: T) { if (closed || stopped) return false; queued = item; return true; },
    flush,
    finish() { closed = true; return flush(); },
    get stopped() { return stopped; },
  };
}
