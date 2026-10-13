# Independent resolver fault setup

`endpoint_dns.rs` compiles this small Linux libc shim using the installed C compiler
and loads it only into its dedicated child process. It changes no production API,
system resolver configuration or global process environment.

Reserved `.invalid` names yield a public/private mixed answer, no answers, too many
answers, or a first loopback answer followed by an unreachable loopback rebound.
Only the application's public `EndpointPolicy::connect` and `VerifiedEndpoint`
operations determine pass/fail. No resolver call counts or private policy helpers
are assertion oracles.

The shim refuses every port-443 socket locally. This lets the public-literal cases
observe the actual caller-visible transport-error class without sending traffic
to public addresses. The successful pinning case uses a real local HTTP identity
and metadata peer on an ephemeral port. The fixture needs Linux dynamic linking
and `cc`; other platforms retain the portable public policy module tests.
