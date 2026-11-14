//! Structural hashing: hashes a value field by field so padding bytes never
//! leak into the result (hashing `mem::as_bytes(&struct)` reads undefined
//! padding and makes memo keys change at random).
//!
//! Rules: bool/int/enum by value; floats by bit pattern; optionals by
//! presence + payload; arrays element-wise; structs recursively; enums by tag +
//! payload; `&str` / `&[u8]` (and other byte slices) by content; other slices
//! element-wise. Raw pointers are not hashed: there is no meaningful pointer
//! identity and no code needs it.
//!
//! The hash function is wyhash final version 4.2, **not** `std::hash` and not
//! the `wyhash` crate (which implements older wyhash versions): TSX child ids
//! and cache keys depend on the exact values, which the tests below pin. The
//! `wyhash` workspace dependency is therefore not used by this module.
//!
//! [`HashValue`] has impls for the primitive shapes. Structs and enums
//! implement the trait field by field (enums: tag, then payload).

/// Wyhash (final version 4.2), incremental and one-shot.
#[derive(Clone)]
pub struct Wyhash {
    a: u64,
    b: u64,
    state: [u64; 3],
    total_len: usize,
    buf: [u8; 48],
    buf_len: usize,
}

const SECRET: [u64; 4] = [
    0xa0761d6478bd642f,
    0xe7037ed1a0b428db,
    0x8ebc6af09c88c6e3,
    0x589965cc75374cc3,
];

impl Wyhash {
    /// A hasher seeded with `seed`.
    pub fn init(seed: u64) -> Self {
        let state = seed ^ mix(seed ^ SECRET[0], SECRET[1]);
        Wyhash {
            a: 0,
            b: 0,
            state: [state; 3],
            total_len: 0,
            buf: [0; 48],
            buf_len: 0,
        }
    }

    /// Feeds `input` into the hash.
    ///
    /// This is subtly different from other hash function update calls: wyhash
    /// requires the last full 48-byte block to go through `final1` when the
    /// total length is exactly aligned to 48 bytes.
    pub fn update(&mut self, input: &[u8]) {
        self.total_len += input.len();

        if input.len() <= 48 - self.buf_len {
            self.buf[self.buf_len..self.buf_len + input.len()].copy_from_slice(input);
            self.buf_len += input.len();
            return;
        }

        let mut i: usize = 0;

        if self.buf_len > 0 {
            i = 48 - self.buf_len;
            self.buf[self.buf_len..self.buf_len + i].copy_from_slice(&input[..i]);
            let buffered = self.buf;
            self.round(&buffered);
            self.buf_len = 0;
        }

        while i + 48 < input.len() {
            let mut block = [0u8; 48];
            block.copy_from_slice(&input[i..i + 48]);
            self.round(&block);
            i += 48;
        }

        let remaining_bytes = &input[i..];
        if remaining_bytes.len() < 16 && i >= 48 {
            let rem = 16 - remaining_bytes.len();
            self.buf[48 - rem..48].copy_from_slice(&input[i - rem..i]);
        }
        self.buf[..remaining_bytes.len()].copy_from_slice(remaining_bytes);
        self.buf_len = remaining_bytes.len();
    }

    /// The hash of everything fed so far. Idempotent: the hasher is unchanged.
    pub fn finish(&self) -> u64 {
        let mut input: &[u8] = &self.buf[..self.buf_len];
        let mut new_self = self.shallow_copy(); // ensure idempotency
        let mut scratch = [0u8; 16];

        if self.total_len <= 16 {
            new_self.small_key(input);
        } else {
            let mut offset: usize = 0;
            if self.buf_len < 16 {
                let rem = 16 - self.buf_len;
                scratch[..rem].copy_from_slice(&self.buf[48 - rem..48]);
                scratch[rem..rem + self.buf_len].copy_from_slice(&self.buf[..self.buf_len]);

                // Same as input but with additional bytes preceding start in
                // case of a short buffer.
                input = &scratch;
                offset = rem;
            }

            new_self.final0();
            new_self.final1(input, offset);
        }

        new_self.final2()
    }

    /// One-shot hash, equivalent to init + update + finish.
    pub fn hash(seed: u64, input: &[u8]) -> u64 {
        let mut this = Wyhash::init(seed);

        if input.len() <= 16 {
            this.small_key(input);
        } else {
            let mut i: usize = 0;
            if input.len() >= 48 {
                while i + 48 < input.len() {
                    let mut block = [0u8; 48];
                    block.copy_from_slice(&input[i..i + 48]);
                    this.round(&block);
                    i += 48;
                }
                this.final0();
            }
            this.final1(input, i);
        }

        this.total_len = input.len();
        this.final2()
    }

    // Copies the core wyhash state but not any internal buffers.
    fn shallow_copy(&self) -> Wyhash {
        Wyhash {
            a: self.a,
            b: self.b,
            state: self.state,
            total_len: self.total_len,
            buf: [0; 48],
            buf_len: 0,
        }
    }

    fn small_key(&mut self, input: &[u8]) {
        debug_assert!(input.len() <= 16);

        if input.len() >= 4 {
            let end = input.len() - 4;
            let quarter = (input.len() >> 3) << 2;
            self.a = (read4(&input[0..]) << 32) | read4(&input[quarter..]);
            self.b = (read4(&input[end..]) << 32) | read4(&input[end - quarter..]);
        } else if !input.is_empty() {
            self.a = ((input[0] as u64) << 16)
                | ((input[input.len() >> 1] as u64) << 8)
                | input[input.len() - 1] as u64;
            self.b = 0;
        } else {
            self.a = 0;
            self.b = 0;
        }
    }

    fn round(&mut self, input: &[u8; 48]) {
        for i in 0..3 {
            let a = read8(&input[8 * (2 * i)..]);
            let b = read8(&input[8 * (2 * i + 1)..]);
            self.state[i] = mix(a ^ SECRET[i + 1], b ^ self.state[i]);
        }
    }

    fn final0(&mut self) {
        self.state[0] ^= self.state[1] ^ self.state[2];
    }

    fn final1(&mut self, input_lb: &[u8], start_pos: usize) {
        debug_assert!(input_lb.len() >= 16);
        debug_assert!(input_lb.len() - start_pos <= 48);
        let input = &input_lb[start_pos..];

        let mut i: usize = 0;
        while i + 16 < input.len() {
            self.state[0] = mix(
                read8(&input[i..]) ^ SECRET[1],
                read8(&input[i + 8..]) ^ self.state[0],
            );
            i += 16;
        }

        self.a = read8(&input_lb[input_lb.len() - 16..]);
        self.b = read8(&input_lb[input_lb.len() - 8..]);
    }

    fn final2(&mut self) -> u64 {
        self.a ^= SECRET[1];
        self.b ^= self.state[0];
        mum(&mut self.a, &mut self.b);
        mix(
            self.a ^ SECRET[0] ^ self.total_len as u64,
            self.b ^ SECRET[1],
        )
    }
}

fn read4(data: &[u8]) -> u64 {
    let mut bytes = [0u8; 4];
    bytes.copy_from_slice(&data[..4]);
    u32::from_le_bytes(bytes) as u64
}

fn read8(data: &[u8]) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[..8]);
    u64::from_le_bytes(bytes)
}

fn mum(a: &mut u64, b: &mut u64) {
    let x = (*a as u128) * (*b as u128);
    *a = x as u64;
    *b = (x >> 64) as u64;
}

fn mix(a: u64, b: u64) -> u64 {
    let mut a = a;
    let mut b = b;
    mum(&mut a, &mut b);
    a ^ b
}

/// Hashes a value structurally. Implement it field by field for structs, and
/// tag-then-payload for enums.
pub trait HashValue {
    fn hash_value(&self, h: &mut Wyhash);
}

/// Convenience: one-shot hash of `value` with `seed`.
pub fn hash<T: HashValue + ?Sized>(seed: u64, value: &T) -> u64 {
    let mut h = Wyhash::init(seed);
    value.hash_value(&mut h);
    h.finish()
}

impl HashValue for () {
    fn hash_value(&self, _h: &mut Wyhash) {}
}

impl HashValue for bool {
    fn hash_value(&self, h: &mut Wyhash) {
        h.update(&[u8::from(*self)]);
    }
}

macro_rules! impl_hash_int {
    ($($ty:ty as $unsigned:ty),* $(,)?) => {
        $(
            impl HashValue for $ty {
                fn hash_value(&self, h: &mut Wyhash) {
                    // Reinterpreted as the unsigned integer of the same
                    // width, then hashed as its little-endian bytes.
                    h.update(&(*self as $unsigned).to_le_bytes());
                }
            }
        )*
    };
}

impl_hash_int!(
    u8 as u8,
    i8 as u8,
    u16 as u16,
    i16 as u16,
    u32 as u32,
    i32 as u32,
    u64 as u64,
    i64 as u64,
    u128 as u128,
    i128 as u128,
);

// `usize`/`isize` are hashed as 64-bit values; every supported target is 64-bit.
#[cfg(target_pointer_width = "64")]
impl HashValue for usize {
    fn hash_value(&self, h: &mut Wyhash) {
        h.update(&(*self as u64).to_le_bytes());
    }
}

#[cfg(target_pointer_width = "64")]
impl HashValue for isize {
    fn hash_value(&self, h: &mut Wyhash) {
        h.update(&(*self as u64).to_le_bytes());
    }
}

#[cfg(not(target_pointer_width = "64"))]
impl HashValue for usize {
    fn hash_value(&self, h: &mut Wyhash) {
        h.update(&(*self as u32).to_le_bytes());
    }
}

#[cfg(not(target_pointer_width = "64"))]
impl HashValue for isize {
    fn hash_value(&self, h: &mut Wyhash) {
        h.update(&(*self as u32).to_le_bytes());
    }
}

impl HashValue for f32 {
    fn hash_value(&self, h: &mut Wyhash) {
        self.to_bits().hash_value(h);
    }
}

impl HashValue for f64 {
    fn hash_value(&self, h: &mut Wyhash) {
        self.to_bits().hash_value(h);
    }
}

impl HashValue for char {
    fn hash_value(&self, h: &mut Wyhash) {
        // A codepoint hashes as its `u32` value.
        (*self as u32).hash_value(h);
    }
}

impl<T: HashValue> HashValue for Option<T> {
    fn hash_value(&self, h: &mut Wyhash) {
        match self {
            Some(payload) => {
                h.update(&[1]);
                payload.hash_value(h);
            }
            None => h.update(&[0]),
        }
    }
}

impl<T: HashValue> HashValue for [T] {
    fn hash_value(&self, h: &mut Wyhash) {
        // Slices hash their length first, then their elements. For byte
        // slices, feeding `u8`s one at a time produces the same byte stream as
        // hashing the content in one call, hence the same result.
        self.len().hash_value(h);
        for item in self {
            item.hash_value(h);
        }
    }
}

impl<T: HashValue> HashValue for Vec<T> {
    fn hash_value(&self, h: &mut Wyhash) {
        self.as_slice().hash_value(h);
    }
}

impl<T: HashValue, const N: usize> HashValue for [T; N] {
    fn hash_value(&self, h: &mut Wyhash) {
        // Arrays hash element-wise, without a length prefix.
        for item in self {
            item.hash_value(h);
        }
    }
}

impl HashValue for str {
    fn hash_value(&self, h: &mut Wyhash) {
        self.as_bytes().hash_value(h);
    }
}

impl HashValue for String {
    fn hash_value(&self, h: &mut Wyhash) {
        self.as_str().hash_value(h);
    }
}

impl<T: HashValue + ?Sized> HashValue for &T {
    fn hash_value(&self, h: &mut Wyhash) {
        (**self).hash_value(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    #[repr(u8)]
    enum Kind {
        X = 0,
        Y = 1,
    }

    #[derive(Clone)]
    enum Tagged<'a> {
        N(u16),
        S(&'a str),
    }

    impl HashValue for Kind {
        fn hash_value(&self, h: &mut Wyhash) {
            // Enums hash their backing integer.
            (*self as u8).hash_value(h);
        }
    }

    impl HashValue for Tagged<'_> {
        fn hash_value(&self, h: &mut Wyhash) {
            // Enums with data hash the tag then the payload.
            match self {
                Tagged::N(payload) => {
                    0u8.hash_value(h);
                    payload.hash_value(h);
                }
                Tagged::S(payload) => {
                    1u8.hash_value(h);
                    payload.hash_value(h);
                }
            }
        }
    }

    #[derive(Clone)]
    struct Structural<'a> {
        a: u8,
        b: u64,
        name: &'a str,
        opt: Option<f32>,
        kind: Kind,
        u: Tagged<'a>,
    }

    impl HashValue for Structural<'_> {
        fn hash_value(&self, h: &mut Wyhash) {
            self.a.hash_value(h);
            self.b.hash_value(h);
            self.name.hash_value(h);
            self.opt.hash_value(h);
            self.kind.hash_value(h);
            self.u.hash_value(h);
        }
    }

    fn structural(union_s: bool) -> Structural<'static> {
        Structural {
            a: 1,
            b: 2,
            name: "abc",
            opt: Some(1.5),
            kind: Kind::Y,
            u: if union_s {
                Tagged::S("hi")
            } else {
                Tagged::N(3)
            },
        }
    }

    /// Regression values: TSX child ids and cache keys depend on them, so
    /// the hash of each primitive shape must never change.
    #[test]
    fn hash_values_are_pinned() {
        assert_eq!(0x0409_638e_e2bd_e459, hash(0, &()));
        assert_eq!(0xd5b2_6ee4_85f2_a92d, hash(0, &true));
        assert_eq!(0xfbe5_af10_e5f8_bd85, hash(0, &false));
        assert_eq!(0x17fd_d546_027b_e849, hash(0, &0xABu8));
        assert_eq!(0xa092_0e9c_a332_6c3b, hash(0, &0x1234u16));
        assert_eq!(0x01a5_c7e4_4722_ad15, hash(0, &0x1234_5678u32));
        assert_eq!(0x4d88_cb9d_8848_8361, hash(0, &-2i64));
        assert_eq!(0x64d0_ccb7_54ae_815e, hash(0, &7usize));
        assert_eq!(
            0xffac_7836_bf9a_88e4,
            hash(0, &0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10u128)
        );
        assert_eq!(0x62c7_08bc_7e04_e7b1, hash(0, &1.5f32));
        assert_eq!(0x3766_3db0_029a_eb3d, hash(0, &-0.25f64));
        assert_eq!(0xf0a2_e69f_cd76_3b43, hash(0, &'\u{00E9}'));
        assert_eq!(0x6c53_bceb_bc76_3359, hash(0, &Some(1.5f32)));
        assert_eq!(0xfbe5_af10_e5f8_bd85, hash(0, &Option::<f32>::None));
        assert_eq!(0x1f99_ad67_8323_5999, hash(0, "abc"));
        assert_eq!(0x3045_c81e_05bf_f88b, hash(0, &[1u16, 2][..]));
        assert_eq!(0xfbe5_af10_e5f8_bd85, hash(0, &Kind::X));
        assert_eq!(0xd5b2_6ee4_85f2_a92d, hash(0, &Kind::Y));
        assert_eq!(0xc3e9_27b4_07f2_b4b3, hash(0, &[1u8, 2, 3]));
        assert_eq!(0xc562_1cb2_3e28_1dbf, hash(0, &structural(true)));
        assert_eq!(0x759a_2dab_8a7a_594d, hash(0, &structural(false)));
        assert_eq!(0x9bad_c291_da40_59c6, hash(42, "abc"));

        // > 48 bytes exercises the buffered/chunked update path; the pinned
        // value is the hash of the slice in one call, while this hashes each
        // byte individually.
        let long: Vec<u8> = (0..100).map(|i| i as u8).collect();
        assert_eq!(0x6191_9d3a_7952_af41, hash(0, &long));

        // One-shot `Wyhash::hash` vectors from the wyhash reference test table.
        assert_eq!(0x0409_638e_e2bd_e459, Wyhash::hash(0, b""));
        assert_eq!(0x32dd_92e4_b291_5153, Wyhash::hash(2, b"abc"));
        assert_eq!(
            0xff42_329b_90e5_0d58,
            Wyhash::hash(
                5,
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
            )
        );
    }

    #[test]
    fn structural_hash_ignores_padding_and_sees_content() {
        let one = structural(true);
        let mut two = structural(true);
        assert_eq!(hash(0, &one), hash(0, &two));
        two.name = "abd";
        assert_ne!(hash(0, &one), hash(0, &two));
        two.name = "abc";
        two.u = Tagged::N(3);
        assert_ne!(hash(0, &one), hash(0, &two));
    }

    /// The incremental API must agree with the one-shot hash for any chunking.
    #[test]
    fn incremental_matches_one_shot() {
        let data: Vec<u8> = (0..=255).map(|i| i as u8).collect();
        for len in 0..data.len() {
            let mut hasher = Wyhash::init(7);
            for chunk in data[..len].chunks(1) {
                hasher.update(chunk);
            }
            assert_eq!(Wyhash::hash(7, &data[..len]), hasher.finish());
            assert_eq!(hasher.finish(), hasher.finish());
        }
    }
}
