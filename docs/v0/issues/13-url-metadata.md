## Parent

Part of #1.

## What to build

Add deterministic native URL metadata retrieval for a Note classified or detected as a reference URL. Rust retrieves bounded public HTTP(S) content, extracts final URL/title/description/short text excerpt, and passes the validated object into Prompt A. Fetched content is always untrusted data.

## Decisions

- Only explicit `http` and `https` URLs are eligible.
- Resolve hostnames before connecting and reject loopback, private, link-local, multicast, reserved, documentation, carrier-grade NAT, and cloud-metadata destinations for IPv4 and IPv6.
- Revalidate every redirect destination after DNS resolution; cap redirects at five.
- Use a six-second total timeout, 2 MiB response limit, and 2,000-character normalized excerpt limit.
- Accept HTML/XHTML for metadata extraction. For other content types, return status plus safe type description without reading arbitrary binary bodies.
- Do not execute scripts, load subresources, evaluate DOM, store cookies, or send credentials/referrers.
- Extract Open Graph title/description, standard title/description, and visible text with deterministic precedence.
- A retrieval failure does not block Note save or ordinary AI organization; Prompt A receives null/error metadata.
- No web search or general browsing.

## Acceptance criteria

- [ ] Public HTML URL Notes receive bounded metadata and Prompt A can use it as data.
- [ ] Invalid schemes and every prohibited address category fail before content is exposed.
- [ ] Redirects are capped and each destination is revalidated.
- [ ] Timeout, DNS failure, 404, other HTTP errors, oversized body, malformed HTML, and non-text content return typed outcomes.
- [ ] Note persistence and manual use succeed when retrieval fails.
- [ ] Fetched instructions cannot alter the approved Prompt A task.
- [ ] Logs and errors avoid leaking response bodies.
- [ ] The old Next server route is not needed by the desktop path.

## Testing decisions

- Use controlled local fixtures while testing the validator separately from allowed network routing.
- Cover IPv4/IPv6 ranges, hostname resolution, redirect-to-private, DNS rebinding defense, timeout, size limits, content types, extraction precedence, Unicode, and prompt-injection HTML.
- Test the end-to-end reference Note -> metadata -> organization path with fake network/provider adapters.

## Blocked by

- #12

## Scope fence

Do not implement web search, crawling, PDF parsing, authentication, cookies, browser rendering, attachments, or arbitrary URL preview storage.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
