//! The one hasher of the crate's hash maps and sets: fast, deterministic, and the same on every
//! platform, also on 32-bit wasm (the web version) and 64-bit desktops.
//!
//! [`HashMap`] and [`HashSet`] are std's maps with [`Fx64BuildHasher`]. Use them for every map
//! and set in the crate; Clippy (`clippy.toml`, `disallowed_types`) rejects std's randomly seeded
//! default hasher and `rustc_hash`, whose hashes depend on the pointer width.
//!
//! [`Fx64Hasher`] is the Fx hash of `rustc-hash` 2.x (one multiply-add per integer, a rotation at
//! the end) with its state fixed at `u64`. `rustc-hash`'s `FxHasher` keeps its state in a
//! `usize` with a different multiplier and rotation on 32-bit targets, so its hashes differ
//! between wasm32 and a 64-bit desktop. Here every integer is widened to `u64` (`usize` and
//! `isize` too, signed values sign-extended), so the same key gives the same hash everywhere. For
//! the integer keys of this crate (`u32`, `u64`, tuples and structs of them, enum discriminants)
//! the hashes are exactly those of `rustc-hash` on 64-bit targets, so a native run is unchanged.
//! Byte strings (not used as keys here) get a simple portable hash of their 8-byte words.
//!
//! Identical hashes do not guarantee an identical iteration order: std's hash table probes in
//! groups of 16 slots with SSE2 (x86-64) and of 8 slots elsewhere (wasm32, ARM), so entries that
//! collide can land in different slots. Iteration order is deterministic on each platform, but
//! where it affects a result or a logged value it must not differ between platforms: iterate in
//! a defined order there (sort the keys or the values, or use a `BTreeMap`).

// The one place that names std's maps with a hasher of our choice.
#![allow(clippy::disallowed_types)]

use std::hash::{BuildHasher, Hasher};

/// Hash map with [`Fx64BuildHasher`], deterministic and the same on all platforms.
pub type HashMap<K, V> = std::collections::HashMap<K, V, Fx64BuildHasher>;
/// Hash set with [`Fx64BuildHasher`], deterministic and the same on all platforms.
pub type HashSet<T> = std::collections::HashSet<T, Fx64BuildHasher>;

/// Multiplier of `rustc-hash` 2.x on 64-bit targets (Steele and Vigna, "Computationally easy,
/// spectrally good multipliers for congruential pseudorandom number generators").
const K: u64 = 0xf135_7aea_2e62_a9c5;
/// Final rotation of `rustc-hash` 2.x on 64-bit targets: moves the well-mixed high bits to the
/// low bits, from which the hash table takes the bucket index.
const ROTATE: u32 = 26;

/// Fx hasher with a `u64` state; see the [module documentation](self).
#[derive(Clone, Copy, Debug, Default)]
pub struct Fx64Hasher {
    hash: u64,
}

impl Fx64Hasher {
    #[inline]
    fn add_to_hash(&mut self, i: u64) {
        self.hash = self.hash.wrapping_add(i).wrapping_mul(K);
    }
}

impl Hasher for Fx64Hasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, rest) = bytes.as_chunks::<8>();
        for &chunk in chunks {
            self.add_to_hash(u64::from_le_bytes(chunk));
        }
        if !rest.is_empty() {
            let mut word = [0; 8];
            word[..rest.len()].copy_from_slice(rest);
            self.add_to_hash(u64::from_le_bytes(word));
        }
        // The length tells "ab" from "ab\0".
        self.add_to_hash(bytes.len() as u64);
    }

    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add_to_hash(u64::from(i));
    }

    #[inline]
    fn write_u16(&mut self, i: u16) {
        self.add_to_hash(u64::from(i));
    }

    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add_to_hash(u64::from(i));
    }

    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add_to_hash(i);
    }

    #[inline]
    fn write_u128(&mut self, i: u128) {
        self.add_to_hash(i as u64);
        self.add_to_hash((i >> 64) as u64);
    }

    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add_to_hash(i as u64);
    }

    // The signed writes default to the unsigned ones of the same width (`i as u32` etc.), which
    // is portable, except `isize`: `-1isize as usize` is 32 one bits on wasm32 and 64 natively.
    // `#[derive(Hash)]` hashes enum discriminants as `isize`.
    #[inline]
    fn write_isize(&mut self, i: isize) {
        self.add_to_hash(i as i64 as u64);
    }

    #[inline]
    fn finish(&self) -> u64 {
        self.hash.rotate_left(ROTATE)
    }
}

/// Builds [`Fx64Hasher`]s, all with the same (zero) initial state.
#[derive(Clone, Copy, Debug, Default)]
pub struct Fx64BuildHasher;

impl BuildHasher for Fx64BuildHasher {
    type Hasher = Fx64Hasher;

    #[inline]
    fn build_hasher(&self) -> Fx64Hasher {
        Fx64Hasher::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::Hash;

    fn hash<T: Hash>(value: T) -> u64 {
        Fx64BuildHasher.hash_one(value)
    }

    #[derive(Hash)]
    enum Kind {
        A,
        B(u32),
    }

    /// Fixed reference values: the hashes must be the same on every platform, 32-bit too (this
    /// test also runs on wasm32, see `notes/issue-87/`). Derived independently of this code, from
    /// the definition above and the `Hash` impls of std (`str` adds a `0xff` byte, slices
    /// their length, `#[derive(Hash)]` the discriminant as `isize`).
    #[test]
    fn reference_values() {
        let cases: [(&str, u64); 16] = [
            ("u8", hash(7u8)),
            ("u16", hash(0xbeefu16)),
            ("u32", hash(12_345u32)),
            ("u32 max", hash(u32::MAX)),
            ("u64", hash(0x0123_4567_89ab_cdefu64)),
            ("u128", hash(0x0123_4567_89ab_cdef_fedc_ba98_7654_3210u128)),
            ("usize", hash(12_345usize)),
            ("isize -1", hash(-1isize)),
            ("i32 -1", hash(-1i32)),
            ("i64 -1", hash(-1i64)),
            ("(u32, u32)", hash((3u32, 17u32))),
            ("(u32, u32) swapped", hash((17u32, 3u32))),
            ("enum unit", hash(Kind::A)),
            ("enum field", hash(Kind::B(5))),
            ("str", hash("world-simulator")),
            (
                "bytes",
                hash([1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11].as_slice()),
            ),
        ];
        let expected: [u64; 16] = [
            0x9d12_ca91_8e61_d971,
            0x3a20_72d3_af88_9e3c,
            0x5b4f_32f3_76ef_d11b,
            0x6b46_7558_ecf4_b4bb,
            0x2c8e_98ff_aeb7_f9df,
            0x502c_94ed_ec52_c20b,
            0x5b4f_32f3_76ef_d11b,
            0x5746_7558_ec3b_2a14,
            0x6b46_7558_ecf4_b4bb,
            0x5746_7558_ec3b_2a14,
            0x52eb_296b_8104_116c,
            0xfce4_b321_e0a9_0f7a,
            0x0000_0000_0000_0000,
            0x007d_5c09_cb28_cc87,
            0x0ccc_2988_c9d8_3821,
            0xbb6c_1f75_c263_2a76,
        ];
        for ((name, actual), expected) in cases.iter().zip(expected) {
            assert_eq!(*actual, expected, "{name}: {actual:#018x}");
        }
    }

    /// `usize` and `isize` hash like the `u64`/`i64` of the same value, on every platform.
    #[test]
    fn pointer_width_independent() {
        assert_eq!(hash(12_345usize), hash(12_345u64));
        assert_eq!(hash(usize::MAX >> 1), hash((usize::MAX >> 1) as u64));
        assert_eq!(hash(-1isize), hash(-1i64));
        assert_eq!(hash(isize::MIN), hash(isize::MIN as i64));
    }

    /// On 64-bit targets the hashes of integer keys are exactly `rustc-hash`'s, which the crate
    /// used before, so the layout and iteration order of native maps did not change.
    #[cfg(target_pointer_width = "64")]
    #[test]
    #[allow(clippy::disallowed_types)]
    fn same_as_rustc_hash_on_64_bit() {
        let fx = rustc_hash::FxBuildHasher;
        for i in [0u32, 1, 2, 3, 1000, 123_456, u32::MAX] {
            assert_eq!(hash(i), fx.hash_one(i));
            assert_eq!(hash(u64::from(i) << 20), fx.hash_one(u64::from(i) << 20));
            assert_eq!(hash((i, i / 3)), fx.hash_one((i, i / 3)));
            assert_eq!(hash(i as usize), fx.hash_one(i as usize));
            assert_eq!(hash(-(i as isize)), fx.hash_one(-(i as isize)));
            assert_eq!(hash(u128::from(i) << 70), fx.hash_one(u128::from(i) << 70));
        }
        assert_eq!(hash(Kind::B(9)), fx.hash_one(Kind::B(9)));
    }
}
