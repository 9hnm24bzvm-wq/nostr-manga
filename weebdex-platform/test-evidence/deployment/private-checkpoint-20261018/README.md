# Private checkpoint finite seam

Eighteen stdlib tests pass against synthetic files and an injected PostgreSQL command stand-in. The official installed libpq parses synthetic connection values without connecting. No real dump, restore, listener, service or runtime database was used. The runbook explicitly requires an isolated operator recovery drill before production reliance.
