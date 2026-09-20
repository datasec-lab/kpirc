use crate::byte_fuse_filter::ByteFuseFilter;
use core::cmp::max;
/// empty struct, just provide static method
pub struct ByteFuseUtils;

impl ByteFuseUtils {
    /// Calculates segment length for arity = 3, must be a power-of-2.
    pub fn calc_arity3_segment_len(size: usize) -> usize {
        let tmp = f64::floor((size as f64).ln() / f64::ln(3.33) + 2.25) as usize;
        return 1 << tmp;
    }
    /// Calculates size factor c for arity = 3.
    pub fn calc_arity3_size_factor(size: usize) -> f64 {
        return f64::max(
            1.125,
            0.875 + 0.25 * (1000000.0f64).ln() / (size as f64).ln(),
        );
    }

    /// Shrink the hash to a value [0, n).
    /// Kind of like modulo, but using multiplication and shift, which are faster to compute.
    pub fn reduce(hash: u32, n: usize) -> u32 {
        (((hash & 0xffffffffu32) as u64 * (n as u32 & 0xffffffffu32) as u64) >> 32u8) as u32
    }

    pub fn subi(p: &mut Vec<u8>, q: &Vec<u8>, byte_len: usize) {
        assert!(p.len() == byte_len && q.len() == byte_len);
        for i in 0..byte_len {
            p[i] -= q[i];
        }
    }

    pub fn addi(p: &mut Vec<u8>, q: &Vec<u8>, byte_len: usize) {
        assert!(p.len() == byte_len && q.len() == byte_len);
        for i in 0..byte_len {
            p[i] += q[i];
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::zip;

    use super::*;

    #[test]
    fn reduce_test() {
        let hash = [
            545693717, 669843641, 1444826400, 1662843740, 2007160190, 1045961011, 520381769,
            1935571071, 1201080092, 811845399, 247091498, 374828729, 1289836721, 446935794,
            1155000838, 237493908, 2102044607, 494050218, 1496085257, 699478723, 1814492027,
            1132207814, 1407444416, 1662677460, 1858811833, 831582605, 827162104, 814404617,
            15316593, 1377823551, 1035385672, 721232524, 1306638730, 714725358, 1449207397,
            445756601, 2058877122, 1315834551, 1713043639, 1362173298, 1744775515, 701912272,
            2014587579, 1884080969, 477892291, 867232084, 2122163685, 1254291581, 1633831782,
            122065897, 1644905615, 842743086, 2071283097, 1668460622, 447210336, 990073496,
            1548612076, 1174582537, 1730110982, 1239289081, 841286108, 1829728276, 959331184,
            355404037, 2065718101, 2067106922, 2063262925, 303702530, 209184166, 331788261,
            563540286, 3403416, 210126025, 36279622, 1144786026, 951074315, 1933858010, 1749610336,
            195945573, 1340454124, 1331293005, 432930483, 666951985, 1330341601, 899826796,
            2020894995, 1866357077, 1681249134, 1278804474, 443446601, 1924715292, 2075235893,
            1037855303, 1504899834, 2102401481, 1805564379, 417293353, 2074347406, 2009975817,
            761322158,
        ];
        let res = [
            130, 159, 344, 396, 478, 249, 124, 461, 286, 193, 58, 89, 307, 106, 275, 56, 501, 117,
            356, 166, 432, 269, 335, 396, 443, 198, 197, 194, 3, 328, 246, 171, 311, 170, 345, 106,
            490, 313, 408, 324, 415, 167, 480, 449, 113, 206, 505, 299, 389, 29, 392, 200, 493,
            397, 106, 236, 369, 280, 412, 295, 200, 436, 228, 84, 492, 492, 491, 72, 49, 79, 134,
            0, 50, 8, 272, 226, 461, 417, 46, 319, 317, 103, 159, 317, 214, 481, 444, 400, 304,
            105, 458, 494, 247, 358, 501, 430, 99, 494, 479, 181,
        ];
        let n = 1024;

        for (h, r) in zip(hash, res) {
            assert_eq!(r, ByteFuseUtils::reduce(h, n));
        }
    }

    #[test]
    fn calc_arity3_segment_len_test() {
        let size_array = [
            4usize, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
        ];

        let res_array = [
            8usize, 8, 16, 16, 32, 64, 64, 128, 128, 256, 512, 512, 1024, 1024,
        ];
        for (size, res) in zip(size_array, res_array) {
            assert_eq!(res, ByteFuseUtils::calc_arity3_segment_len(size));
        }
    }

    #[test]
    fn calc_arity3_size_factor_test() {
        let size_array = [
            4usize, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384, 32768,
        ];

        let res_array = [
            3.3664460711655217 as f64,
            2.5359640474436813,
            2.1207230355827607,
            1.8715784284662087,
            1.7054820237218407,
            1.5868417346187205,
            1.4978615177913803,
            1.428654682481227,
            1.3732892142331043,
            1.3279901947573676,
            1.2902410118609202,
            1.2582993955639266,
            1.2309208673093601,
            1.2071928094887363,
        ];
        for (size, res) in zip(size_array, res_array) {
            assert_eq!(res, ByteFuseUtils::calc_arity3_size_factor(size));
        }
    }
}
