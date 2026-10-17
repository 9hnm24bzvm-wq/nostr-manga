// Display only actual instance-account observations. This is neither a global
// MangaDex population nor a calculated Bayesian score.
export function instanceRatingLabel(input:unknown):string|undefined{
 if(!input||typeof input!=='object')return;const stats=(input as {public_api?:{statistics?:{rating?:unknown}}}).public_api?.statistics?.rating;
 if(!stats||typeof stats!=='object')return;const row=stats as {average?:unknown;distribution?:unknown;nostr?:{population?:unknown;observedCount?:unknown;bayesian?:unknown}};
 if(row.nostr?.population!=='persisted-instance-account-ratings'||row.nostr.bayesian!=='not-calculated-no-global-model'||!Number.isSafeInteger(row.nostr.observedCount)||Number(row.nostr.observedCount)<0||!row.distribution||typeof row.distribution!=='object'||Array.isArray(row.distribution))return;
 const distribution=row.distribution as Record<string,unknown>,keys=Object.keys(distribution);if(keys.length!==10||keys.some(key=>! /^(?:[1-9]|10)$/.test(key)))return;
 let count=0;for(const value of Object.values(distribution)){if(!Number.isSafeInteger(value)||Number(value)<0)return;count+=Number(value);if(!Number.isSafeInteger(count))return;}
 if(count!==row.nostr.observedCount||!count||typeof row.average!=='number'||!Number.isFinite(row.average)||row.average<1||row.average>10)return;
 return row.average.toFixed(1);
}
