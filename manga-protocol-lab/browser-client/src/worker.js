import { LocalLibrary } from "./store.js";
import { buildGraph } from "./codec.js";
self.onmessage = async (
  /** @type {MessageEvent<{ database?: string }>} */ { data },
) => {
  let store;
  try {
    store = await LocalLibrary.open(data.database);
    const snapshot = await store.snapshot();
    const graph = buildGraph(snapshot.records);
    self.postMessage(
      { revision: snapshot.revision, graph },
      { transfer: graph.objects.map((o) => o.data.buffer) },
    );
  } catch (error) {
    self.postMessage({
      error:
        error instanceof Error ? error.message : "Snapshot preparation failed",
    });
  } finally {
    store?.close();
  }
};
