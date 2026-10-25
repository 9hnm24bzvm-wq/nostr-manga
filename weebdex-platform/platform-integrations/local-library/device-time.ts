// Device ingestion preserves valid date spellings. Compare instants, not their
// original zone/fraction strings, while leaving the displayed value untouched.
export function deviceTimestampInstant(value: string): readonly [number, string] {
  if (typeof value !== 'string' || value.length > 40) throw Error('Invalid saved-device date');
  const milliseconds = Date.parse(value);
  if (!Number.isFinite(milliseconds)) throw Error('Invalid saved-device date');
  const iso = /^(?:\d{4}|[+-]\d{6})-\d{2}-\d{2}[Tt ]\d{2}:\d{2}:\d{2}(?:\.(\d+))?(?:[Zz]|[+-]\d{2}:?\d{2})$/.exec(value);
  const fraction = iso ? iso[1] ?? '' : String((milliseconds % 1000 + 1000) % 1000).padStart(3, '0');
  // Date.parse supplies the UTC whole second, including offsets and negative
  // epochs. Retain all ISO sub-ms precision admitted by the 40-character input.
  return [Math.floor(milliseconds / 1000), fraction.padEnd(40, '0')];
}
