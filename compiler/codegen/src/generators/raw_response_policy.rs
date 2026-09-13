// SPDX-License-Identifier: CC0-1.0

//! Central inventory of **non-mechanical** Raw response codegen behavior.
//!
//! # Policy
//!
//! - **Do not add** new `(RPC, field) → Rust type` overrides here without:
//!   1. An **IR or OpenRPC change** (or documented proof the wire shape cannot be expressed yet), and
//!   2. A **serde decode golden** in the `compiler/codegen-golden-decode` workspace crate that exercises
//!      the affected response (fixtures under `resources/testdata/rpc_golden/`).
//!   3. A row in [`RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES`] for that RPC (CI enforces this via
//!      `ethos-codegen` and `ethos-codegen-golden-decode` tests).
//!   4. The same fixture path must appear in [`RPC_DECODE_GOLDEN_FIXTURES`] (superset of decode coverage).
//!
//! Overrides exist because the default `map_ir_type_to_rust` path would emit `serde_json::Value` or a
//! wrong container type for these fields; the long-term goal is to eliminate them via richer IR.

/// Serde decode goldens: every `(rpc, fixture)` exercised by `ethos-codegen-golden-decode` tests.
///
/// Paths are relative to `resources/testdata/rpc_golden/` at the workspace root. Keep sorted by `(0, 1)`.
/// The same RPC may appear more than once when multiple wire shapes matter (e.g. string vs object unions).
pub const RPC_DECODE_GOLDEN_FIXTURES: &[(&str, &str)] = &[
    ("analyzepsbt", "analyzepsbt_result_min.json"),
    ("decodepsbt", "decodepsbt_result_min.json"),
    ("decoderawtransaction", "decoderawtransaction_result_min.json"),
    ("estimatesmartfee", "estimatesmartfee_result_min.json"),
    ("finalizepsbt", "finalizepsbt_result_min.json"),
    ("getblockchaininfo", "getblockchaininfo_result_min.json"),
    ("getblocktemplate", "getblocktemplate_verbose_min.json"),
    ("getconnectioncount", "getconnectioncount_result_min.json"),
    ("getdifficulty", "getdifficulty_result_min.json"),
    ("getrawtransaction", "getrawtransaction_verbose_min.json"),
    ("listdescriptors", "listdescriptors_result_min.json"),
    ("validateaddress", "validateaddress_result_min.json"),
];

/// `(rpc_name, json_field_ident, rust_type_fragment)` — sorted by `(rpc, field)` for review diffs.
///
/// Keep sorted lexicographically by `(0, 1)`.
pub const RPC_FIELD_TYPE_OVERRIDES: &[(&str, &str, &str)] =
    &[("getblocktemplate", "transactions", "Vec<GetBlockTemplateTransaction>")];

/// One golden JSON fixture per RPC that appears in [`RPC_FIELD_TYPE_OVERRIDES`].
///
/// Paths are relative to `resources/testdata/rpc_golden/` at the workspace root. Keep sorted by `(0, 1)`.
/// `ethos-codegen-golden-decode` must decode each file into the generated response type (see that crate’s tests).
pub const RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES: &[(&str, &str)] =
    &[("getblocktemplate", "getblocktemplate_verbose_min.json")];

/// Field keys that Core may omit on some networks / versions; always emitted as `Option<…>`.
pub const FIELD_NAMES_ALWAYS_OPTIONAL: &[&str] = &[
    "blockmintxfee",
    "limitclustercount",
    "limitclustersize",
    "maxdatacarriersize",
    "permitbaremultisig",
];

/// Stronger Rust type for a specific response field, if allowlisted.
#[must_use]
pub fn rpc_field_type_rust_override(rpc_name: &str, field_name: &str) -> Option<&'static str> {
    RPC_FIELD_TYPE_OVERRIDES
        .iter()
        .find(|(r, f, _)| *r == rpc_name && *f == field_name)
        .map(|(_, _, ty)| *ty)
}

/// Whether this JSON object field key is allowlisted to be omitted on the wire for some networks/versions.
#[must_use]
pub fn field_name_always_optional_for_wire_absence(field_name: &str) -> bool {
    FIELD_NAMES_ALWAYS_OPTIONAL.contains(&field_name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use super::*;

    fn rpc_golden_dir() -> PathBuf {
        path::rpc_golden_dir(&path::workspace_root_from_manifest(env!("CARGO_MANIFEST_DIR"), 2))
    }

    #[test]
    fn rpc_field_override_golden_fixtures_sorted_and_unique_rpc() {
        for w in RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES.windows(2) {
            let (a_rpc, a_f) = w[0];
            let (b_rpc, b_f) = w[1];
            assert!(
                (a_rpc, a_f) < (b_rpc, b_f),
                "RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES must stay sorted by (rpc, file); found ({a_rpc},{a_f}) before ({b_rpc},{b_f})"
            );
        }
        let mut seen_rpc = BTreeSet::new();
        for (r, _) in RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES {
            assert!(
                seen_rpc.insert(*r),
                "duplicate golden entry for rpc {r}; use one row per RPC or extend the policy test"
            );
        }
    }

    #[test]
    fn rpc_decode_golden_fixtures_sorted() {
        for w in RPC_DECODE_GOLDEN_FIXTURES.windows(2) {
            let (a_rpc, a_f) = w[0];
            let (b_rpc, b_f) = w[1];
            assert!(
                (a_rpc, a_f) < (b_rpc, b_f),
                "RPC_DECODE_GOLDEN_FIXTURES must stay sorted by (rpc, file); found ({a_rpc},{a_f}) before ({b_rpc},{b_f})"
            );
        }
    }

    #[test]
    fn rpc_decode_golden_fixture_files_exist() {
        let dir = rpc_golden_dir();
        for (_, file) in RPC_DECODE_GOLDEN_FIXTURES {
            let path = dir.join(file);
            assert!(
                path.is_file(),
                "missing decode golden fixture {} (expected at {})",
                file,
                path.display()
            );
        }
    }

    #[test]
    fn rpc_golden_directory_matches_decode_inventory() {
        let dir = rpc_golden_dir();
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("read_dir {}: {e}", dir.display()))
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".json"))
            .collect();
        on_disk.sort();
        let mut from_policy: Vec<String> =
            RPC_DECODE_GOLDEN_FIXTURES.iter().map(|(_, f)| (*f).to_string()).collect();
        from_policy.sort();
        assert_eq!(
            on_disk, from_policy,
            "every *.json under rpc_golden must be listed exactly once in RPC_DECODE_GOLDEN_FIXTURES (and vice versa)"
        );
    }

    #[test]
    fn rpc_field_override_fixtures_are_decode_subset() {
        let decode: BTreeSet<_> =
            RPC_DECODE_GOLDEN_FIXTURES.iter().map(|(r, f)| (*r, *f)).collect();
        for (r, f) in RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES {
            assert!(
                decode.contains(&(r, f)),
                "RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES row ({r}, {f}) must also appear in RPC_DECODE_GOLDEN_FIXTURES"
            );
        }
    }

    #[test]
    fn rpc_field_override_golden_fixtures_cover_every_override_rpc() {
        let override_rpcs: BTreeSet<_> =
            RPC_FIELD_TYPE_OVERRIDES.iter().map(|(r, _, _)| *r).collect();
        let golden_rpcs: BTreeSet<_> =
            RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES.iter().map(|(r, _)| *r).collect();
        assert_eq!(
            override_rpcs, golden_rpcs,
            "every RPC in RPC_FIELD_TYPE_OVERRIDES must have exactly one row in RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES (and vice versa)"
        );
    }

    #[test]
    fn rpc_field_override_golden_fixture_files_exist() {
        let dir = rpc_golden_dir();
        for (_, file) in RPC_FIELD_OVERRIDE_GOLDEN_FIXTURES {
            let path = dir.join(file);
            assert!(
                path.is_file(),
                "missing golden fixture {} (expected at {})",
                file,
                path.display()
            );
        }
    }

    #[test]
    fn rpc_field_overrides_sorted_and_unique() {
        for w in RPC_FIELD_TYPE_OVERRIDES.windows(2) {
            let (a_rpc, a_f, _) = w[0];
            let (b_rpc, b_f, _) = w[1];
            assert!(
                (a_rpc, a_f) < (b_rpc, b_f),
                "RPC_FIELD_TYPE_OVERRIDES must stay sorted by (rpc, field); found ({a_rpc},{a_f}) before ({b_rpc},{b_f})"
            );
        }
        let mut seen = std::collections::BTreeSet::new();
        for (r, f, _) in RPC_FIELD_TYPE_OVERRIDES {
            assert!(seen.insert((*r, *f)), "duplicate override ({r}, {f})");
        }
    }

    #[test]
    fn optional_field_names_sorted_and_unique() {
        for w in FIELD_NAMES_ALWAYS_OPTIONAL.windows(2) {
            assert!(w[0] < w[1], "FIELD_NAMES_ALWAYS_OPTIONAL must be sorted");
        }
        let mut seen = std::collections::BTreeSet::new();
        for k in FIELD_NAMES_ALWAYS_OPTIONAL {
            assert!(seen.insert(*k), "duplicate field key {k}");
        }
    }
}
