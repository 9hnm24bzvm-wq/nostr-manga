// Narrow round-two reader repairs. Supplied source, HTML and styles are unchanged.
// Independently review and mount this module before claiming source closure.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

const single = 'src/lib/components/reader/reader-pages-single.svelte';
const double = 'src/lib/components/reader/reader-pages-double.svelte';
const keyboard = 'src/lib/components/reader/reader-keyboard-listener.svelte';
const sourceHashes = {
  [single]: '0d93d109e1fdb772bf4b1b61e4c9246cbda19bfcb4c154a004219c139cda65aa',
  [double]: '8442a0b86a9b99d2e47b7fa2b9cf9c1e7ae0cc673c06745929e7c6762eaaa9bf',
  [keyboard]: '4d10f5e6efed6fe727c984f91d9711e20a2cc17e8200f0d5dd0380f90ecc26e4'
};
const patches = [];
function repair(path, label, before, after) {
  patches.push({ path, label, sha256: sourceHashes[path], count: 1,
    before: before.replace(/\n/g, '\r\n'), after: after.replace(/\n/g, '\r\n') });
}

// The retained visibility expression compares currentPage to page. For a spread,
// page must be its selected member when one is selected, or its first member when
// this spread is not selected. Observer transitions still select the first member
// of a newly entered spread, while direct page-two selection keeps that spread on.
repair(double, 'selected second page keeps its double spread visible',
  '\tconst page = $derived(spread[0]);', '');
repair(double, 'derive spread visibility only after reader context is initialized',
  '\tconst reader = useReader();',
  '\tconst reader = useReader();\n\tconst page = $derived(spread.find(candidate => candidate === reader.currentPage) ?? spread[0]);');

for (const path of [single, double]) {
  repair(path, 'scroll observer cannot reattach after disposal',
    '\tlet isMounted = $state(false);',
    '\tlet isMounted = $state(false);\n\tlet disposed = false;');
  repair(path, 'abandon a mount tick that completed after disposal',
    '\t\tawait tick();', '\t\tawait tick();\n\t\tif (disposed) return;');
  repair(path, 'observe resize after switching from paged to scroll layout',
    '\t\t\twindow.addEventListener("resize", attachObserver);', '');
  repair(path, 'attach one lifetime-owned resize listener in either initial layout',
    '\t\tisMounted = true;',
    '\t\twindow.addEventListener("resize", attachObserver);\n\t\tisMounted = true;');
  repair(path, 'release and fence scroll observers on disposal',
    '\tonDestroy(() => {\n\t\tobserver?.disconnect();',
    '\tonDestroy(() => {\n\t\tdisposed = true;\n\t\tobserver?.disconnect();');
  repair(path, 'ignore queued resize and preference observer work after disposal',
    '\t\tif (!ref || $readerPrefs.layout !== Constants.Reader.Layout.Scroll) {',
    '\t\tif (disposed || !ref || $readerPrefs.layout !== Constants.Reader.Layout.Scroll) {');
  repair(path, 'ignore a queued intersection notification after disposal',
    '\t\t\t\tif (isMounted && entry.isIntersecting && reader.scroll.scrolling && page !== reader.currentPage) {',
    '\t\t\t\tif (!disposed && isMounted && entry.isIntersecting && reader.scroll.scrolling && page !== reader.currentPage) {');
  repair(path, 'long strip intersection threshold fits inside the viewport',
    '\t\t\t\tthreshold: ref.getBoundingClientRect().height > window.innerHeight ? 0.4 : 0.65',
    '\t\t\t\tthreshold: ref.getBoundingClientRect().height > window.innerHeight\n\t\t\t\t\t? Math.min(0.4, (window.innerHeight / Math.max(1, ref.getBoundingClientRect().height)) * 0.4)\n\t\t\t\t\t: 0.65');
}

// Width/layout changes can resize a long strip without a window resize. Observe
// actual geometry and rebuild at most once per animation frame only when changed.
for (const path of [single, double]) {
  repair(path, 'reader image geometry observer owns bounded threshold refresh',
    '\tlet observer: IntersectionObserver | undefined;',
    '\tlet observer: IntersectionObserver | undefined;\n\tlet geometryObserver: ResizeObserver | undefined, geometryFrame = 0, geometryWidth = -1, geometryHeight = -1;');
  repair(path, 'actual reader page resize recomputes its reachable threshold',
    '\t\twindow.addEventListener("resize", attachObserver);\n\t\tisMounted = true;',
    '\t\twindow.addEventListener("resize", attachObserver);\n\t\tif (ref && typeof ResizeObserver !== "undefined") {\n\t\t\tgeometryObserver = new ResizeObserver(([entry]) => {\n\t\t\t\tif (disposed || !entry) return;\n\t\t\t\tconst { width, height } = entry.contentRect;\n\t\t\t\tif (Math.abs(width - geometryWidth) < 0.5 && Math.abs(height - geometryHeight) < 0.5) return;\n\t\t\t\tgeometryWidth = width; geometryHeight = height;\n\t\t\t\tif (!geometryFrame) geometryFrame = window.requestAnimationFrame(() => { geometryFrame = 0; if (!disposed) void attachObserver(); });\n\t\t\t}); geometryObserver.observe(ref);\n\t\t}\n\t\tisMounted = true;');
  repair(path, 'release reader geometry observer and queued frame on disposal',
    '\t\tdisposed = true;\n\t\tobserver?.disconnect();',
    '\t\tdisposed = true;\n\t\tgeometryObserver?.disconnect(); window.cancelAnimationFrame(geometryFrame); geometryFrame = 0;\n\t\tobserver?.disconnect();');
}

repair(keyboard, 'keyboard animation frame has component lifetime ownership',
  '\timport { Constants } from "$lib";',
  '\timport { Constants } from "$lib";\n\timport { onDestroy, onMount } from "svelte";');
repair(keyboard, 'stop keyboard scrolling on blur and component destruction',
  '\tconst onKeyUp = () => {\n\t\twindow.cancelAnimationFrame(raf);\n\t\traf = 0;\n\t};',
  '\tconst onKeyUp = () => {\n\t\twindow.cancelAnimationFrame(raf);\n\t\traf = 0;\n\t};\n\tonMount(() => {\n\t\twindow.addEventListener("blur", onKeyUp);\n\t\treturn () => window.removeEventListener("blur", onKeyUp);\n\t});\n\tonDestroy(onKeyUp);');

// Reader shortcuts yield to actual controls, handled events and editable ancestors.
repair(keyboard, 'round6 reader recognizes control-owned keyboard paths',
  '\tconst onKeyDown = (ev: KeyboardEvent) => {',
  '\tconst ownsKeyboard = (target: EventTarget | null) => target instanceof Element && (!!target.closest("input, textarea, select, button, a[href], [contenteditable]:not([contenteditable=false]), [role=combobox], [role=listbox], [role=slider], [role=textbox], [role=button]") || target instanceof HTMLElement && target.isContentEditable);\n\tconst controlOwnsEvent = (ev: Event) => ev.composedPath().some(ownsKeyboard);\n\tconst onKeyDown = (ev: KeyboardEvent) => {');
repair(keyboard, 'round6 reader does not steal native or already handled shortcuts',
  '\t\t\tev.target instanceof HTMLInputElement ||\n\t\t\tev.target instanceof HTMLTextAreaElement ||',
  '\t\t\tev.defaultPrevented ||\n\t\t\tcontrolOwnsEvent(ev) ||');
repair(keyboard, 'reader yields system modifiers and active text composition',
  '\t\t\tev.ctrlKey ||',
  '\t\t\tev.ctrlKey ||\n\t\t\tev.altKey ||\n\t\t\tev.metaKey ||\n\t\t\tev.isComposing ||');
repair(keyboard, 'round6 focused control terminates a held reader animation',
  '\t\t\tgetIsOpen()\n\t\t) {\n\t\t\treturn;',
  '\t\t\tgetIsOpen()\n\t\t) {\n\t\t\tonKeyUp();\n\t\t\treturn;');
repair(keyboard, 'round6 focus transition stops owned reader scroll frame',
  '\t\twindow.addEventListener("blur", onKeyUp);\n\t\treturn () => window.removeEventListener("blur", onKeyUp);',
  '\t\tconst onFocus = (ev: FocusEvent) => { if (controlOwnsEvent(ev)) onKeyUp(); };\n\t\twindow.addEventListener("blur", onKeyUp); window.addEventListener("focusin", onFocus);\n\t\treturn () => { window.removeEventListener("blur", onKeyUp); window.removeEventListener("focusin", onFocus); };');

const hash = value => createHash('sha256').update(value).digest('hex');
export function createReaderOverlays(frontendRoot) {
  const byPath = new Map();
  for (const patch of patches) {
    const absolute = resolve(frontendRoot, patch.path);
    if (hash(readFileSync(absolute)) !== patch.sha256) throw Error(`Reader overlay source changed: ${patch.path}`);
    const entries = byPath.get(absolute) ?? [];
    entries.push(patch); byPath.set(absolute, entries);
  }
  return { name: 'weebdex-reviewed-reader-overlays', enforce: 'pre', transform: { order: 'pre', handler(code, id) {
    if (id.includes('?')) return null;
    const entries = byPath.get(resolve(id)); if (!entries) return null;
    if (hash(code) !== entries[0].sha256) throw Error(`Reader overlay input differs from supplied source: ${id}`);
    let result = code;
    for (const patch of entries) {
      if (result.split(patch.before).length - 1 !== patch.count) throw Error(`Reader overlay anchor changed: ${patch.label}`);
      result = result.split(patch.before).join(patch.after);
    }
    return { code: result, map: null };
  } } };
}
export const readerOverlayManifest = patches.map(({ before, after, ...metadata }) => metadata);
