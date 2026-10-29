/** Local form buffers only: never persists content or clears command fences. */
export const UNSAVED_EDITOR_MESSAGE = 'You have unsaved changes. Leave and discard them?';
export function allowEditorDiscard(dirty: boolean, confirm: (message: string) => boolean): boolean {
 return !dirty || confirm(UNSAVED_EDITOR_MESSAGE);
}
export function guardEditorNavigation(event: {willUnload?: boolean; cancel(): void}, dirty: boolean, confirm: (message: string) => boolean): void {
 if (!dirty) return;
 // SvelteKit displays the browser-native confirmation for document unload.
 if (event.willUnload || !allowEditorDiscard(dirty, confirm)) event.cancel();
}
