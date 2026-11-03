// SPDX-License-Identifier: CC0-1.0

//! Shared Core JSON-RPC wire serde helpers emitted into generated crates.
//!
//! Home for unit-explicit FeeRate modules so params and responses do not invent
//! per-file visitors. Amount fields use `bitcoin::amount::serde::as_btc`.

/// Core JSON-RPC fee-rate wire units (document next to mapping rules).
///
/// - **sat/vB**: param `feerate` / `fee_rate` (post-0.21 bumpfee etc.). Use
///   `bitcoin_units::fee_rate::serde::as_sat_per_vb_floor` (+ `::opt`).
/// - **BTC/kvB**: param `maxfeerate`; response rates such as `estimated_feerate`,
///   `relayfee`, `mempoolminfee`, `minrelaytxfee`, `incrementalrelayfee`,
///   `blockmintxfee`, estimatesmartfee `feerate`. Use [`AS_BTC_PER_KVB_MODULE`].
pub const FEE_RATE_WIRE_UNITS_DOC: &str = "\
FeeRate wire units (Core JSON-RPC):\n\
- sat/vB: param feerate / fee_rate → bitcoin_units::fee_rate::serde::as_sat_per_vb_floor\n\
- BTC/kvB: param maxfeerate; response estimated_feerate, relayfee, mempoolminfee,\n\
  minrelaytxfee, incrementalrelayfee, blockmintxfee, feerate → as_btc_per_kvb\n";

/// Source for `pub mod serde_fee_rate { pub mod as_btc_per_kvb { … } }` (+ `opt`).
///
/// Uses `bitcoin_units::FeeRate` so the same blob works in `types/mod.rs`,
/// `responses.rs`, and golden-decode (no `crate::types` required).
///
/// Params path: `crate::types::serde_fee_rate::as_btc_per_kvb`.
/// Responses path (module emitted in-file): `serde_fee_rate::as_btc_per_kvb`.
pub const AS_BTC_PER_KVB_MODULE: &str = r#"
/// FeeRate serde for Core wire units (BTC per kvB as JSON f64).
pub mod serde_fee_rate {
    /// Serialize/deserialize [`bitcoin_units::FeeRate`] as BTC per kvB (`f64`).
    pub mod as_btc_per_kvb {
        use bitcoin_units::FeeRate;
        use serde::{Deserialize, Deserializer, Serializer};

        pub fn serialize<S>(f: &FeeRate, s: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
        {
            s.serialize_f64((f.to_sat_per_kvb_floor() as f64) / 100_000_000.0)
        }

        pub fn deserialize<'d, D: Deserializer<'d>>(d: D) -> Result<FeeRate, D::Error> {
            let v = f64::deserialize(d)?;
            let sat_per_kvb = (v * 100_000_000.0).round().clamp(0.0, u32::MAX as f64) as u32;
            Ok(FeeRate::from_sat_per_kvb(sat_per_kvb))
        }

        /// Serialize/deserialize `Option<FeeRate>` as BTC per kvB (`f64`).
        pub mod opt {
            use bitcoin_units::FeeRate;
            use serde::{Deserialize, Deserializer, Serializer};

            #[allow(clippy::ref_option)]
            pub fn serialize<S>(f: &Option<FeeRate>, s: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                match f {
                    Some(r) => s.serialize_some(&((r.to_sat_per_kvb_floor() as f64) / 100_000_000.0)),
                    None => s.serialize_none(),
                }
            }

            pub fn deserialize<'d, D: Deserializer<'d>>(d: D) -> Result<Option<FeeRate>, D::Error> {
                let opt: Option<f64> = Option::deserialize(d)?;
                Ok(opt.map(|v| {
                    let sat_per_kvb = (v * 100_000_000.0).round().clamp(0.0, u32::MAX as f64) as u32;
                    FeeRate::from_sat_per_kvb(sat_per_kvb)
                }))
            }
        }
    }
}
"#;
