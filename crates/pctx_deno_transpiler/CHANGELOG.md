# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 (2026-10-05)

### Chore

 - <csr-id-88fd00f339e10f30f0cad212c148aa885bfd7e16/> bump deno stack to deno_core 0.412 / deno_ast 0.53
   Moves to v8 150 and temporal_rs 0.2 so downstream workspaces can use
   icu_calendar 2.3. op2(async) becomes op2 (async ops are eager by default)
   and PollEventLoopOptions::pump_v8_message_loop is gone.

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 1 commit was understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Merge pull request #167 from BezotCorp/modernize-deno-stack ([`4603698`](https://github.com/portofcontext/pctx/commit/460369836fc592f4dfd2b361dfdd6a7e81bd6046))
    - Bump deno stack to deno_core 0.412 / deno_ast 0.53 ([`88fd00f`](https://github.com/portofcontext/pctx/commit/88fd00f339e10f30f0cad212c148aa885bfd7e16))
</details>

## 0.1.1 (2026-01-20)

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 4 commits contributed to the release.
 - 8 days passed between releases.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_config v0.1.1, pctx_code_execution_runtime v0.1.1, pctx_codegen v0.1.1, pctx_deno_transpiler v0.1.1, pctx_type_check_runtime v0.1.1, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`f9f91b7`](https://github.com/portofcontext/pctx/commit/f9f91b787f18a7e0a33888013ba240b34ca7c5cf))
    - Adjusting changelogs prior to release of pctx_config v0.1.1, pctx_code_execution_runtime v0.1.1, pctx_codegen v0.1.1, pctx_deno_transpiler v0.1.1, pctx_type_check_runtime v0.1.1, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`1226141`](https://github.com/portofcontext/pctx/commit/1226141953fe727fc35c42ef50d1c95bd27037e0))
    - Merge pull request #52 from portofcontext/bump-deps-for-goose ([`8a25938`](https://github.com/portofcontext/pctx/commit/8a259388b64d111a671cf930cdd6294449b70d8b))
    - Bump deps and export needed code mode structs and funcs ([`ddf9f35`](https://github.com/portofcontext/pctx/commit/ddf9f35cfb9d9bc760b8e31d02eb332cbc04b1ed))
</details>

## 0.1.0 (2026-01-12)

### Added

- Initial release of pctx_deno_transpiler

### Commit Statistics

<csr-read-only-do-not-edit/>

 - 2 commits contributed to the release.
 - 0 commits were understood as [conventional](https://www.conventionalcommits.org).
 - 0 issues like '(#ID)' were seen in commit messages

### Commit Details

<csr-read-only-do-not-edit/>

<details><summary>view details</summary>

 * **Uncategorized**
    - Release pctx_deno_transpiler v0.1.0, pctx_type_check_runtime v0.1.0, pctx_executor v0.1.0, pctx_code_mode v0.1.0 ([`6abd5b1`](https://github.com/portofcontext/pctx/commit/6abd5b160dd1895e88a216360b1ff07e355f819e))
    - Deno_transpile package rename, taken on crates.io ([`4da2765`](https://github.com/portofcontext/pctx/commit/4da27656112812eb787349dbe1adced502da64fe))
</details>

