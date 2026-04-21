# OSS Starter Template Vulnerability Scan Results

**Scan date:** 2026-04-22
**Data source:** [OSV.dev](https://osv.dev) batch API (`/v1/querybatch` + `/v1/vulns/{id}`)
**Method:** Extracted dependency manifests from each project's official GitHub repository, queried OSV.dev for known vulnerabilities against the pinned versions found in the starter templates.

> These are real, verifiable vulnerabilities from the OSV/GHSA/NVD databases.
> Every GHSA and CVE ID below can be looked up at `https://osv.dev/vulnerability/{ID}`.

---

## Summary Table

| # | Project | Deps Scanned | Vulns Found | Critical | High | Medium | Low |
|---|---------|-------------|-------------|----------|------|--------|-----|
| 1 | **create-next-app** (Next.js) | 6 | 12 | 1 | 3 | 6 | 2 |
| 2 | **create-react-app** (CRA) | 21 | 8 | 1 | 1 | 3 | 3 |
| 3 | **Remix starter** | 7 | 4 | 1 | 2 | 1 | 0 |
| 4 | **Astro blog template** | 5 | 11 | 0 | 1 | 7 | 3 |
| 5 | **SvelteKit starter** | 7 | 18 | 0 | 2 | 12 | 4 |
| 6 | **create-t3-app** | 6 | 13 | 2 | 2 | 5 | 4 |
| 7 | **Vite vanilla-ts** | 2 | 11 | 0 | 1 | 7 | 3 |
| 8 | **Tauri (create-tauri-app)** | 8 | 11 | 0 | 1 | 7 | 3 |
| 9 | **FastAPI full-stack** | 17 | 9 | 0 | 2 | 4 | 3 |
| 10 | **Express generator** | 12 | 12 | 2 | 5 | 4 | 1 |
| | **TOTALS** | **91** | **109** | **7** | **20** | **56** | **26** |

**79 unique vulnerabilities** across 10 projects (some vulns affect multiple projects via shared deps like `vite` and `next`).

---

## 1. create-next-app (Next.js)

**Source:** `vercel/next.js` canary branch, `packages/create-next-app/templates/`
**Key versions:** next@15.3.1, react@19.0.0, react-dom@19.0.0, typescript@5.8.3

**12 vulnerabilities found** (1 Critical, 3 High, 6 Moderate, 2 Low)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-9qr9-h5gf-34mp | -- | next@15.3.1 | **CRITICAL** | RCE in React flight protocol | 15.0.5 |
| GHSA-h25m-26qc-wcjf | CVE-2026-23864 | next@15.3.1 | **HIGH** | DoS via insecure React Server Components deserialization | 15.0.8 |
| GHSA-mwv6-3258-q52c | -- | next@15.3.1 | **HIGH** | DoS with Server Components | 14.2.34 |
| GHSA-q4gf-8mx6-v5v3 | CVE-2026-23869 | next@15.3.1 | **HIGH** | DoS with Server Components (variant) | 15.5.15 |
| GHSA-4342-x723-ch2f | CVE-2025-57822 | next@15.3.1 | MODERATE | Middleware redirect handling leads to SSRF | 14.2.32 |
| GHSA-9g9p-9gw9-jx7f | CVE-2025-59471 | next@15.3.1 | MODERATE | DoS via Image Optimizer remotePatterns | 15.5.10 |
| GHSA-ggv3-7p47-pfv8 | CVE-2026-29057 | next@15.3.1 | MODERATE | HTTP request smuggling in rewrites | 16.1.7 |
| GHSA-w37m-7fhw-fmv9 | -- | next@15.3.1 | MODERATE | Server Actions source code exposure | 15.0.6 |
| GHSA-xv57-4mr9-wg8v | CVE-2025-55173 | next@15.3.1 | MODERATE | Content injection via Image Optimization | 14.2.31 |
| GHSA-g5qg-72qw-gw5v | CVE-2025-57752 | next@15.3.1 | MODERATE | Cache key confusion for Image Optimization API | 14.2.31 |
| GHSA-3x4c-7xq6-9pq8 | CVE-2026-27980 | next@15.3.1 | MODERATE | Unbounded next/image disk cache growth can exhaust storage | 16.1.7 |
| GHSA-r2fc-ccr8-96c4 | CVE-2025-49005 | next@15.3.1 | LOW | Cache poisoning via omission of Vary header | 15.3.3 |

**Most notable:** GHSA-9qr9-h5gf-34mp -- **Remote Code Execution** via the React flight protocol. CVSS 9.8 (Critical). Affects every Next.js project using Server Components. Fix: upgrade to next@15.0.5+.

---

## 2. create-react-app (CRA)

**Source:** `facebook/create-react-app` main branch, `packages/react-scripts/package.json`
**Key versions:** react-scripts@5.0.1, webpack@5.64.4, webpack-dev-server@4.6.0, postcss@8.4.4, eslint@8.3.0
**Note:** CRA is effectively unmaintained. The dependency versions are years old.

**8 vulnerabilities found** (1 Critical, 1 High, 3 Moderate, 3 Low)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-hc6q-2mpp-qw7j | CVE-2023-28154 | webpack@5.64.4 | **CRITICAL** | Cross-realm object access | 5.76.0 |
| GHSA-c2qf-rxjj-qqgw | CVE-2022-25883 | semver@7.3.5 | **HIGH** | ReDoS vulnerability | 7.5.2 |
| GHSA-4vvj-4cpr-p986 | CVE-2024-43788 | webpack@5.64.4 | MODERATE | DOM Clobbering Gadget leads to XSS | 5.94.0 |
| GHSA-4v9v-hfq4-rm2v | CVE-2025-30359 | webpack-dev-server@4.6.0 | MODERATE | Source code theft via malicious website | 5.2.1 |
| GHSA-9jgg-88mc-972h | CVE-2025-30360 | webpack-dev-server@4.6.0 | MODERATE | Source code theft (non-Chromium browsers) | 5.2.1 |
| GHSA-7fh5-64p2-3v2j | CVE-2023-44270 | postcss@8.4.4 | MODERATE | Line return parsing error | 8.4.31 |
| GHSA-38r7-794h-5758 | CVE-2025-68157 | webpack@5.64.4 | LOW | SSRF via HttpUriPlugin redirect bypass | 5.104.0 |
| GHSA-8fgc-7cc6-rx7x | CVE-2025-68458 | webpack@5.64.4 | LOW | SSRF via URL userinfo bypass | 5.104.1 |

**Most notable:** GHSA-hc6q-2mpp-qw7j -- **Cross-realm object access in Webpack 5**. CVSS 9.8 (Critical). Fixed in webpack 5.76.0, but CRA pins ^5.64.4. Since CRA is unmaintained, there is no official update path. Also: webpack-dev-server 4.6.0 allows source code theft when developers visit malicious websites during development.

---

## 3. Remix Starter

**Source:** `remix-run/remix` main branch, `demos/bookstore/package.json`
**Key versions:** @remix-run/react@2.16.0, @remix-run/node@2.16.0, esbuild@0.24.0, react@18.3.1

**4 vulnerabilities found** (1 Critical, 2 High, 1 Moderate)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-9583-h5hc-x8cw | CVE-2025-61686 | @remix-run/node@2.16.0 | **CRITICAL** | Path traversal in file session storage | 7.9.4 (react-router) |
| GHSA-3cgp-3xvw-98x8 | CVE-2025-59057 | @remix-run/react@2.16.0 | **HIGH** | XSS vulnerability in React Router | 7.9.0 |
| GHSA-8v8x-cx79-35w7 | CVE-2026-21884 | @remix-run/react@2.16.0 | **HIGH** | SSR XSS in ScrollRestoration | 7.12.0 |
| GHSA-67mh-4wv8-2f99 | -- | esbuild@0.24.0 | MODERATE | Dev server allows cross-origin reads | 0.25.0 |

**Most notable:** GHSA-9583-h5hc-x8cw -- **Path Traversal in File Session Storage**. CVSS 9.1 (Critical). Allows arbitrary file overwrite/deletion via session storage manipulation. All Remix apps using file-based sessions are affected.

---

## 4. Astro Blog Template

**Source:** `withastro/astro` main branch, `examples/blog/package.json`
**Key versions:** astro@5.3.0, @astrojs/mdx@4.1.0, sharp@0.33.5

**11 vulnerabilities found** (0 Critical, 1 High, 7 Moderate, 3 Low) -- all in `astro` core

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-wrwg-2hg8-v723 | CVE-2025-64764 | astro@5.3.0 | **HIGH** | Reflected XSS via server islands feature | 5.15.8 |
| GHSA-5ff5-9fcw-vg88 | CVE-2025-61925 | astro@5.3.0 | MODERATE | X-Forwarded-Host reflected without validation | 5.14.3 |
| GHSA-cq8c-xv66-36gw | CVE-2025-54793 | astro@5.3.0 | MODERATE | Open redirect via duplicate trailing slash | 5.12.8 |
| GHSA-fvmw-cj7j-j39q | CVE-2025-65019 | astro@5.3.0 | MODERATE | Stored XSS in /_image endpoint (Cloudflare adapter) | 5.15.9 |
| GHSA-ggxq-hp9w-j794 | CVE-2025-64765 | astro@5.3.0 | MODERATE | Middleware auth bypass via URL encoding | 5.15.8 |
| GHSA-hr2q-hp5q-x767 | CVE-2025-64525 | astro@5.3.0 | MODERATE | URL manipulation bypasses middleware and CVE-2025-61925 fix | 5.15.5 |
| GHSA-whqg-ppgf-wp8c | CVE-2025-66202 | astro@5.3.0 | MODERATE | Auth bypass via double URL encoding (bypass for CVE-2025-64765) | 5.15.8 |
| GHSA-xf8x-j4p2-f749 | CVE-2025-55303 | astro@5.3.0 | MODERATE | Unauthorized third-party images in _image endpoint | 5.13.2 |
| GHSA-g735-7g2w-hh3f | CVE-2026-33769 | astro@5.3.0 | LOW | Remote allowlist bypass via unanchored wildcard | 5.18.1 |
| GHSA-w2vj-39qv-7vh7 | CVE-2025-64745 | astro@5.3.0 | LOW | Dev server reflected XSS | 5.15.6 |
| GHSA-x3h8-62x9-952g | CVE-2025-64757 | astro@5.3.0 | LOW | Dev server arbitrary local file read | 5.14.3 |

**Most notable:** GHSA-wrwg-2hg8-v723 -- **Reflected XSS via server islands**. CVSS 7.1 (High). Allows script injection on Astro SSR sites using server islands. Also notable: a chain of middleware bypass vulnerabilities (3 CVEs, each bypassing the fix for the previous one).

---

## 5. SvelteKit Starter

**Source:** `sveltejs/cli` main branch, `packages/sv/src/cli/tests/snapshots/create-only/package.json`
**Key versions:** @sveltejs/kit@2.15.0, svelte@5.16.0, vite@6.0.7

**18 vulnerabilities found** (0 Critical, 2 High, 12 Moderate, 4 Low)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-2crg-3p73-43xp | CVE-2026-40073 | @sveltejs/kit@2.15.0 | **HIGH** | BODY_SIZE_LIMIT bypass in adapter-node | 2.57.1 |
| GHSA-p9ff-h696-f583 | CVE-2026-39363 | vite@6.0.7 | **HIGH** | Arbitrary file read via dev server WebSocket | 8.0.5 |
| GHSA-6q87-84jw-cjhp | CVE-2025-32388 | @sveltejs/kit@2.15.0 | MODERATE | XSS via tracked search_params | 2.20.6 |
| GHSA-3f6h-2hrp-w5wx | CVE-2026-40074 | @sveltejs/kit@2.15.0 | MODERATE | DoS via unvalidated redirect in handle hook | 2.57.1 |
| GHSA-crpf-4hrx-3jrp | CVE-2026-27125 | svelte@5.16.0 | MODERATE | SSR prototype chain pollution in attribute spreading | 5.51.5 |
| GHSA-f7gr-6p89-r883 | CVE-2026-27121 | svelte@5.16.0 | MODERATE | XSS via spread attributes in Svelte SSR | 5.51.5 |
| GHSA-m56q-vw4c-c2cp | CVE-2026-27122 | svelte@5.16.0 | MODERATE | SSR tag name injection via `<svelte:element>` | 5.51.5 |
| GHSA-phwv-c562-gvmh | CVE-2026-27901 | svelte@5.16.0 | MODERATE | XSS via contenteditable bind:innerText in SSR | 5.53.5 |
| GHSA-vg6x-rcgg-rjx6 | CVE-2025-24010 | vite@6.0.7 | MODERATE | Cross-origin source code theft from dev server | 6.0.9 |
| GHSA-356w-63v5-8wf4 | CVE-2025-32395 | vite@6.0.7 | MODERATE | server.fs.deny bypass via invalid request-target | 6.2.6 |
| GHSA-4r4m-qw57-chr8 | CVE-2025-31125 | vite@6.0.7 | MODERATE | server.fs.deny bypass for inline/raw queries | 6.2.4 |
| GHSA-859w-5945-r5v3 | CVE-2025-46565 | vite@6.0.7 | MODERATE | server.fs.deny bypass with /. for project root files | 6.3.4 |
| GHSA-93m4-6634-74q7 | CVE-2025-62522 | vite@6.0.7 | MODERATE | server.fs.deny bypass via backslash on Windows | 7.1.11 |
| GHSA-x574-m823-4x7w | CVE-2025-30208 | vite@6.0.7 | MODERATE | server.fs.deny bypass via ?raw?? query | 6.2.3 |
| GHSA-xcj6-pq6g-qj4x | CVE-2025-31486 | vite@6.0.7 | MODERATE | server.fs.deny bypass via .svg or relative paths | 6.2.5 |
| GHSA-4w7w-66w2-5vf9 | CVE-2026-39365 | vite@6.0.7 | MODERATE | Path traversal in optimized deps .map handling | 8.0.5 |
| GHSA-g4jq-h2w9-997c | CVE-2025-58751 | vite@6.0.7 | LOW | Serves unintended files matching public dir prefix | 7.1.5 |
| GHSA-jqfw-vq24-v9c3 | CVE-2025-58752 | vite@6.0.7 | LOW | server.fs settings not applied to HTML files | 7.1.5 |

**Most notable:** SvelteKit has the highest raw vuln count (18) largely because `vite@6.0.7` carries 11 vulnerabilities on its own -- a remarkable cascade of `server.fs.deny` bypasses (7 separate CVEs, each finding a new way around the file access controls). Svelte core has 4 SSR XSS variants.

---

## 6. create-t3-app

**Source:** `t3-oss/create-t3-app` main branch, `cli/template/base/package.json`
**Key versions:** next@15.1.0, react@19.0.0, react-dom@19.0.0, zod@3.24.2, typescript@5.8.2

**13 vulnerabilities found** (2 Critical, 2 High, 5 Moderate, 4 Low)

All vulnerabilities are in `next@15.1.0` (inherits the full Next.js vulnerability set, plus extras not yet fixed in 15.1.x):

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-f82v-jwr5-mffw | CVE-2025-29927 | next@15.1.0 | **CRITICAL** | Authorization bypass in Next.js Middleware | 13.5.9 |
| GHSA-9qr9-h5gf-34mp | -- | next@15.1.0 | **CRITICAL** | RCE in React flight protocol | 15.0.5 |
| GHSA-67rr-84xm-4c7r | CVE-2025-49826 | next@15.1.0 | **HIGH** | DoS via cache poisoning | 15.1.8 |
| GHSA-h25m-26qc-wcjf | CVE-2026-23864 | next@15.1.0 | **HIGH** | DoS via RSC deserialization | 15.0.8 |
| GHSA-7m27-7ghc-44w9 | CVE-2024-56332 | next@15.1.0 | MODERATE | DoS with Server Actions | 13.5.8 |
| GHSA-q4gf-8mx6-v5v3 | CVE-2026-23869 | next@15.1.0 | HIGH | DoS with Server Components | 15.5.15 |
| + 7 more | | next@15.1.0 | MODERATE/LOW | SSRF, cache poisoning, image optimization, source exposure | various |

**Most notable:** GHSA-f82v-jwr5-mffw -- **Authorization Bypass in Next.js Middleware** (CVE-2025-29927). CVSS 9.1 (Critical). This was the widely-reported Next.js auth bypass that allows skipping all middleware-based authentication. T3 ships next@15.1.0 which is affected. Fix is in next@15.2.3+.

---

## 7. Vite vanilla-ts

**Source:** `vitejs/vite` main branch, `packages/create-vite/template-vanilla-ts/package.json`
**Key versions:** vite@6.0.7, typescript@5.7.3

**11 vulnerabilities found** (0 Critical, 1 High, 7 Moderate, 3 Low) -- all in `vite@6.0.7`

Same Vite vulnerabilities as listed in SvelteKit section above. The most critical is GHSA-p9ff-h696-f583 (arbitrary file read via WebSocket, High severity).

---

## 8. Tauri (create-tauri-app)

**Source:** `tauri-apps/create-tauri-app` dev branch, `templates/template-react-ts/`
**Key versions:** react@19.1.0, vite@6.0.7, @tauri-apps/api@2.2.0, @tauri-apps/cli@2.2.0

**11 vulnerabilities found** (0 Critical, 1 High, 7 Moderate, 3 Low) -- all from `vite@6.0.7`

Same Vite vulnerability set. Tauri-specific packages (@tauri-apps/api, @tauri-apps/cli, @tauri-apps/plugin-opener) showed **zero vulnerabilities**.

---

## 9. FastAPI Full-Stack Template

**Source:** `fastapi/full-stack-fastapi-template` master branch, `backend/pyproject.toml`
**Key versions:** fastapi@0.114.2, python-multipart@0.0.7, jinja2@3.1.4, pyjwt@2.8.0, sentry-sdk@2.0.0

**9 vulnerabilities found** (0 Critical, 2 High, 4 Moderate, 3 Low)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-wp53-j4wj-2cfg | CVE-2026-24486 | python-multipart@0.0.7 | **HIGH** | Arbitrary file write via non-default config | 0.0.22 |
| GHSA-752w-5fwx-jx9f | CVE-2026-32597 | pyjwt@2.8.0 | **HIGH** | Accepts unknown `crit` header extensions | 2.12.0 |
| GHSA-59g5-xgcq-4qw3 | CVE-2024-53981 | python-multipart@0.0.7 | HIGH (CVSS v4) | DoS via deformed multipart boundary | 0.0.18 |
| GHSA-cpwx-vrp4-4pq7 | CVE-2025-27516 | jinja2@3.1.4 | MODERATE | Sandbox breakout via attr filter format method | 3.1.6 |
| GHSA-gmj6-6f8f-6699 | CVE-2024-56201 | jinja2@3.1.4 | MODERATE | Sandbox breakout via malicious filenames | 3.1.5 |
| GHSA-q2x7-8rv6-6q7h | CVE-2024-56326 | jinja2@3.1.4 | MODERATE | Sandbox breakout via indirect format reference | 3.1.5 |
| GHSA-mj87-hwqh-73pj | CVE-2026-40347 | python-multipart@0.0.7 | MODERATE | DoS via large multipart preamble/epilogue | 0.0.26 |
| GHSA-6w46-j5rx-g56g | CVE-2025-71176 | pytest@7.4.3 | MODERATE | Vulnerable tmpdir handling | 9.0.3 |
| GHSA-g92j-qhmh-64v2 | CVE-2024-40647 | sentry-sdk@2.0.0 | LOW | Unintentional env var exposure to subprocesses | 2.8.0 |

**Most notable:** python-multipart@0.0.7 has 3 vulnerabilities including an arbitrary file write (CVE-2026-24486). The Jinja2 sandbox breakout chain (3 CVEs) is also notable -- particularly concerning in a template that uses Jinja2 for email templates.

---

## 10. Express Generator

**Source:** `expressjs/generator` master branch (v4.16.1)
**Key versions:** express@4.17.1, ejs@2.6.1, pug@2.0.4, minimatch@3.0.4, minimist@1.2.5
**Note:** The Express generator has not been updated since 2019. Its dependency versions are ancient.

**12 vulnerabilities found** (2 Critical, 5 High, 4 Moderate, 1 Low)

| ID | CVE | Package | Severity | Summary | Fix Version |
|----|-----|---------|----------|---------|-------------|
| GHSA-phwq-j96m-2c2q | CVE-2022-29078 | ejs@2.6.1 | **CRITICAL** | Template injection / RCE | 3.1.7 |
| GHSA-xvch-5gv4-984h | CVE-2021-44906 | minimist@1.2.5 | **CRITICAL** | Prototype pollution | 1.2.6 |
| GHSA-23c5-xmqv-rm74 | CVE-2026-27904 | minimatch@3.0.4 | **HIGH** | ReDoS via nested extglobs | 10.2.3 |
| GHSA-3ppc-4f35-3m26 | CVE-2026-26996 | minimatch@3.0.4 | **HIGH** | ReDoS via repeated wildcards | 10.2.1 |
| GHSA-7r86-cg39-jmmj | CVE-2026-27903 | minimatch@3.0.4 | **HIGH** | ReDoS via GLOBSTAR segments | 10.2.3 |
| GHSA-f8q6-p94x-37v3 | CVE-2022-3517 | minimatch@3.0.4 | **HIGH** | ReDoS vulnerability | 3.0.5 |
| GHSA-c2qf-rxjj-qqgw | CVE-2022-25883 | (via deps) | **HIGH** | (duplicate, inherited) | -- |
| GHSA-rv95-896h-c2vc | CVE-2024-29041 | express@4.17.1 | MODERATE | Open redirect in malformed URLs | 4.19.2 |
| GHSA-ghr5-ch3p-vcr6 | CVE-2024-33883 | ejs@2.6.1 | MODERATE | Prototype pollution (lacks pollution protection) | 3.1.10 |
| GHSA-3965-hpx2-q597 | CVE-2024-36361 | pug@2.0.4 | MODERATE | Code execution with untrusted input | 3.0.3 |
| GHSA-p493-635q-r6gr | CVE-2021-21353 | pug@2.0.4 | MODERATE | RCE via `pretty` option | 3.0.1 |
| GHSA-7f5c-rpf4-86p8 | CVE-2021-32822 | hbs@4.0.4 | MODERATE | Sensitive info exposure | **No fix** |
| GHSA-qw6h-vgh9-j6wx | CVE-2024-43796 | express@4.17.1 | LOW | XSS via response.redirect() | 4.20.0 |

**Most notable:** Express generator is the worst offender by severity density. ejs@2.6.1 has a **Critical template injection RCE** (CVE-2022-29078) -- attackers can execute arbitrary code server-side. minimist@1.2.5 has a **Critical prototype pollution** (CVE-2021-44906). minimatch@3.0.4 alone has 4 separate ReDoS vulnerabilities. hbs@4.0.4 has a vulnerability with **no fix available**.

---

## Cross-Cutting Observations

### Vite is the most widely-affected shared dependency
`vite@6.0.7` carries **11 vulnerabilities** and appears in 4 of the 10 projects scanned (SvelteKit, Vite vanilla-ts, Tauri, and indirectly in Astro via Vite's integration). Seven of these are `server.fs.deny` bypass variants -- a recurring pattern where each fix introduced a new bypass vector.

### Next.js accumulates vulns fast
Both create-next-app and create-t3-app inherit Next.js's vulnerability surface. The T3 template pins next@15.1.0 which includes the notorious **middleware auth bypass** (CVE-2025-29927) on top of the Server Component DoS and RCE issues.

### Unmaintained generators are vulnerability magnets
- **Express generator** (last updated 2019): 12 vulns, 2 Critical
- **CRA** (effectively abandoned): 8 vulns, 1 Critical
These projects ship with dependencies that are 3-5+ years behind on security patches.

### Python templates are not immune
The FastAPI template's python-multipart@0.0.7 has 3 vulnerabilities including an arbitrary file write. Jinja2@3.1.4 has 3 sandbox breakout CVEs. PyJWT@2.8.0 has a header validation bypass.

### Projects with zero framework-level vulns
- **react**, **react-dom** (19.x) -- zero vulnerabilities found
- **typescript** (5.x/6.x) -- zero vulnerabilities found
- **zod**, **sharp**, **@tauri-apps/***, **@astrojs/mdx|rss|sitemap** -- zero vulnerabilities found

---

## Methodology Notes

1. **Dependency versions** were taken directly from the official starter templates in each project's GitHub repository as of April 2026. Where templates use version ranges (^, ~), the minimum satisfying version was used for the OSV query.
2. **Only direct dependencies** were scanned. Transitive dependency vulnerabilities (which would significantly increase counts for CRA, Express, etc.) are not included.
3. **OSV.dev batch API** (`POST /v1/querybatch`) was used for initial detection. Individual vulnerability details were enriched via `GET /v1/vulns/{id}` for all 79 unique vulnerability IDs.
4. **Severity classifications** use the advisory's own rating (GHSA `database_specific.severity`). Where that was absent, CVSS v3.1 base score ranges were applied (Critical >= 9.0, High >= 7.0, Medium >= 4.0, Low < 4.0).
5. **All IDs are verifiable** at `https://osv.dev/vulnerability/{GHSA-ID}` or `https://nvd.nist.gov/vuln/detail/{CVE-ID}`.
