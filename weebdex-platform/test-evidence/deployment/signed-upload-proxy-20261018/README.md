# Signed upload proxy ceilings

Base source: `f2a4f7d`. Two concrete deployment-template defects were repaired: valid UUID signed multipart routes inherited1MiB/30s instead of the existing Rust upload ceilings; existing unquoted regex quantifiers exposed braces to nginx configuration tokenization.

The example now quotes regex arguments and routes only exact hyphenated UUID shapes for `/api/v1/upload/{session}` (96MiB), `/api/v1/nostr/uploads/{session}/files/{file}/replace` (40MiB), and `/api/v1/cover/{manga}` (40MiB), each with180s proxy body inactivity timeout. Non-POST method-specific JSON budgets remain authoritative in unchanged Rust code. Native short-ID routes, URI/query/method/body/header forwarding, streaming, admin restriction and native WebSocket behavior remain unchanged.

Ten finite directive/route/source-budget fixtures pass. They include uppercase/custom-version/nil UUID shapes, native paths, malformed/control/preview/lookalike negatives, pre-fix1MiB witness and unchanged proxy security controls. No backend/original protocol source changed.

Official nginx syntax execution is not claimed: no executable was available in PATH or the checked recovered vendor locations, and no installation/build was authorized. Operators must still run their official `nginx -t` with their provisioned certificates before enabling the service, followed by actual large signed upload/replacement/cover requests through that proxy.

Primary syntax references: [nginx location selection](https://nginx.org/en/docs/http/ngx_http_core_module.html#location), [quoted regex quantifiers](https://nginx.org/en/docs/http/server_names.html#regex_names), and [official configuration token reader](https://github.com/nginx/nginx/blob/release-1.28.0/src/core/ngx_conf_file.c#L678-L762). The finite lexer is deliberately a bounded seam for this example, not a replacement for the official parser.
