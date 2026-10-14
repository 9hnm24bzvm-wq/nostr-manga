// The graph producer owns these bytes for transfer. Keep valid typed-array
// offsets/lengths intact, and list a shared backing buffer only once.
export function graphTransferBuffers(graph: { objects: readonly { data: unknown }[] }): ArrayBuffer[] {
  const buffers = new Set<ArrayBuffer>();
  for (const object of graph.objects) {
    if (!(object.data instanceof Uint8Array) || !object.data.byteLength || !(object.data.buffer instanceof ArrayBuffer)) throw Error('Graph object does not own transferable bytes');
    buffers.add(object.data.buffer);
  }
  return [...buffers];
}
