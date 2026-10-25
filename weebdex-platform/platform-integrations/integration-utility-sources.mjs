import path from 'node:path';
// Tailwind's automatic scan starts at the copied frontend. Integration components
// live outside that tree, so explicitly include their reviewed utility classes.
export function createIntegrationUtilitySources(frontend, integrationRoot) {
  const stylesheet = path.resolve(frontend, 'src/routes/layout.css');
  return {
    name: 'weebdex-reviewed-integration-utility-sources',
    enforce: 'pre',
    transform(code, id) {
      if (id.split('?')[0] !== stylesheet) return null;
      return { code: code + '\n@source ' + JSON.stringify(path.resolve(integrationRoot).split(path.sep).join('/')) + ';\n', map: null };
    },
  };
}
