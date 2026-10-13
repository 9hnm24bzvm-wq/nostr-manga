// A route action loses ownership when a newer navigation starts, even before
// SvelteKit commits its URL. This carries no account or mutation authority.
let epoch = 0;
export const captureNavigationEpoch = () => epoch;
export const sameNavigationEpoch = (captured: number) => captured === epoch;
export const invalidateNavigationEpoch = () => { epoch++; };
