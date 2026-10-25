// Fixed diagnostic identities only; no response values or exception text.
export const homeFailureDiagnosticChecks = Object.freeze({
  "ui14-home-failures": Object.freeze([
    ...["capability", "open", "held", "begin", "outage", "first-failure", "retry-second", "retry-third", "three-denials", "no-false-empty", "resume", "restored", "recovery-retry", "recovery-read"].map(check => `home-pg-${check}`),
    ...["login", "logout", "request-form", "mail", "link", "cached-session", "redemption", "restricted-auth", "native-denial", "widget-open", "feedback", "single-read", "retry", "repeated-denial"].map(check => `home-private403-${check}`)
  ])
});

// A visible user Retry is a new safe read. The actual HTTP layer does not
// automatically replay a settled503. Wait for this exact additional denied
// response and actionable idle control before admitting another gesture.
export async function retryDeniedHomeRead(section, seen, until, insist) {
  const boundary = seen.rows.length, retry = section.getByRole("button", { name: "Retry updates", exact: true });
  await retry.click();
  await until(() => seen.rows, rows => rows.length === boundary + 1 && rows.at(-1).status === 503 && rows.at(-1).body?.code === "unavailable", "Explicit home Retry did not receive its own single genuine dependency503");
  await seen.settle();
  await section.getByText("Updates unavailable", { exact: true }).waitFor({ state: "visible" });
  await until(() => retry.isEnabled(), Boolean, "Explicit home Retry did not return to actionable idle failure");
  insist(await retry.isEnabled(), "Explicit home Retry did not return to actionable idle failure");
}
