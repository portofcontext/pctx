# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.3.4 (2026-10-05)

### Bug Fixes

 - <csr-id-56f08e457cf309584f76aa6fb72c619681e490c9/> union array item ()

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release over the course of 35 calendar days.
 - 65 days passed between releases.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Merge pull request #155 from portofcontext/fix/153-array-item-union-parens ([`391c286`](https://github.com/portofcontext/pctx/commit/391c28629f8f0b65cdad9cbdec95925a5cd78385))
    - Union array item () ([`56f08e4`](https://github.com/portofcontext/pctx/commit/56f08e457cf309584f76aa6fb72c619681e490c9))
    - Union array item test cases ([`c7dd9dc`](https://github.com/portofcontext/pctx/commit/c7dd9dc8c91be57ab713666dbbcbd3a8c59b409d))
</details>

## 0.3.3 (2026-07-31)

### Bug Fixes

 - <csr-id-6ec96875f8619e0a44f31f43db6108ef88b0bd2b/> report degraded tool types to the caller
   Two follow-ups from review:
   
   - Codegen degradation to `any` only reached the logs, so a client
     registering tools against a remotely deployed session server had no way
     to learn its tool lost its types. `Tool` now records why each schema was
     degraded, `CallbackReport` carries them as `warnings`, and
     `/register/tools` returns them alongside `failed`.
   - `with_callbacks` swallowed the `CallbackReport` from `add_callbacks`,
     hiding both failures and warnings from the builder-style caller. It now
     returns `(Self, CallbackReport)`. Breaking, and infallible: per-tool
     isolation means the batch cannot fail, so the report is the only
     outcome.
 - <csr-id-d42790b14c54030d5588c920f05e72f7ab7f1901/> isolate per-tool registration failures
   `/register/tools` failed the whole batch when a single tool's schema
   couldn't be typed by our codegen (e.g. a recursive `$ref`). The portal
   federates arbitrary upstream JSON Schema, so this fired constantly: in
   prod each session degraded to ~135 sequential per-tool register calls
   (~6.4s, ~274 daily 500s) instead of one batch call.
   
   Fix it at the layer that owns the problem:
   
   - Codegen never fails a tool over typing. `Tool::new` is infallible;
     `generate_types` failures degrade to a permissive `any` signature
     (with a warning) so the tool stays callable, just untyped.
   - Registration isolates per-tool. `add_callbacks` returns a
     `CallbackReport { registered, failed }` instead of bubbling the first
     error; a genuinely bad tool (name clash, unparseable schema) is
     skipped and reported, never aborts the batch. The handler returns 200
     with the report.
   
   The portal's existing batch call now succeeds, so its per-tool fallback
   never triggers — no portal change required to fix prod.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 9 days passed between releases.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_codegen v0.3.3, pctx_registry v0.1.3, pctx_code_execution_runtime v0.2.2, pctx_executor v0.2.2, pctx_code_mode v0.5.0, safety bump pctx v0.8.0 ([`0b4e9ec`](https://github.com/portofcontext/pctx/commit/0b4e9ec255a11ed81012a0ea88c4bb1b8b1f6473))
    - Merge branch 'main' into fix/concurrency ([`369d302`](https://github.com/portofcontext/pctx/commit/369d302e4ab37cc51a137991c2b6dd7b4100ab71))
    - Merge pull request #136 from portofcontext/fix/register-tools-per-tool-isolation ([`5269d81`](https://github.com/portofcontext/pctx/commit/5269d81884c64114607d42b459243e8d820db794))
    - Report degraded tool types to the caller ([`6ec9687`](https://github.com/portofcontext/pctx/commit/6ec96875f8619e0a44f31f43db6108ef88b0bd2b))
    - Isolate per-tool registration failures ([`d42790b`](https://github.com/portofcontext/pctx/commit/d42790b14c54030d5588c920f05e72f7ab7f1901))
</details>

## 0.3.2 (2026-07-22)

### Bug Fixes

 - <csr-id-555ed2b069ee1bbf4154780216c77c84a41a3060/> resolve in-document pointer $refs during type generation
   The recursion fix in v0.3.1 handles `$defs`-named recursive refs, but not a
   `$ref` that is an in-document JSON pointer — e.g. a recursive query filter
   whose `and`/`or` groups reference the filter itself via
   `#/properties/filter/anyOf/0`. `schema_type::follow` resolves a ref by its
   trailing segment against the definitions map, so that pointer became
   `#/$defs/0` → "does not exist" → type generation errored, and consumers
   dropped the whole tool's input type to `any`.
   
   Add a normalization pass (`normalize::normalize_in_document_refs`) run at
   the top of `generate_types`: each distinct in-document pointer target is
   hoisted into `definitions` under a generated name, and every ref to it —
   including refs inside the hoisted target, so a self-referential filter
   becomes a proper named recursive type — is repointed at
   `#/definitions/<name>`. Schemas that already use `$defs`/`definitions`
   refs pass through untouched (verified against the existing snapshots).
   
   Such a filter schema now generates a recursive TypeScript union instead of
   erroring; regression test added.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 3 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_codegen v0.3.2, pctx_code_mode v0.4.2 ([`cee9b86`](https://github.com/portofcontext/pctx/commit/cee9b86add007cf68f0eaf06dece1f0ca00ece63))
    - Merge pull request #120 from portofcontext/fix/codegen-in-document-recursive-refs ([`123a8b0`](https://github.com/portofcontext/pctx/commit/123a8b0e418f7c1b4a97263f2e80bf2b39fb0b7f))
    - Resolve in-document pointer $refs during type generation ([`555ed2b`](https://github.com/portofcontext/pctx/commit/555ed2b069ee1bbf4154780216c77c84a41a3060))
</details>

## 0.3.1 (2026-07-16)

### Bug Fixes

 - <csr-id-06f33332aee2c753ee892e1e1e78138da9edfd37/> preserve recursive schema refs during type generation

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_codegen v0.3.1, pctx_registry v0.1.2, pctx_code_execution_runtime v0.2.1, pctx_code_mode v0.4.1 ([`998fa8c`](https://github.com/portofcontext/pctx/commit/998fa8c1b48fbcbba21b103dbb9db07d8b2a2187))
    - Merge branch 'main' into websocket-event-size ([`14210a5`](https://github.com/portofcontext/pctx/commit/14210a5d21dda490656804ac3c33107e05f5c08d))
    - Merge pull request #113 from lifeizhou-ap/fix/pctx-codegen-recursive-schema-refs ([`4f2973f`](https://github.com/portofcontext/pctx/commit/4f2973f95358491d1b3d9f60ef8ed170f4499329))
    - Additional tests, unify circluar ref tests, and changelog ([`6a9c8b6`](https://github.com/portofcontext/pctx/commit/6a9c8b651be4d9d561ed1707363cb6a0bd209816))
    - Preserve recursive schema refs during type generation ([`06f3333`](https://github.com/portofcontext/pctx/commit/06f33332aee2c753ee892e1e1e78138da9edfd37))
</details>

## 0.3.0 (2026-03-13)

### Other

 - <csr-id-12e0b624c08fb8a4cb8a87e73de6fb64ffb5a862/> unified registry
 - <csr-id-e274f784d1f7041b92b1e21d34dea248a7b47933/> optional namespace

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 16 commits contributed to the release.
 - 2 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Merge pull request #64 from portofcontext/ts-sidecar ([`aa7d8c8`](https://github.com/portofcontext/pctx/commit/aa7d8c8b1ef1ba2c6eac45810f3b3e9990252720))
    - Changelog + version bumps ([`a4b2bdf`](https://github.com/portofcontext/pctx/commit/a4b2bdf8d36bdd3cbee494565030e20ff07b1225))
    - Update dev command key & test snapshots ([`6181692`](https://github.com/portofcontext/pctx/commit/6181692f145878b69fcfadd8b47820fe7839cbf6))
    - Invoke map for better error msgs ([`5b3970c`](https://github.com/portofcontext/pctx/commit/5b3970c15c1c5900a7d6a1ca6c8bed339baa6f3c))
    - Support direct tool calls in sidecar style ([`8b5817c`](https://github.com/portofcontext/pctx/commit/8b5817c41d590ec02c32b305254b99c882b33e21))
    - Add upstream tools to list tool res depending on disclosure style ([`1b52b1f`](https://github.com/portofcontext/pctx/commit/1b52b1f65aab43f9985aeba68b85ceb05ea7213b))
    - TypescriptMode ([`887e5b3`](https://github.com/portofcontext/pctx/commit/887e5b3dcc406c946136a2251bed8062d8cc6234))
    - Unified registry ([`12e0b62`](https://github.com/portofcontext/pctx/commit/12e0b624c08fb8a4cb8a87e73de6fb64ffb5a862))
    - Optional namespace ([`e274f78`](https://github.com/portofcontext/pctx/commit/e274f784d1f7041b92b1e21d34dea248a7b47933))
    - Ignore namespace for execute_typescript_simple ([`d82fa4a`](https://github.com/portofcontext/pctx/commit/d82fa4ab96dbdeada6e61437b79077b556650804))
    - Execute_typescript_simple ([`61b5bf0`](https://github.com/portofcontext/pctx/commit/61b5bf0439415f35b3dd33dd597fb83ef698fb29))
    - Merge typescipt updates ([`797c1e5`](https://github.com/portofcontext/pctx/commit/797c1e5c02211ff88aca4d6ad82054d9974cee36))
    - Merge ([`c19b36c`](https://github.com/portofcontext/pctx/commit/c19b36cc9a6317879af5225204aeb0bcce35d7b4))
    - Async not needed for type check ([`50be02d`](https://github.com/portofcontext/pctx/commit/50be02df73c8f310289c044c85db416a922ab34d))
    - Types base on boolean ([`c237aa3`](https://github.com/portofcontext/pctx/commit/c237aa3a6c49ad898ba6133de5812e373eff1dc7))
    - Fix merge ([`ae69455`](https://github.com/portofcontext/pctx/commit/ae694559e756fa9aede6053b6d50a71ea5eeeef1))
</details>

## 0.2.0 (2026-02-04)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 8 commits contributed to the release.
 - 14 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_codegen v0.2.0, pctx_type_check_runtime v0.1.2, pctx_executor v0.1.2, pctx_code_mode v0.2.3 ([`dbe7858`](https://github.com/portofcontext/pctx/commit/dbe78587850bc31d42e24f8132069ce705313351))
    - Release pctx_codegen v0.2.0, pctx_type_check_runtime v0.1.2, pctx_executor v0.1.2, pctx_code_mode v0.2.3 ([`2b9e359`](https://github.com/portofcontext/pctx/commit/2b9e359c8366d1cf50fdf23b49287e77025ecf4e))
    - Release pctx_codegen v0.2.0, pctx_type_check_runtime v0.1.2, pctx_executor v0.1.2, pctx_code_mode v0.2.3 ([`ca2531d`](https://github.com/portofcontext/pctx/commit/ca2531dac6d202d926c55beec03c73496a75e056))
    - Release pctx_codegen v0.2.0, pctx_type_check_runtime v0.1.2, pctx_executor v0.1.2, pctx_code_mode v0.2.2 ([`8cd4cb3`](https://github.com/portofcontext/pctx/commit/8cd4cb3354416208f70927e8fdb6540162402eef))
    - Merge pull request #54 from portofcontext/empty-args ([`bf72e21`](https://github.com/portofcontext/pctx/commit/bf72e211d13336baf1f79d89e60c98ba6aa4bf7c))
    - Support for automatic default objects for "all optional" inputs ([`1df03d0`](https://github.com/portofcontext/pctx/commit/1df03d0b5996ec96b64bb3f0d64fc72ef3535108))
    - Upgrade codegen, tests, and dependants to support optional input schemas ([`11f9b86`](https://github.com/portofcontext/pctx/commit/11f9b86f1884a5adfcc27bbc13409eb4faf42467))
    - Tool tests & fixtures ([`f2c81ee`](https://github.com/portofcontext/pctx/commit/f2c81ee718d6786653b903fc44fb0ef15adb3789))
</details>

## 0.1.1 (2026-01-20)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release over the course of 1 calendar day.
 - 8 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_config v0.1.1, pctx_code_execution_runtime v0.1.1, pctx_codegen v0.1.1, pctx_deno_transpiler v0.1.1, pctx_type_check_runtime v0.1.1, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`f9f91b7`](https://github.com/portofcontext/pctx/commit/f9f91b787f18a7e0a33888013ba240b34ca7c5cf))
    - Adjusting changelogs prior to release of pctx_config v0.1.1, pctx_code_execution_runtime v0.1.1, pctx_codegen v0.1.1, pctx_deno_transpiler v0.1.1, pctx_type_check_runtime v0.1.1, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`1226141`](https://github.com/portofcontext/pctx/commit/1226141953fe727fc35c42ef50d1c95bd27037e0))
    - Merge pull request #51 from portofcontext/otlp-traceparent ([`1a0c13e`](https://github.com/portofcontext/pctx/commit/1a0c13e4c739285f278f887e0abd4fa2d1a43f08))
    - Consolidate deps to root cargo toml ([`4da94a7`](https://github.com/portofcontext/pctx/commit/4da94a7fb913fc9e4d6052277077eb6f7a87628a))
</details>

## 0.1.0 (2026-01-12)

### Added

- Initial release of pctx_codegen

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 5 commits contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_config v0.1.0, pctx_code_execution_runtime v0.1.0, pctx_codegen v0.1.0, deno_transpiler v0.1.0, pctx_type_check_runtime v0.1.0, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`821751c`](https://github.com/portofcontext/pctx/commit/821751c5ba7e28d2be1741454fcfe73fde3e8414))
    - Changelog init ([`899cdfc`](https://github.com/portofcontext/pctx/commit/899cdfcf69f9b404668188f768cc24ed853daf7d))
    - Adjusting changelogs prior to release of pctx_config v0.1.0, pctx_code_execution_runtime v0.1.0, pctx_codegen v0.1.0, deno_transpiler v0.1.0, pctx_type_check_runtime v0.1.0, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`44a3253`](https://github.com/portofcontext/pctx/commit/44a325347bad9a22a69e87691c583dfb3721ab39))
    - Init changelogs ([`60b8c14`](https://github.com/portofcontext/pctx/commit/60b8c14b41da72b74a843c1e4a20297ddc17f364))
    - Update crates setup for crates.io publishing ([`92502a4`](https://github.com/portofcontext/pctx/commit/92502a46c7b006023fb767796600cc0267fbf5e0))
</details>

