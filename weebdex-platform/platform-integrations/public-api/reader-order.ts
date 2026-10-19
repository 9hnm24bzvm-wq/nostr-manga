import { compareDecimal } from './exact-order';

type ChapterPosition = { volume: string | null; chapter: string | null };

// A reader aggregate is newest-to-oldest by volume, then chapter. Publication
// time is edition metadata and must not turn a late old upload into "Next".
// Keep the actual label as the tie breaker without Number/parseFloat rounding.
function descendingLabel(left: string | null, right: string | null): number {
  const numeric = (value: string | null) => value !== null && /^\d+(?:\.\d+)?$/.test(value);
  const a = numeric(left), b = numeric(right);
  if (a !== b) return a ? -1 : 1;
  if (a && b) {
    const order = compareDecimal(right, left);
    if (order) return order;
  }
  if (left === right) return 0;
  if (left === null) return 1;
  if (right === null) return -1;
  return left < right ? 1 : -1;
}

export function compareReaderChapters(left: ChapterPosition, right: ChapterPosition): number {
  return descendingLabel(left.volume, right.volume) || descendingLabel(left.chapter, right.chapter);
}
