// Safe-read continuation. Each visibility entry or explicit gesture admits at
// most four extra sequential pages; compact huge feeds retain a reachable button.
type Feed = { page: number; limit: number; total: number; data?: unknown[] };
type Options = {
  target: HTMLElement; isAlive: () => boolean; getFeed: () => Feed | undefined;
  loadPage: (page: number) => Promise<boolean>; settle: () => Promise<void>;
  captureContext: () => () => boolean; pause: (paused: boolean, failed: boolean) => void;
};
export function createFeedContinuation(options: Options) {
  let disposed = false, running = false, paused = false, inside = false;
  const more = () => { const feed = options.getFeed(); return !!feed && !!feed.data?.length && feed.limit * feed.page < feed.total; };
  const near = () => { const rect = options.target.getBoundingClientRect(), margin = window.innerHeight; return rect.bottom >= -margin && rect.top <= window.innerHeight + margin; };
  const observer = new IntersectionObserver(([entry]) => {
    if (disposed || !entry || entry.target !== options.target) return;
    const entered = entry.isIntersecting && !inside; inside = entry.isIntersecting;
    if (entered && !paused) void pump(false);
  }, { rootMargin: window.innerHeight + 'px' });
  async function pump(manual: boolean) {
    if (disposed || running || !options.isAlive() || !more() || (!manual && !near())) return;
    const owned = options.captureContext();
    if (!owned()) return;
    running = true;
    let failed = false;
    try {
      for (let count = 0; count < 4; count++) {
        if (disposed || !options.isAlive() || !owned() || !more() || (count > 0 && !near())) break;
        const previous = options.getFeed()!;
        const success = await options.loadPage(previous.page + 1);
        if (disposed || !options.isAlive() || !owned()) return;
        await options.settle();
        if (disposed || !options.isAlive() || !owned()) return;
        const next = options.getFeed();
        if (!success || !next || next.page !== previous.page + 1) { failed = true; break; }
        if (!next.data?.length || next.limit * next.page >= next.total) break;
      }
    } finally {
      running = false;
      if (!disposed && options.isAlive()) {
        // A still-compact footer always gets an explicit continuation, including
        // context cancellation and truthful error/no-progress stopping.
        paused = more() && (failed || near());
        options.pause(paused, failed);
      }
    }
  }
  observer.observe(options.target);
  return {
    resume() { void pump(true); },
    dispose() { disposed = true; observer.disconnect(); }
  };
}
