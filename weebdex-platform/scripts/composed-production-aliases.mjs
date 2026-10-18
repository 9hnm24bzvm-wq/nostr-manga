// Mirror only the real, reviewed Vite aliases used by the strict projection.
// Regex support is deliberately bounded to this exact installed package root.
export function productionAliasName(find) {
  if (typeof find === 'string') return find;
  if (find instanceof RegExp && find.flags === '' && find.source === '^@lucide\\/svelte$') return '@lucide/svelte';
  throw Error('Unsupported non-string production alias');
}
export function resolveProductionAlias(alias, specifier) {
  const name = productionAliasName(alias.find);
  if (typeof alias.find === 'string') return specifier === name || specifier.startsWith(name + '/') ? alias.replacement + specifier.slice(name.length) : undefined;
  return alias.find.test(specifier) ? specifier.replace(alias.find, alias.replacement) : undefined;
}
export function describeProductionAlias(alias) {
  productionAliasName(alias.find);
  return { ...alias, find: typeof alias.find === 'string' ? alias.find : { regularExpression: alias.find.source, flags: alias.find.flags } };
}
