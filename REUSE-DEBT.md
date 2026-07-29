# REUSE debt — py2rust (P24f bootstrap)

**Status:** Compliant (`reuse lint` clean) as of 2026-07-29.

Coverage is via aggregate annotations in `REUSE.toml` (including `crates/**`).
File-level SPDX headers may be added incrementally; do not mass-rewrite.

Copyright: 2026 Tyler Zervas — SPDX-License-Identifier: MIT

## History

- P24f bootstrap covered top-level paths; Rust workspace under `crates/` was
  still unannotated until `crates/**` was added (closes fleet-gap #13 residual
  after Jules #36).
