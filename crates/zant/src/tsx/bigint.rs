//! A minimal arbitrary-precision unsigned integer for the JS number helpers
//! (`parseInt`, `toFixed`, `toPrecision`): exact decimal conversion of binary
//! floating-point values and of long integer prefixes.
//!
//! Rust has no big integer types and the workspace has no `num-bigint`
//! dependency, so this module carries the few operations
//! the helpers need (multiply by a small value, shift left by bits, decimal
//! conversion). It stays internal to `tsx`.

/// Little-endian 64-bit limbs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BigUint {
    limbs: Vec<u64>,
}

impl BigUint {
    pub(crate) fn from_u64(value: u64) -> BigUint {
        let mut limbs = vec![value];
        BigUint::trim(&mut limbs);
        BigUint { limbs }
    }

    fn trim(limbs: &mut Vec<u64>) {
        while limbs.len() > 1 && limbs.last() == Some(&0) {
            limbs.pop();
        }
    }

    pub(crate) fn is_zero(&self) -> bool {
        self.limbs == [0]
    }

    /// `self *= factor` (the limb-wise carry math of a small multiplication).
    pub(crate) fn mul_u64(&mut self, factor: u64) {
        let mut carry: u128 = 0;
        for limb in &mut self.limbs {
            let product = u128::from(*limb) * u128::from(factor) + carry;
            *limb = product as u64;
            carry = product >> 64;
        }
        while carry != 0 {
            self.limbs.push(carry as u64);
            carry >>= 64;
        }
        BigUint::trim(&mut self.limbs);
    }

    /// `self += value`.
    pub(crate) fn add_u64(&mut self, value: u64) {
        let mut carry = u128::from(value);
        for limb in &mut self.limbs {
            let sum = u128::from(*limb) + carry;
            *limb = sum as u64;
            carry = sum >> 64;
            if carry == 0 {
                break;
            }
        }
        if carry != 0 {
            self.limbs.push(carry as u64);
        }
    }

    /// `self <<= bits`.
    pub(crate) fn shl(&mut self, bits: usize) {
        if bits == 0 || self.is_zero() {
            return;
        }
        let limb_shift = bits / 64;
        let bit_shift = bits % 64;
        if bit_shift != 0 {
            let mut carry: u64 = 0;
            for limb in &mut self.limbs {
                let next = *limb >> (64 - bit_shift);
                *limb = (*limb << bit_shift) | carry;
                carry = next;
            }
            if carry != 0 {
                self.limbs.push(carry);
            }
        }
        if limb_shift > 0 {
            let mut limbs = vec![0; limb_shift];
            limbs.extend_from_slice(&self.limbs);
            self.limbs = limbs;
        }
        BigUint::trim(&mut self.limbs);
    }

    /// `self /= divisor`, returning the remainder.
    pub(crate) fn div_rem_u64(&mut self, divisor: u64) -> u64 {
        let mut remainder: u128 = 0;
        for limb in self.limbs.iter_mut().rev() {
            let value = (remainder << 64) | u128::from(*limb);
            *limb = (value / u128::from(divisor)) as u64;
            remainder = value % u128::from(divisor);
        }
        BigUint::trim(&mut self.limbs);
        remainder as u64
    }

    /// Parses decimal digits (no sign, empty is zero).
    pub(crate) fn from_decimal(digits: &str) -> BigUint {
        let mut value = BigUint::from_u64(0);
        for byte in digits.bytes() {
            debug_assert!(byte.is_ascii_digit());
            value.mul_u64(10);
            value.add_u64(u64::from(byte - b'0'));
        }
        value
    }

    /// The value in base 10, most significant digit first ("0" for zero).
    pub(crate) fn to_decimal(&self) -> String {
        let mut value = self.clone();
        let mut digits = Vec::new();
        while !value.is_zero() {
            digits.push(b'0' + value.div_rem_u64(10) as u8);
        }
        if digits.is_empty() {
            return String::from("0");
        }
        digits.reverse();
        String::from_utf8(digits).expect("decimal digits are ASCII")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_and_decimal_conversion() {
        let mut value = BigUint::from_u64(1);
        value.shl(100);
        assert_eq!(value.to_decimal(), "1267650600228229401496703205376");
        value.mul_u64(5);
        value.add_u64(7);
        assert_eq!(
            value.to_decimal(),
            "6338253001141147007483516026887".to_string()
        );
        let mut small = BigUint::from_u64(0);
        assert_eq!(small.to_decimal(), "0");
        small.shl(200);
        assert!(small.is_zero());
        small.add_u64(42);
        assert_eq!(small.to_decimal(), "42");
        // 2^64 needs two limbs
        let mut wide = BigUint::from_u64(u64::MAX);
        wide.add_u64(1);
        assert_eq!(wide.to_decimal(), "18446744073709551616");
    }
}
