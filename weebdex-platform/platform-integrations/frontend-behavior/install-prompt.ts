// Browser extensions are admitted only after their actual runtime shape checks.
export type InstallPromptEvent = Event & { prompt(): unknown };
export function isInstallPromptEvent(event: Event): event is InstallPromptEvent {
  return 'prompt' in event && typeof event.prompt === 'function';
}
export function isStandaloneNavigator(value: Navigator): boolean {
  return 'standalone' in value && value.standalone === true;
}
