import { deviceTimestampInstant as followedAtInstant } from './device-time';
export { deviceTimestampInstant as followedAtInstant } from './device-time';
import { decimalKey } from '../public-api/exact-order';
export type TitleSortRow = { id: string; _sort_value?: unknown; _sort_text?: string; _sort_decimal?: boolean };
// IDB arrays compare recursively. Never reverse the cursor: NULL stays last
// and the UUID tie remains ascending in BOTH requested primary directions.
function reverseText(value: string): string {
  let result = '';
  for (let i = 0; i < value.length; i++) result += String.fromCharCode(65534 - value.charCodeAt(i));
  return result + '\uffff'; // A longer original sorts before its own prefix.
}
function decimalOrder(input: unknown, direction: number): IDBValidKey[] {
  const raw = decimalKey(input), negative = raw.startsWith('-');
  const [whole, fraction = ''] = (negative ? raw.slice(1) : raw).split('.');
  const integer = whole.replace(/^0+/, '');
  const digits = (integer + fraction).replace(/^0+/, '').replace(/0+$/, '');
  if (!digits) return [1]; // Every spelling of signed zero is equal.
  const exponent = integer.length || -fraction.search(/[1-9]/);
  const descending = (negative ? -1 : 1) * direction < 0;
  return descending ? [0, -exponent, reverseText(digits)] : [2, exponent, digits];
}
export function titleOrderKey(row: TitleSortRow, direction: number): IDBValidKey[] {
  if (direction !== 1 && direction !== -1 || typeof row.id !== 'string' || !row.id.length || row.id.length > 64) throw Error('Invalid title index identity or direction');
  const value = row._sort_value, text = row._sort_text ?? '';
  if (typeof text !== 'string' || text.length > 8000 || /[\uffff]/.test(text)) throw Error('Invalid title secondary key');
  let primary: IDBValidKey = '';
  if (value != null) {
    if (row._sort_decimal) primary = decimalOrder(value, direction);
    else if (typeof value === 'number' && Number.isFinite(value)) primary = direction * value;
    else if (typeof value === 'string' && value.length <= 8000 && !/[\uffff]/.test(value)) primary = direction === 1 ? value : reverseText(value);
    else throw Error('Invalid title comparison key');
  }
  return [value == null ? 1 : 0, primary, direction === 1 ? text : reverseText(text), row.id];
}
export function titleKeyBytes(key: IDBValidKey[]): number {
  // Fixed shape, bounded strings; this is a conservative admission charge,
  // not a claim about browser/IDB implementation overhead or disk allocation.
  return 256 + JSON.stringify(key).length * 2;
}

export function followedAtOrderKey(id: string, value: string, direction: number): IDBValidKey[] {
  const [seconds, fraction] = followedAtInstant(value);
  const key = titleOrderKey({ id, _sort_value: seconds }, direction);
  key[2] = direction === 1 ? fraction : reverseText(fraction);
  return key;
}
