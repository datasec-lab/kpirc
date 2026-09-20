use crate::utils::cwutils::choose;

pub const DEFAULT: u64 = u64::MAX;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Scheme {
    TCWKPIR,
    TCAKPIR,
    TCAKPIR_OPT,
}

#[derive(Debug)]
pub struct DatabaseParameters {
    pub num_keywords: usize,
    pub max_keyword_bitlen: u64,
    pub max_data_bitlen: u64,
    pub partition_bitlen: u64,
}

impl DatabaseParameters {
    pub fn new(
        num_keywords: usize,
        mut max_keyword_bitlen: u64,
        max_data_bitlen: u64,
        partition_bitlen: u64,
    ) -> Self {
        assert!(
            partition_bitlen <= max_data_bitlen,
            "partition_bitlen must be <= max_data_bitlen"
        );

        if max_keyword_bitlen < 64 && num_keywords > (1 << max_keyword_bitlen) {
            max_keyword_bitlen = (64 - num_keywords.leading_zeros()) as u64;
        }

        Self {
            num_keywords,
            max_keyword_bitlen,
            max_data_bitlen,
            partition_bitlen,
        }
    }

    pub fn num_partitions(&self) -> usize {
        let data_bitlen = self.max_data_bitlen;
        let partition_bitlen = self.partition_bitlen;

        ((data_bitlen + partition_bitlen - 1) / self.partition_bitlen) as usize
    }

    pub fn db_size_in_B(&self) -> usize {
        (self.num_keywords * ((self.max_keyword_bitlen + self.max_data_bitlen) as usize)) / 8
    }

    pub fn db_size_in_KB(&self) -> usize {
        (self.num_keywords * ((self.max_keyword_bitlen + self.max_data_bitlen) as usize)) / 8 / 1024
    }
}

#[derive(Debug)]
pub struct ByteFuseFilterParameters {
    pub filter_len: usize,
    pub filter_seed: Vec<u8>,
    pub digest_byte_len: usize,
    pub partition_bitlen: usize,
}

impl ByteFuseFilterParameters {
    pub fn key_byte_len(&self) -> usize {
        self.digest_byte_len * (8 / self.partition_bitlen as usize)
    }
}

pub struct ConstantWeightParameters {
    pub hamming_weight: u64,
    pub codeword_bitlen: u64,
}

impl ConstantWeightParameters {
    pub fn new(hamming_weight: u64, keyword_bitlen: u64) -> Self {
        let codeword_bitlen = Self::get_codeword_bitlen(keyword_bitlen, hamming_weight);

        Self {
            hamming_weight,
            codeword_bitlen,
        }
    }

    fn get_codeword_bitlen(keyword_bitlen: u64, hamming_weight: u64) -> u64 {
        let mut codeword_bitlen = hamming_weight;

        while choose(codeword_bitlen, hamming_weight) < (1 << keyword_bitlen) {
            codeword_bitlen += 1;
        }

        codeword_bitlen
    }
}

pub struct FuzzyPIRParameters {
    pub delta: usize,
    pub k: usize,
}

impl FuzzyPIRParameters {
    pub fn cal_num_queries(&self) -> usize {
        self.delta * 2 + 1
    }
}

// cargo test common::test_get_codeword_bitlen --release -- --nocapture
#[test]
fn test_get_codeword_bitlen() {
    // let mut table = Vec::new();

    let mut l = 18;

    let encoding_size = ConstantWeightParameters::get_codeword_bitlen(l, 2);

    println!(
        "Codeword bitlen for l={} and hamming_weight=2: {}",
        l, encoding_size
    );

    // while l > 0 {
    //     let mut row = Vec::new();
    //     for h in 1..=l {
    //         let codeword_bitlen = ConstantWeightParameters::get_codeword_bitlen(l, h);
    //         let query_size = codeword_bitlen * 192;
    //         let query_size_KB = query_size / 1024;
    //         row.push((h, query_size_KB));
    //     }
    //     table.push(row);
    //     l -= 1;
    // }

    // for (idx, row) in table.iter().rev().enumerate() {
    //     println!("DB Size: {} -> {:?}", 1 << (idx + 1), row);
    // }
}
