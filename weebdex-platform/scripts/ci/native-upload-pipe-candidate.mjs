// Reviewable bounded transport candidate, not enabled in production builds.
// Replace only the exact frozen production seams; fail closed on source drift.
import { createHash } from 'node:crypto';
export const TRANSPORT_SOURCE_SHA256 = '7093cdee6a1eff833447f0f4c488e52569a74e8df8e083e46f516368c03788de';
const off = 'const uploadRequestRefusalRecording = false;';
const on = 'const uploadRequestRefusalRecording = true;';
const changes = [
  [`    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;`, `    let bodyPipe: Promise<void> | undefined;`],
  [`      refusalStage = 'body-read'; reader = receivedResponse.body?.getReader();
      const chunks: Uint8Array[] = [];
      let size = 0;
      if (reader) for (;;) {
        refusalStage = 'body-read';
        const part = await guarded(() => reader!.read());
        if (part.done) break;
        refusalStage = 'body-budget';
        if (!(part.value instanceof Uint8Array) || part.value.byteLength > limit - size) throw Error('Upload response exceeds budget');
        size += part.value.byteLength;
        chunks.push(new Uint8Array(part.value));
      }
      bodyFinished = true;
      refusalStage = 'body-budget';
      const received = new Uint8Array(size);
      let offset = 0;
      for (const chunk of chunks) { received.set(chunk, offset); offset += chunk.length; }
`, `      // pipeTo owns the response's reader through EOF and releases its lock.
      // Each sink write checks authority and the actual bytes before copying.
      // One lazy backing allocation avoids per-fragment metadata growth, even
      // for empty/tiny chunks. Response copies total at most twice the hard cap.
      let buffer: Uint8Array | undefined, size = 0;
      refusalStage = 'body-read';
      if (receivedResponse.body) await guarded(() => {
        const sink = new WritableStream<Uint8Array>({
          write(chunk) {
            checkpoint();
            refusalStage = 'body-budget';
            if (!(chunk instanceof Uint8Array) || chunk.byteLength > limit - size) throw Error('Upload response exceeds budget');
            if (chunk.byteLength) {
              buffer ??= new Uint8Array(limit);
              buffer.set(chunk, size);
              size += chunk.byteLength;
            }
            checkpoint();
            refusalStage = 'body-read';
          },
          close() { checkpoint(); },
        });
        bodyPipe = receivedResponse.body!.pipeTo(sink, { signal: controller.signal });
        return bodyPipe;
      });
      bodyFinished = true;
      refusalStage = 'body-budget';
      const received = new Uint8Array(size);
      if (buffer) received.set(buffer.subarray(0, size));
      buffer = undefined;
`],
  [`      if (reader) {
        if (!bodyFinished) try { void reader.cancel().catch(() => {}); } catch { /* best effort */ }
        try { reader.releaseLock(); } catch { /* pending read is being cancelled */ }
      } else if (!bodyFinished) discard(response);
`, `      // An active pipe owns cancellation/lock release through the same signal.
      // Responses refused before piping still need explicit body disposal.
      if (!bodyFinished && !bodyPipe) discard(response);
`],
];

export function withBoundedBodyPipe(source) {
  const production = source.replace(on, off);
  if (createHash('sha256').update(production).digest('hex') !== TRANSPORT_SOURCE_SHA256) throw Error('Unreviewed transport source');
  for (const [before, after] of changes) {
    if (source.split(before).length !== 2) throw Error('Transport candidate seam changed');
    source = source.replace(before, after);
  }
  return source;
}
export function restoreBoundedBodyPipe(source) {
  for (const [before, after] of [...changes].reverse()) {
    if (source.split(after).length !== 2) throw Error('Transport candidate seam changed');
    source = source.replace(after, before);
  }
  // Validate the entire reconstructed baseline, not just edited anchors.
  withBoundedBodyPipe(source);
  return source;
}
