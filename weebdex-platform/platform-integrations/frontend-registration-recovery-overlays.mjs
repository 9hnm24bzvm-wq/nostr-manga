// Exact additive patches against the preserved registration form and existing ownership repairs.
export const registrationRecoveryPatches = [
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration recovery imports presentation-only nonsecret handoff",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "import { safeRegistrationReturnTo } from \"$platform/account-onboarding\";",
    "after": "import { safeRegistrationReturnTo } from \"$platform/account-onboarding\";\r\n\timport { registrationDetails, clearRegistrationPassword, retainRegistrationDetails, registrationError, rememberRegistrationDetails, takeRegistrationDetails } from \"$platform/registration-recovery\";"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration expiry state and explicit renewal stay fenced to current form",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\tlet flow = $state<RegistrationFlow>();",
    "after": "\tlet flow = $state<RegistrationFlow>();\r\n let registrationRecovery = $state<\"expired\" | \"unavailable\" | \"uncertain\">();\r\n let registrationMessage = $state(\"\");\r\n let registrationRenewing = $state(false);\r\n let registrationRenewalEpoch = 0;\r\n const renewRegistrationForm = async () => {\r\n  if (mutex.isLocked() || registrationRenewing || registrationWelcome || registrationRecovery === \"uncertain\") return;\r\n  const authIntent = authFormOwner.capture(flow?.id, () => flow?.id);\r\n  if (!authIntent.alive()) return;\r\n  const epoch = ++registrationRenewalEpoch, oldFlow = flow?.id;\r\n  const details = registrationDetails(ref), returnTo = safeRegistrationReturnTo(flow?.return_to || page.url.searchParams.get(\"redirect\"));\r\n  clearRegistrationPassword(ref);\r\n  registrationRenewing = true;\r\n  registrationMessage = \"Getting a fresh signup form\u2026\";\r\n  try {\r\n   const result = await Kratos.createBrowserRegistrationFlow({ returnTo });\r\n   if (!authIntent.alive() || epoch !== registrationRenewalEpoch) return;\r\n   if (result.status !== StatusCodes.OK || !result.data?.ui || !result.data.id || result.data.id === oldFlow) throw Error(\"No fresh form was returned\");\r\n   const next = retainRegistrationDetails(result.data, details);\r\n   const url = new URL(window.location.href);\r\n   url.searchParams.set(\"flow\", next.id); url.searchParams.set(\"redirect\", returnTo);\r\n   rememberRegistrationDetails(next.id, url.href, details);\r\n   if (!authIntent.replaceHistory(() => replaceState(url.pathname + url.search + url.hash, page.state))) return;\r\n   registrationRenewing = false;\r\n   registrationRecovery = undefined;\r\n   registrationMessage = \"Your form is ready. Re-enter your password, then choose Create Account.\";\r\n   flow = next;\r\n  } catch {\r\n   if (!authIntent.alive() || epoch !== registrationRenewalEpoch) return;\r\n   registrationRenewing = false;\r\n   registrationMessage = \"We couldn't get a fresh form. Your email and username are still here. Try again when your connection is ready.\";\r\n  }\r\n };\r\n const cancelRegistrationRenewal = () => {\r\n  registrationRenewalEpoch++;\r\n  registrationRenewing = false;\r\n  registrationMessage = \"Getting a fresh form was canceled. No account was submitted. You can try again when you're ready.\";\r\n };"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "replacement registration restores only exact one-use nonsecret details",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\t\t\t\tflow = res.data;",
    "after": "\t\t\t\tconst renewedDetails = takeRegistrationDetails(res.data.id, window.location.href);\r\n                flow = retainRegistrationDetails(res.data, renewedDetails);\r\n                if (renewedDetails) registrationMessage = \"Your form is ready. Re-enter your password, then choose Create Account.\";"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration initial load failure offers form retry instead of a blank card",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\t\t\tif (res instanceof AxiosError) {\r\n\t\t\t\tif (res.response?.data.error?.message) {\r\n\t\t\t\t\ttoast.error(res.response.data.error.message, { duration: 10000 });\r\n\t\t\t\t} else throw Constants.Error.Unknown;",
    "after": "\t\t\tif (res instanceof AxiosError) {\r\n                registrationRecovery = \"unavailable\";\r\n                registrationMessage = registrationError(res.response?.data, res.response?.status).message || \"We couldn't load the signup form. Please try again.\";"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration submit never retries expired or uncertain forms",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "if (!flow || !ref || registrationWelcome) {",
    "after": "if (!flow || !ref || registrationWelcome || registrationRecovery || registrationRenewing) {"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration field validation keeps only nonsecret details and rejects unrelated flow",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\t\t\t\tflow = data;\r\n\t\t\t} else if (data?.error?.reason) {\r\n\t\t\t\ttoast.error(data.error.reason, { duration: 10000 });\r\n\t\t\t} else throw Constants.Error.Unknown;",
    "after": "                clearRegistrationPassword(ref);\r\n                flow = retainRegistrationDetails(data, method === \"password\" ? body.traits : undefined);\r\n            } else {\r\n                clearRegistrationPassword(ref);\r\n                const status = res instanceof AxiosError ? res.response?.status : res.status;\r\n                const failure = registrationError(data, status);\r\n                if (failure.expired) {\r\n                    registrationRecovery = \"expired\";\r\n                    registrationMessage = failure.message;\r\n                } else if (status !== undefined && status >= 400 && status < 500 && failure.message) {\r\n                    registrationMessage = failure.message;\r\n                    toast.error(failure.message, { duration: 10000 });\r\n                } else {\r\n                    registrationRecovery = \"uncertain\";\r\n                    registrationMessage = \"We couldn't confirm whether your account was created. Try signing in before submitting again. This form will not resend your details.\";\r\n                }\r\n            }"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration duplicate submit and recovery state stop before mutex dispatch",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "if (mutex.isLocked() || registrationWelcome) return;",
    "after": "if (mutex.isLocked() || registrationWelcome || registrationRecovery || registrationRenewing) return;"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "OIDC registration also refuses duplicate or expired form submissions",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\tconst onOidc = async (name: string, provider: string) =>\r\n\t\tmutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));",
    "after": "\tconst onOidc = async (name: string, provider: string) => {\r\n        if (mutex.isLocked() || registrationWelcome || registrationRecovery || registrationRenewing) return;\r\n        return mutex.exec(async () => updateFlow(\"oidc\", { [name]: provider }));\r\n    };"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "registration recovery provides an accessible explicit refresh and cancel action",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\t\t\t<form onsubmit={onSubmit} bind:this={ref}>",
    "after": "            {#if registrationMessage}\r\n                <section class=\"mb-4 space-y-2 rounded border p-3\" aria-label=\"Signup form help\">\r\n                    <p role={registrationRecovery ? \"alert\" : \"status\"}>{registrationMessage}</p>\r\n                    {#if registrationRecovery === \"expired\" || registrationRecovery === \"unavailable\"}\r\n                        <Button type=\"button\" variant=\"outline\" disabled={registrationRenewing || $mutex} onclick={renewRegistrationForm}>Get a fresh signup form</Button>\r\n                    {/if}\r\n                    {#if registrationRenewing}<Button type=\"button\" variant=\"ghost\" onclick={cancelRegistrationRenewal}>Cancel</Button>{/if}\r\n                    {#if registrationRecovery === \"uncertain\"}<a class=\"underline\" href={\"/auth/login?redirect=\" + encodeURIComponent(safeRegistrationReturnTo(flow?.return_to))}>Try signing in</a>{/if}\r\n                </section>\r\n            {/if}\r\n\t\t\t<form onsubmit={onSubmit} bind:this={ref}>",
    "approved_registration_recovery_exception": "explicit_signup_form_recovery"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "expired signup cannot submit until a fresh form is explicitly ready",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "<Button type=\"submit\" disabled={$mutex}>Create Account</Button>",
    "after": "<Button type=\"submit\" disabled={$mutex || !!registrationRecovery || registrationRenewing || !flow}>Create Account</Button>",
    "approved_registration_recovery_exception": "explicit_signup_form_recovery"
  },
  {
    "path": "src/routes/(app)/auth/register/+page.svelte",
    "label": "expired signup cannot submit another provider through old flow",
    "sha256": "e2b34dfce161f0384fbb311f3a322a35324db13a9167ef681431be499e943a3c",
    "count": 1,
    "before": "\t\t\t\t\t\t\t\t\tdisabled={$mutex}\r\n\t\t\t\t\t\t\t\t\tonclick={() => onOidc(node.attributes.name, node.attributes.value)}",
    "after": "\t\t\t\t\t\t\t\t\tdisabled={$mutex || !!registrationRecovery || registrationRenewing || !flow}\r\n\t\t\t\t\t\t\t\t\tonclick={() => onOidc(node.attributes.name, node.attributes.value)}",
    "approved_registration_recovery_exception": "explicit_signup_form_recovery"
  }
];
