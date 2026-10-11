# AGENTS.md

Project instructions for AI-assisted work in `org-defuddle`.

This file is self-contained. It imports the applicable rules from
`~/repos/coding-guidelines` and adapts them to this repository.

## Project Goal

`org-defuddle` aims to reimplement the functionality of
`kepano/defuddle` in Rust and expose it to Emacs through a dynamic
module, with an Elisp frontend that makes extraction into Org convenient.

Do not redefine the goal around the current partial implementation. Every
change should move the project closer to feature parity with defuddle's
core behavior: metadata extraction, schema.org fallback, DOM cleanup,
content scoring, media handling, URL normalization, site-specific
extractors, and high-quality Org output.

## Architecture Boundaries

- `crates/org-defuddle-core` owns extraction logic: HTML parsing,
  metadata, cleanup, scoring, URL normalization, and HTML-to-Org
  rendering.
- `crates/org-defuddle-module` is only an Emacs dynamic module boundary.
  Keep it thin: convert Emacs inputs to Rust inputs, call the core, and
  convert the result back.
- `crates/org-defuddle-cli` owns command-line input/output, including URL,
  file, and stdin sources. Keep extraction policy in the core and fetch/output
  boundary behavior in the CLI.
- `org-defuddle.el` owns Emacs UX only: module loading, interactive
  commands, URL retrieval, buffer insertion, and user-facing errors.
- Do not add Node/TypeScript runtime dependencies. The original defuddle
  repository is a reference implementation and fixture source, not a
  runtime dependency.
- Do not split files just to make them smaller. Split only around stable
  responsibilities with clear ownership.

## General Engineering Rules

- Prefer boring, direct code over clever abstractions.
- Add an abstraction only when it removes real duplication, centralizes a
  rule, or makes callers simpler.
- Delete unused code instead of leaving compatibility shims or "removed"
  comments.
- Keep experiments narrow. Prove one feature slice with tests before
  widening it.
- Find the root cause before changing behavior. After two failed fixes on
  the same issue, stop patching and gather more evidence.
- Errors should surface at the owning boundary. Do not silently swallow
  internal failures and return plausible defaults.
- Avoid broad fallback paths that hide broken invariants.
- Do not transform structured formats with brittle string insertion when
  parser-backed or DOM-aware handling is available.

## Testing and Fixtures

- Treat upstream defuddle fixtures as the primary behavioral reference.
  Use `ORG_DEFUDDLE_DEFUDDLE_DIR` to point tests at a defuddle checkout;
  `/private/tmp/defuddle-elisp-source` may be used as a local default.
  Check out the commit pinned in `tests/upstream-revision`, which is also read
  by the release workflow. When updating it, run the independent exact Org
  differential before regenerating snapshots.
- Prefer tests that drive public paths: Rust core API, dynamic module
  calls, and Elisp wrapper calls.
- Tests must assert specific metadata and content. A test that would still
  pass if extraction returned unrelated article text is not useful.
- When changing behavior already covered by tests, update those tests in
  the same change.
- For user-visible bugs or parity gaps, write or strengthen a failing test
  first when practical, then fix the code.
- Keep tests focused. Do not lock in incidental formatting unless the
  formatting is part of the intended Org output contract.

Required verification before considering a code change ready:

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo build --release -p org-defuddle-module
cargo build --release -p org-defuddle-cli
emacs --batch -Q --eval '(byte-compile-file "org-defuddle.el")'
emacs --batch -Q -L . -L test -l test/org-defuddle-test.el -f ert-run-tests-batch-and-exit
```

For changes to module loading or Elisp wrapper behavior, also run an
actual Emacs wrapper call through `org-defuddle-html-to-org`.

## Rust Rules

- Keep the Rust core free of Emacs-specific concepts.
- Keep the dynamic module crate free of extraction policy.
- Prefer typed data structures at API boundaries. Serialize only at
  external boundaries such as the Emacs module JSON function.
- Keep side effects at the edges. Parsing and rendering functions should
  take inputs and return explicit results.
- Do not catch or suppress errors inside core logic unless the operation is
  explicitly best-effort and the caller can still distinguish that outcome.
- Use dependency public APIs only. Do not rely on private internals of
  crates such as `kuchiki`.
- When adding dependencies or raising compiler/runtime baselines, update
  `Cargo.toml`, `README.org`, and this file if the workflow changes.

## Emacs Lisp Rules

- Loading `org-defuddle.el` must not change active editing behavior.
  Activation must be explicit through commands or function calls.
- Public Elisp symbols use the `org-defuddle-` prefix. Private symbols use
  `org-defuddle--`.
- Do not call another package's double-dash private symbols.
- Use `user-error` for user-caused problems and `error` for programmer
  bugs.
- Interactive commands should be thin wrappers: validate input, call the
  internal function, then present results.
- Use `defcustom` for user-configurable values with precise `:type` and
  `:group`.
- Add `;;;###autoload` to user-facing interactive commands.
- Keep byte-compilation honest with `declare-function`/`defvar` only at
  real boundaries; do not use declarations to patch poor ownership.
- Elisp package files must use lexical binding, provide the feature, end
  with the standard footer, and byte-compile with zero warnings.
- For distributable package changes, also run `checkdoc`; public
  functions, variables, and customization options need useful docstrings.

## Documentation Rules

- User-visible changes must update `README.org` in the same change.
- Code is the source of truth. If docs and code diverge, fix the docs.
- Optimize docs for rendered reading, not source-width wrapping.
- When deliberately deferring a known defuddle parity gap, document it in
  `README.org` or a future decision/postmortem note.

## Pre-Commit Discipline

- Read the full diff before committing or declaring work ready.
- Compile clean: no compiler, byte-compiler, or linter warnings in the
  touched surface.
- Run the full relevant test suite, not just the nearest unit test.
- Remove duplicated logic or dead code introduced by the change.
- If a feature is intentionally partial, state the limitation explicitly
  instead of leaving a silent "good enough" behavior.
