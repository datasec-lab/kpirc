use std::{
    collections::HashMap,
    default,
    fmt::{Debug, Display},
    ops::{BitAndAssign, Shl},
};

use crate::{
    byte_fuse_filter::Arity3ByteFuseFilter,
    common::*,
    utils::{cwutils::*, from_decomposition, partition_u8, tfhe_utils::*},
};
use blake2::{Blake2s256, Digest};
use bytebuffer::ByteBuffer;
use num_traits::{FromPrimitive, ToPrimitive, Unsigned};
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
use rayon::prelude::*;
use tfhe::{
    boolean::prelude::LweDimension,
    core_crypto::prelude::{lwe_ciphertext_add_assign, lwe_ciphertext_cleartext_mul, Cleartext},
    prelude::*,
    shortint::{
        parameters::{Degree, NoiseLevel},
        ClassicPBSParameters,
    },
    FheUint64, FheUint8,
};

use tfhe::shortint::ciphertext::Ciphertext as ShortintCiphertext;
use tfhe::shortint::ServerKey as ShortintServerKey;

pub struct Database<'a, T> {
    // Database parameters
    pub db_params: &'a DatabaseParameters,

    // Keywords
    keywrords: Vec<u64>,
    encoded_keywords: Vec<Vec<u8>>,

    // Data vectors
    data: Vec<Vec<T>>,
    fused_data: Vec<Vec<u8>>,
}

impl<'a, T> Database<'a, T>
where
    T: Unsigned
        + Copy
        + ToPrimitive
        + FromPrimitive
        + Display
        + BitAndAssign
        + Shl<u64, Output = T>
        + Debug,
    Standard: Distribution<T>,
{
    fn generate_data(&self) -> Vec<T> {
        let data_bitlen = self.db_params.max_data_bitlen;
        let partition_bitlen = self.db_params.partition_bitlen;
        let num_partitions = self.db_params.num_partitions();
        let mut rng = rand::thread_rng();
        let mut data = vec![T::zero(); num_partitions];

        let radix = 1 << partition_bitlen;
        // `u64::MAX` is reserved for the default value
        loop {
            for i in 0..num_partitions {
                let mut rand_val = rng.gen::<T>();
                let mask = if i == num_partitions - 1 && data_bitlen % partition_bitlen > 0 {
                    (T::one() << (data_bitlen % partition_bitlen)) - T::one()
                } else {
                    (T::one() << partition_bitlen) - T::one()
                };
                rand_val &= mask;
                data[i] = rand_val;
            }

            let decomposed_value = from_decomposition(
                radix,
                &data
                    .iter()
                    .map(|&x| x.to_u8().unwrap())
                    .collect::<Vec<u8>>(),
            );

            if decomposed_value != u64::MAX {
                break;
            }
        }

        data
    }

    fn generate_rows(&mut self) {
        let max_keyword_bitlen = self.db_params.max_keyword_bitlen;
        let num_keywords = self.db_params.num_keywords;

        let step = 1 << (max_keyword_bitlen - (num_keywords as f64).log2().ceil() as u64);
        for i in 0..num_keywords {
            let new_keyword = i * step;
            self.keywrords.push(new_keyword as u64);
            self.data.push(self.generate_data());
        }
    }

    fn new(db_params: &'a DatabaseParameters) -> Self {
        let mut db = Self {
            db_params,
            keywrords: Vec::new(),
            encoded_keywords: Vec::new(),
            data: Vec::new(),
            fused_data: Vec::new(),
        };

        db.generate_rows();

        db
    }

    pub fn get_data_by_keyword(&self, keyword: u64) -> Option<&Vec<T>> {
        match self.keywrords.iter().position(|&x| x == keyword) {
            Some(index) => Some(&self.data[index]),
            None => None,
        }
    }

    pub fn get_encoded_kws(&self) -> &Vec<Vec<u8>> {
        &self.encoded_keywords
    }

    pub fn get_data(&self) -> &Vec<Vec<T>> {
        &self.data
    }

    pub fn get_data_mut(&mut self) -> &mut Vec<Vec<T>> {
        &mut self.data
    }

    pub fn get_keywords(&self) -> &Vec<u64> {
        &self.keywrords
    }

    pub fn get_fused_data(&self) -> &Vec<Vec<u8>> {
        &self.fused_data
    }

    pub fn get_index_by_keyword(&self, keyword: u64) -> Option<usize> {
        self.keywrords.iter().position(|&x| x == keyword)
    }

    pub fn print_db(&self) {
        for (kw, data) in self.keywrords.iter().zip(self.data.iter()) {
            println!("Keyword: {:?}, Data: {:?}", kw, data);
        }
    }

    /// Rebuild the Byte Fuse Filter storage from current keyword/payload rows.
    pub fn build_byte_fuse_filter(&mut self, bff_params: &mut ByteFuseFilterParameters)
    where
        T: ToPrimitive,
    {
        let mut hasher = Blake2s256::new();
        let mut key_value_map: HashMap<ByteBuffer, Vec<u8>> = HashMap::new();

        for (kw, val) in self.keywrords.iter().zip(self.data.iter()) {
            let key_buffer: ByteBuffer = ByteBuffer::from_vec(kw.to_be_bytes().to_vec());
            hasher.update(key_buffer.as_bytes());
            let digest: Vec<u8> = hasher.finalize_reset()[0..bff_params.digest_byte_len]
                .try_into()
                .unwrap();

            let mut digest_partitioned = Vec::new();
            digest.iter().for_each(|&d| {
                digest_partitioned.extend_from_slice(&partition_u8(
                    d,
                    8usize,
                    self.db_params.partition_bitlen as usize,
                ))
            });

            let mut val_u8 = Vec::new();
            for v in val {
                val_u8.push(
                    v.to_u8()
                        .expect("Conversion failed:: value out of range for u8"),
                );
            }
            digest_partitioned.extend(&val_u8);

            key_value_map.insert(key_buffer, digest_partitioned);
        }

        let val_byte_len = self.db_params.num_partitions();
        let fuse_filter =
            Arity3ByteFuseFilter::new(key_value_map, bff_params.key_byte_len() + val_byte_len);
        bff_params.filter_seed = fuse_filter.seed();
        bff_params.filter_len = fuse_filter.filter_len();
        self.fused_data = fuse_filter.storage();
    }
}

/// Preprocesses the database.
pub fn prep_db<'a, T>(
    db_params: &'a DatabaseParameters,
    cw_params: Option<&ConstantWeightParameters>,
    bff_params: Option<&mut ByteFuseFilterParameters>,
) -> Database<'a, T>
where
    T: Unsigned
        + Copy
        + ToPrimitive
        + FromPrimitive
        + Display
        + BitAndAssign
        + Shl<u64, Output = T>
        + Debug,
    Standard: Distribution<T>,
{
    let mut db = Database::<T>::new(db_params);

    match cw_params {
        Some(cw_params) => {
            for kw in &db.keywrords {
                db.encoded_keywords
                    .push(get_perfect_constant_weight_codeword(
                        *kw,
                        cw_params.codeword_bitlen,
                        cw_params.hamming_weight,
                    ));
            }
        }
        None => {}
    }

    match bff_params {
        Some(bff_params) => {
            db.build_byte_fuse_filter(bff_params);
        }
        None => {}
    }

    db
}

pub fn check_equality(
    sks: &ShortintServerKey,
    ct_left: &[ShortintCiphertext],
    ct_right: &[ShortintCiphertext],
) -> ShortintCiphertext {
    let log_p = sks.message_modulus.0.ilog2();

    let mut results: Vec<ShortintCiphertext> = ct_left
        .iter()
        .zip(ct_right.iter())
        .map(|(a, b)| sks.bitxor(a, b))
        .collect();

    while results.len() > 1 {
        let mut next = Vec::new();
        for chunk in results.chunks(2) {
            let e = if chunk.len() == 2 {
                sks.bitor(&chunk[0], &chunk[1])
            } else {
                chunk[0].clone()
            };
            next.push(e);
        }
        results = next;
    }

    let mut e = results.pop().unwrap();

    for i in 0..log_p - 1 {
        let shifted_e = sks.scalar_right_shift(&e, 1);
        sks.bitor_assign(&mut e, &shifted_e);
    }

    sks.scalar_bitand_assign(&mut e, 1);
    sks.scalar_bitxor_assign(&mut e, 1);

    e
}

/// Computes the constant weight equality for the given keyword.
fn constant_weight_eq(query: &[FheUint8], encoded_kw: &[u8]) -> FheUint8 {
    let mut e = None;

    for idx in 0..query.len() {
        if encoded_kw[idx] == 1 {
            e = match e {
                None => Some(query[idx].clone()),
                Some(acc) => Some(acc & &query[idx]),
            };
        }
    }

    e.unwrap_or(FheUint8::encrypt_trivial(0u8))
}

fn constant_weight_eq_shorint(
    sks: &ShortintServerKey,
    query: &[ShortintCiphertext],
    encoded_kw: &[u8],
) -> ShortintCiphertext {
    let mut e = None;

    for idx in 0..query.len() {
        if encoded_kw[idx] == 1 {
            e = match e {
                None => Some(query[idx].clone()),
                Some(acc) => Some(sks.mul(&acc, &query[idx])),
            };
        }
    }

    e.unwrap_or(sks.create_trivial(0u64))
}

/// Generates the selection vector for the given query.
pub fn generate_selection_vector(db: &Database<u64>, query: &[FheUint8]) -> Vec<FheUint64> {
    let mut selection_vector: Vec<FheUint64> = Vec::new();

    for idx in 0..db.db_params.num_keywords as usize {
        selection_vector.push(constant_weight_eq(query, &db.encoded_keywords[idx]).cast_into());
    }

    selection_vector
}

pub fn generate_selection_vector_shortint(
    sks: &ShortintServerKey,
    db: &Database<u8>,
    query: &[ShortintCiphertext],
) -> Vec<ShortintCiphertext> {
    (0..db.db_params.num_keywords as usize)
        .into_par_iter()
        .map(|idx| constant_weight_eq_shorint(sks, query, &db.encoded_keywords[idx]))
        .collect()
}

pub fn accumulate_selection(
    sks: &ShortintServerKey,
    sv: &[ShortintCiphertext],
) -> ShortintCiphertext {
    // let sel = sv.par_iter().fold(
    //     || sks.create_trivial(0),
    //     |mut acc, ct| {
    //         lwe_ciphertext_add_assign(&mut acc.ct, &ct.ct);
    //         acc
    //     },
    // ).reduce(
    //     || sks.create_trivial(0),
    //     |mut a, b| {
    //         lwe_ciphertext_add_assign(&mut a.ct, &b.ct);
    //         a
    //     },
    // );

    let mut sel = sks.create_trivial(0);
    for ridx in 0..sv.len() as usize {
        lwe_ciphertext_add_assign(&mut sel.ct, &sv[ridx].ct);

        // if ridx > 0 && ridx % 511 == 0 {
        //     sks.message_extract_assign(&mut sel);
        // }
    }

    ShortintCiphertext::new(
        sel.ct,
        Degree::new(sks.message_modulus.0 - 1),
        NoiseLevel::NOMINAL,
        sks.message_modulus,
        sks.carry_modulus,
        sks.pbs_order,
    )
}

/// Computes the inner product of the selection vector and the database values.
pub fn inner_product(db: &Database<u64>, selection_vector: &[FheUint64]) -> FheUint64 {
    assert!(
        db.db_params.max_data_bitlen <= 64,
        "Currently only supports 64-bit data"
    );

    let mut result: Option<FheUint64> = None;
    for idx in 0..db.db_params.num_keywords as usize {
        let selection_bit: &FheUint64 = &selection_vector[idx];
        let prod: FheUint64 = ((!selection_bit) + 1) & db.data[idx][0];
        result = Some(match result {
            None => prod,
            Some(result) => result | prod,
        });
    }

    result.unwrap()
}

pub fn inner_product_shorint(
    sks: &ShortintServerKey,
    db: &Database<u8>,
    selection_vector: &[ShortintCiphertext],
) -> Vec<ShortintCiphertext> {
    let mut res = Vec::new();

    let num_partitions = db.db_params.num_partitions();

    for pidx in 0..num_partitions {
        let mut tmp: Option<ShortintCiphertext> = None;
        for ridx in 0..db.db_params.num_keywords as usize {
            let prod = sks.scalar_mul(&selection_vector[ridx], db.data[ridx][pidx]);
            tmp = Some(match tmp {
                None => prod,
                Some(tmp) => sks.add(&tmp, &prod),
            });
        }
        res.push(tmp.unwrap());
    }

    res
}

pub fn inner_product_lwe(
    sks: &ShortintServerKey,
    db: &Database<u8>,
    selection_vector: &[ShortintCiphertext],
) -> Vec<ShortintCiphertext> {
    let num_partitions = db.db_params.num_partitions();

    (0..num_partitions)
        .into_par_iter()
        .map(|pidx| {
            let mut prod = sks.create_trivial(0);
            let mut sum = sks.create_trivial(0);

            for ridx in 0..db.db_params.num_keywords as usize {
                lwe_ciphertext_cleartext_mul(
                    &mut prod.ct,
                    &selection_vector[ridx].ct,
                    Cleartext(db.data[ridx][pidx] as u64),
                );

                lwe_ciphertext_add_assign(&mut sum.ct, &prod.ct);

                if ridx > 0 && ridx % 511 == 0 {
                    sks.message_extract_assign(&mut sum);
                }
            }

            ShortintCiphertext::new(
                sum.ct,
                Degree::new(sks.message_modulus.0 - 1),
                NoiseLevel::NOMINAL,
                sks.message_modulus,
                sks.carry_modulus,
                sks.pbs_order,
            )
        })
        .collect()
}

pub fn client_aided_inner_product(
    sks: &ShortintServerKey,
    bff_params: &ByteFuseFilterParameters,
    db: &Database<u8>,
    selection_vector: &[ShortintCiphertext],
) -> Vec<ShortintCiphertext> {
    let num_partitions = db.db_params.num_partitions();
    let digest_byte_len = bff_params.digest_byte_len;
    let key_byte_len = bff_params.key_byte_len();

    (0..num_partitions + key_byte_len)
        .into_par_iter()
        .map(|pidx| {
            let mut prod = sks.create_trivial(0);
            let mut sum = sks.create_trivial(0);

            for ridx in 0..bff_params.filter_len as usize {
                lwe_ciphertext_cleartext_mul(
                    &mut prod.ct,
                    &selection_vector[ridx].ct,
                    Cleartext(db.fused_data[ridx][pidx] as u64),
                );

                lwe_ciphertext_add_assign(&mut sum.ct, &prod.ct);
            }

            ShortintCiphertext::new(
                sum.ct,
                Degree::new(sks.message_modulus.0 - 1),
                NoiseLevel::NOMINAL,
                sks.message_modulus,
                sks.carry_modulus,
                sks.pbs_order,
            )
        })
        .collect()
}

// generalization of `inner_product`
pub fn select(
    scheme: Scheme,
    params: &ClassicPBSParameters,
    sks: &ShortintServerKey,
    size: usize,
    start_column: usize,
    end_column: usize,
    selection_vector: &[ShortintCiphertext],
    data: &[Vec<u8>],
    default: Option<u64>,
) -> FheUint64 {
    assert!(scheme == Scheme::TCWKPIR, "Currently only supports TCWKPIR");

    let result: Vec<ShortintCiphertext> = (start_column..end_column)
        .into_par_iter()
        .map(|pidx| {
            let mut prod = sks.create_trivial(0);
            let mut sum = sks.create_trivial(0);

            for ridx in 0..size {
                lwe_ciphertext_cleartext_mul(
                    &mut prod.ct,
                    &selection_vector[ridx].ct,
                    Cleartext(data[ridx][pidx] as u64),
                );

                lwe_ciphertext_add_assign(&mut sum.ct, &prod.ct);

                if ridx > 0 && ridx % 511 == 0 {
                    sks.message_extract_assign(&mut sum);
                }
            }

            ShortintCiphertext::new(
                sum.ct,
                Degree::new(sks.message_modulus.0 - 1),
                NoiseLevel::NOMINAL,
                sks.message_modulus,
                sks.carry_modulus,
                sks.pbs_order,
            )
        })
        .collect();

    match default {
        Some(default) => {
            let result = shortint_ciphertexts_to_fheuint64(&params, &result);
            let sel = accumulate_selection(&sks, &selection_vector);
            let sel = shortint_ciphertexts_to_fheuint64(&params, &[sel]);
            (1 - &sel) * default + &sel * result
        }
        None => shortint_ciphertexts_to_fheuint64(&params, &result),
    }
}

// Pre-computes the masks part of the inner product.
pub fn mask_database(
    masks: &[Vec<u64>],
    lwe_dim: usize,
    db: &Database<u8>,
    bff_params: &ByteFuseFilterParameters,
) -> Vec<Vec<u64>> {
    let db_params = db.db_params;

    let num_partitions = db_params.num_partitions();
    let key_byte_len = bff_params.key_byte_len();
    let data = db.get_fused_data();

    // compute masked_data in parallel over columns (i)
    (0..key_byte_len + num_partitions)
        .into_par_iter()
        .map(|i| {
            let mut mask_i = vec![0u64; lwe_dim];
            for j in 0..(bff_params.filter_len as usize) {
                let mask_row = &masks[j];
                let data_val = data[j][i] as u64;
                for k in 0..lwe_dim {
                    // use wrapping ops to match original behavior
                    mask_i[k] = mask_i[k].wrapping_add(mask_row[k].wrapping_mul(data_val));
                }
            }
            mask_i
        })
        .collect()
}

pub fn client_aided_inner_product_from_raw_parts(
    bodies: &[u64],
    masked_data: &[Vec<u64>],
    db: &Database<u8>,
    params: &ClassicPBSParameters,
    bff_params: &ByteFuseFilterParameters,
) -> Vec<ShortintCiphertext> {
    let db_params = db.db_params;

    let num_partitions = db_params.num_partitions();
    let key_byte_len = bff_params.key_byte_len();
    let data = db.get_fused_data();

    // Parallelize over partitions + key bytes. Each iteration is independent; collect preserves order.
    (0..num_partitions + key_byte_len)
        .into_par_iter()
        .map(|pidx| {
            let mut sum: u64 = 0;
            for ridx in 0..bff_params.filter_len as usize {
                sum = sum.wrapping_add(bodies[ridx].wrapping_mul(data[ridx][pidx] as u64));
            }
            create_shortint_ciphertext_from_raw_parts(&params, &masked_data[pidx], sum)
        })
        .collect()
}
