import { LocalLibrary } from "./store.js";
import { buildGraph } from "./codec.js";
self.onmessage = async ({ data }) => {
  let store;
  try {
    store = await LocalLibrary.open(data.database);
    const snapshot = await store.snapshot();
    const graph = buildGraph(snapshot.records);
    self.postMessage(
      { revision: snapshot.revision, graph },
      graph.objects.map((o) => o.data.buffer),
    );
  } catch (error) {
    self.postMessage({ error: error.message });
  } finally {
    store?.close();
  }
};
