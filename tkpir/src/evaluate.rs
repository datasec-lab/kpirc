//! Homomorphic KPIR-C Evaluate helpers for multi-query Sum and Threshold-of-Sum.
//!
//! Response stays as shortint limbs; Evaluate packs the **full** payload into the
//! smallest TFHE integer that fits:
//! - ≤ 8 B   → `FheUint64`
//! - ≤ 16 B  → `FheUint128`
//! - ≤ 32 B  → `FheUint256`
//! - ≤ 64 B  → `FheUint512`
//! - ≤ 128 B → `FheUint1024`
//!
//! Larger entries are rejected. After Evaluate, results are unpacked to shortints
//! and `compress_shortint`'d for download.
//!
//! Before Evaluate, each Response is presence-gated with a default.
//! Both schemes use the same boolean mux ([`apply_default_mux`]):
//! TCWKPIR turns the accumulated 0/1 selection bit into an `FheBool`
//! ([`selection_to_fhebool`]); TCA* uses digest `eq`.
//! For aggregation we use `0` on miss (same idea as `select(..., Some(0))`),
//! not `DEFAULT = u64::MAX`.

use crate::utils::tfhe_utils::{
    compress_shortint, shortint_ciphertexts_to_fheuint1024, shortint_ciphertexts_to_fheuint128,
    shortint_ciphertexts_to_fheuint256, shortint_ciphertexts_to_fheuint512,
    shortint_ciphertexts_to_fheuint64,
};
use tfhe::integer::bigint::U1024;
use tfhe::integer::{IntegerCiphertext, IntegerRadixCiphertext, RadixCiphertext, U256, U512};
use tfhe::prelude::*;
use tfhe::shortint::ciphertext::Ciphertext as ShortintCiphertext;
use tfhe::shortint::ciphertext::CompressedCiphertextList as ShortintCompressedCiphertextList;
use tfhe::shortint::list_compression::CompressionKey as ShortintCompressionKey;
use tfhe::shortint::ClassicPBSParameters;
use tfhe::{FheBool, FheUint1024, FheUint128, FheUint2, FheUint256, FheUint512, FheUint64};

/// Default plaintext substituted on keyword miss before Evaluate.
pub const EVALUATE_DEFAULT: u64 = 0;

/// Integer width chosen from plaintext payload bit length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EvaluateWidth {
    U64,
    U128,
    U256,
    U512,
    U1024,
}

impl EvaluateWidth {
    pub fn bits(self) -> u64 {
        match self {
            EvaluateWidth::U64 => 64,
            EvaluateWidth::U128 => 128,
            EvaluateWidth::U256 => 256,
            EvaluateWidth::U512 => 512,
            EvaluateWidth::U1024 => 1024,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            EvaluateWidth::U64 => "FheUint64",
            EvaluateWidth::U128 => "FheUint128",
            EvaluateWidth::U256 => "FheUint256",
            EvaluateWidth::U512 => "FheUint512",
            EvaluateWidth::U1024 => "FheUint1024",
        }
    }
}

/// Pick Evaluate packing width from entry payload bit length (full entry, not digest).
pub fn choose_evaluate_width(payload_bits: u64) -> EvaluateWidth {
    assert!(payload_bits > 0, "payload_bits must be > 0");
    if payload_bits <= 64 {
        EvaluateWidth::U64
    } else if payload_bits <= 128 {
        EvaluateWidth::U128
    } else if payload_bits <= 256 {
        EvaluateWidth::U256
    } else if payload_bits <= 512 {
        EvaluateWidth::U512
    } else if payload_bits <= 1024 {
        EvaluateWidth::U1024
    } else {
        panic!(
            "payload_bits={} ({} B) exceeds FheUint1024 (128 B); raise Evaluate max width",
            payload_bits,
            payload_bits / 8
        );
    }
}

/// Packed Response payload / Evaluate accumulator at the chosen width.
#[derive(Clone)]
pub enum PackedValue {
    U64(FheUint64),
    U128(FheUint128),
    U256(FheUint256),
    U512(FheUint512),
    U1024(FheUint1024),
}

impl PackedValue {
    pub fn width(&self) -> EvaluateWidth {
        match self {
            PackedValue::U64(_) => EvaluateWidth::U64,
            PackedValue::U128(_) => EvaluateWidth::U128,
            PackedValue::U256(_) => EvaluateWidth::U256,
            PackedValue::U512(_) => EvaluateWidth::U512,
            PackedValue::U1024(_) => EvaluateWidth::U1024,
        }
    }

    /// Trivial encryption of a clear `u64` at the given Evaluate width (escape hatch).
    pub fn encrypt_trivial_u64(width: EvaluateWidth, value: u64) -> PackedValue {
        match width {
            EvaluateWidth::U64 => PackedValue::U64(FheUint64::encrypt_trivial(value)),
            EvaluateWidth::U128 => PackedValue::U128(FheUint128::encrypt_trivial(value as u128)),
            EvaluateWidth::U256 => {
                PackedValue::U256(FheUint256::encrypt_trivial(U256::from(value)))
            }
            EvaluateWidth::U512 => {
                PackedValue::U512(FheUint512::encrypt_trivial(U512::from(value)))
            }
            EvaluateWidth::U1024 => {
                PackedValue::U1024(FheUint1024::encrypt_trivial(U1024::from(value)))
            }
        }
    }

    /// Real client-key encryption of a clear `u64` at the given Evaluate width.
    pub fn encrypt_u64(
        ck: &tfhe::ClientKey,
        width: EvaluateWidth,
        value: u64,
    ) -> PackedValue {
        match width {
            EvaluateWidth::U64 => PackedValue::U64(FheUint64::encrypt(value, ck)),
            EvaluateWidth::U128 => PackedValue::U128(FheUint128::encrypt(value as u128, ck)),
            EvaluateWidth::U256 => {
                PackedValue::U256(FheUint256::encrypt(U256::from(value), ck))
            }
            EvaluateWidth::U512 => {
                PackedValue::U512(FheUint512::encrypt(U512::from(value), ck))
            }
            EvaluateWidth::U1024 => {
                PackedValue::U1024(FheUint1024::encrypt(U1024::from(value), ck))
            }
        }
    }

    fn into_blocks(self) -> Vec<ShortintCiphertext> {
        match self {
            PackedValue::U64(ct) => {
                let (radix, _, _) = ct.into_raw_parts();
                radix.into_blocks()
            }
            PackedValue::U128(ct) => {
                let (radix, _, _) = ct.into_raw_parts();
                radix.into_blocks()
            }
            PackedValue::U256(ct) => {
                let (radix, _, _) = ct.into_raw_parts();
                radix.into_blocks()
            }
            PackedValue::U512(ct) => {
                let (radix, _, _) = ct.into_raw_parts();
                radix.into_blocks()
            }
            PackedValue::U1024(ct) => {
                let (radix, _, _) = ct.into_raw_parts();
                radix.into_blocks()
            }
        }
    }
}

/// Pack **all** payload limbs into the integer type selected by `width`.
pub fn pack_payload(
    params: &ClassicPBSParameters,
    limbs: &[ShortintCiphertext],
    digest_prefix_len: usize,
    width: EvaluateWidth,
) -> PackedValue {
    assert!(limbs.len() >= digest_prefix_len);
    let payload = &limbs[digest_prefix_len..];
    match width {
        EvaluateWidth::U64 => {
            PackedValue::U64(shortint_ciphertexts_to_fheuint64(params, payload))
        }
        EvaluateWidth::U128 => {
            PackedValue::U128(shortint_ciphertexts_to_fheuint128(params, payload))
        }
        EvaluateWidth::U256 => {
            PackedValue::U256(shortint_ciphertexts_to_fheuint256(params, payload))
        }
        EvaluateWidth::U512 => {
            PackedValue::U512(shortint_ciphertexts_to_fheuint512(params, payload))
        }
        EvaluateWidth::U1024 => {
            PackedValue::U1024(shortint_ciphertexts_to_fheuint1024(params, payload))
        }
    }
}

/// Turn the accumulated 0/1 selection shortint into an `FheBool`.
///
/// Uses a 2-bit `ne(0)` rather than widening to `FheUint64` and doing
/// integer mul. The following mux is then the same [`apply_default_mux`]
/// as TCA.
pub fn selection_to_fhebool(sel: ShortintCiphertext) -> FheBool {
    let val = FheUint2::try_from(RadixCiphertext::from_blocks(vec![sel]))
        .expect("selection bit must be a single shortint block");
    val.ne(0u8)
}

/// Presence mux: `present ? payload : default`.
pub fn apply_default_mux(
    present: &tfhe::FheBool,
    payload: &PackedValue,
    default: u64,
) -> PackedValue {
    match payload {
        PackedValue::U64(p) => {
            PackedValue::U64(present.select(p, &FheUint64::encrypt_trivial(default)))
        }
        PackedValue::U128(p) => PackedValue::U128(
            present.select(p, &FheUint128::encrypt_trivial(default as u128)),
        ),
        PackedValue::U256(p) => PackedValue::U256(
            present.select(p, &FheUint256::encrypt_trivial(U256::from(default))),
        ),
        PackedValue::U512(p) => PackedValue::U512(
            present.select(p, &FheUint512::encrypt_trivial(U512::from(default))),
        ),
        PackedValue::U1024(p) => PackedValue::U1024(
            present.select(p, &FheUint1024::encrypt_trivial(U1024::from(default))),
        ),
    }
}

macro_rules! sum_arm {
    ($responses:expr, $variant:ident) => {{
        let mut acc = match &$responses[0] {
            PackedValue::$variant(v) => v.clone(),
            _ => unreachable!(),
        };
        for r in $responses.iter().skip(1) {
            if let PackedValue::$variant(v) = r {
                acc = acc + v;
            }
        }
        PackedValue::$variant(acc)
    }};
}

/// Homomorphic sum of packed Response values (all must share the same width).
pub fn evaluate_sum(responses: &[PackedValue]) -> PackedValue {
    assert!(!responses.is_empty());
    let width = responses[0].width();
    assert!(
        responses.iter().all(|r| r.width() == width),
        "mixed Evaluate widths in sum"
    );

    match width {
        EvaluateWidth::U64 => sum_arm!(responses, U64),
        EvaluateWidth::U128 => sum_arm!(responses, U128),
        EvaluateWidth::U256 => sum_arm!(responses, U256),
        EvaluateWidth::U512 => sum_arm!(responses, U512),
        EvaluateWidth::U1024 => sum_arm!(responses, U1024),
    }
}

/// Homomorphic threshold-of-sum: `1[sum >= threshold]` (bit in low limb).
pub fn evaluate_threshold_of_sum(responses: &[PackedValue], threshold: u64) -> PackedValue {
    let sum = evaluate_sum(responses);
    match sum {
        PackedValue::U64(s) => {
            let ge = s.ge(threshold);
            PackedValue::U64(ge.select(
                &FheUint64::encrypt_trivial(1u64),
                &FheUint64::encrypt_trivial(0u64),
            ))
        }
        PackedValue::U128(s) => {
            let ge = s.ge(threshold as u128);
            PackedValue::U128(ge.select(
                &FheUint128::encrypt_trivial(1u128),
                &FheUint128::encrypt_trivial(0u128),
            ))
        }
        PackedValue::U256(s) => {
            let ge = s.ge(U256::from(threshold));
            PackedValue::U256(ge.select(
                &FheUint256::encrypt_trivial(U256::from(1u64)),
                &FheUint256::encrypt_trivial(U256::from(0u64)),
            ))
        }
        PackedValue::U512(s) => {
            let ge = s.ge(U512::from(threshold));
            PackedValue::U512(ge.select(
                &FheUint512::encrypt_trivial(U512::from(1u64)),
                &FheUint512::encrypt_trivial(U512::from(0u64)),
            ))
        }
        PackedValue::U1024(s) => {
            let ge = s.ge(U1024::from(threshold));
            PackedValue::U1024(ge.select(
                &FheUint1024::encrypt_trivial(U1024::from(1u64)),
                &FheUint1024::encrypt_trivial(U1024::from(0u64)),
            ))
        }
    }
}

/// Unpack aggregate and compress **all** shortint limbs for download.
pub fn prepare_download_sum(
    comp_key: &ShortintCompressionKey,
    aggregate: &PackedValue,
) -> ShortintCompressedCiphertextList {
    let blocks = aggregate.clone().into_blocks();
    compress_shortint(comp_key, &blocks)
}

/// Unpack threshold result and compress **only the first** shortint (encrypted bit).
pub fn prepare_download_threshold(
    comp_key: &ShortintCompressionKey,
    bit_ct: &PackedValue,
) -> ShortintCompressedCiphertextList {
    let blocks = bit_ct.clone().into_blocks();
    assert!(!blocks.is_empty());
    compress_shortint(comp_key, &blocks[..1])
}
