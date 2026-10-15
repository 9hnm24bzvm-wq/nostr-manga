// Navigation only: this does not create an account, identity or publication.
// URLSearchParams already decodes query values; do not decode them a second time.
export function safeRegistrationReturnTo(value: string | null | undefined, origin = window.location.origin): string {
  if (!value || /[\u0000-\u001f\u007f\\]/.test(value)) return '/';
  try {
    const destination = new URL(value, origin);
    if (destination.origin !== origin || destination.username || destination.password || /^\/auth(?:\/|$)/.test(destination.pathname)) return '/';
    return destination.pathname + destination.search + destination.hash;
  } catch { return '/'; }
}

// The retained app customizes URLSearchParams.toString() to include '?'.
// Encode this one known parameter directly, independently of that global patch.
export function registrationContinuationHref(route: '/auth/verification' | '/auth/login', returnTo: string, origin = window.location.origin): string {
  return route + '?redirect=' + encodeURIComponent(safeRegistrationReturnTo(returnTo, origin));
}
