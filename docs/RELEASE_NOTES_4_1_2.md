# Mini Native 4.1.2 – guides and warnings / kézikönyvek és figyelmeztetések

## What changed / Változások

- Hungarian user guide: `docs/MINI_USER_GUIDE_HU.md` (31 chapters).
- English user guide: `docs/MINI_USER_GUIDE_EN.md` (31 chapters).
- `compile` test helper marked `#[cfg(test)]`.
- Unused Thumb backend function map removed.
- Unused conditional enum variants and ARM32 `tst` helper removed; Thumb test-only `bytes` helper annotated `#[cfg(test)]`.
- Redundant `clobbers` field removed from the high-level IR representation.
- Architecture-specific `check_asm` validators in `src/main.rs`, `src/backend_thumb.rs`, and `src/tiny8.rs` must continue validating instruction effects before IR lowering; **do not delete this validation**.

## Verification / Ellenőrzés

The current environment has no `cargo` or `rustc`: no Rust build or target-machine execution is claimed. Run `cargo test`, `cargo build --release`, and `bash tests/manuals_and_warnings412.sh` locally. Source-level checks are not a substitute for runtime and security verification.

## Known limits / Ismert korlátok

The language does not yet have assignment, general arrays, string types, full borrow checking, or a universal 8-bit/Thumb aggregate ABI. Target-dependent assembly is intentionally restricted. Refer to the two user guides.
