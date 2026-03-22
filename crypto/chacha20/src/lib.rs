#![no_std]

use crypto_framework::StreamCipher;

const C: [u32; 4] = [0x61707865, 0x3320646e, 0x79622d32, 0x6b206574];

macro_rules! quarter_round {
    ($matrix:ident, $a:expr, $b:expr, $c:expr, $d:expr) => {
        ($matrix[$a], $matrix[$b], $matrix[$c], $matrix[$d]) =
            quarter_round($matrix[$a], $matrix[$b], $matrix[$c], $matrix[$d]);
    };
}

fn quarter_round(mut a: u32, mut b: u32, mut c: u32, mut d: u32) -> (u32, u32, u32, u32) {
    /* a += b; d ^= a; d <<<= 16; */
    a = a.wrapping_add(b);
    d ^= a;
    d = d.rotate_left(16);

    /* c += d; b ^= c; b <<<= 12; */
    c = c.wrapping_add(d);
    b ^= c;
    b = b.rotate_left(12);

    /* a += b; d ^= a; d <<<= 8; */
    a = a.wrapping_add(b);
    d ^= a;
    d = d.rotate_left(8);

    /* c += d; b ^= c; b <<<= 7; */
    c = c.wrapping_add(d);
    b ^= c;
    b = b.rotate_left(7);

    (a, b, c, d)
}
pub struct ChaCha20 {
    key: [u32; 8],
    nonce: [u32; 3],
    counter: u32,
    state: [u8; 64],
    state_ptr: usize,
}

impl Default for ChaCha20 {
    fn default() -> Self {
        Self {
            key: [0; 8],
            nonce: [0; 3],
            counter: 0,
            state: [0; 64],
            state_ptr: 0,
        }
    }
}

impl ChaCha20 {
    fn update_state(&mut self) {
        let mut matrix = [0; 16];
        matrix[0..4].copy_from_slice(&C);
        matrix[4..12].copy_from_slice(&self.key);
        matrix[12] = self.counter;
        matrix[13..16].copy_from_slice(&self.nonce);
        let origin = matrix;

        for _ in 0..10 {
            quarter_round!(matrix, 0, 4, 8, 12);
            quarter_round!(matrix, 1, 5, 9, 13);
            quarter_round!(matrix, 2, 6, 10, 14);
            quarter_round!(matrix, 3, 7, 11, 15);
            quarter_round!(matrix, 0, 5, 10, 15);
            quarter_round!(matrix, 1, 6, 11, 12);
            quarter_round!(matrix, 2, 7, 8, 13);
            quarter_round!(matrix, 3, 4, 9, 14);
        }

        for (i, ele) in matrix.iter_mut().enumerate() {
            *ele = ele.wrapping_add(origin[i]);
            self.state[4 * i..4 * (i + 1)].copy_from_slice(&ele.to_le_bytes());
        }
    }
}

impl StreamCipher for ChaCha20 {
    fn key_size() -> usize
    where
        Self: Sized,
    {
        32
    }
    fn nonce_size() -> usize
    where
        Self: Sized,
    {
        12
    }
    fn set_key(&mut self, key: &[u8]) {
        assert_eq!(key.len(), Self::key_size());
        for (i, dword) in self.key.iter_mut().enumerate() {
            *dword = u32::from_le_bytes(key[4 * i..4 * (i + 1)].try_into().unwrap());
        }
    }
    fn set_nonce(&mut self, nonce: &[u8]) {
        assert_eq!(nonce.len(), Self::nonce_size());
        for (i, dword) in self.nonce.iter_mut().enumerate() {
            *dword = u32::from_le_bytes(nonce[4 * i..4 * (i + 1)].try_into().unwrap());
        }
    }
    fn encrypt(&mut self, mut block: &mut [u8]) {
        while !block.is_empty() {
            if self.state_ptr == 0 {
                self.update_state();
                self.counter += 1;
            }

            let xor_bytes = core::cmp::min(block.len(), 64 - self.state_ptr);
            for byte in block.iter_mut().take(xor_bytes) {
                *byte ^= self.state[self.state_ptr];
                self.state_ptr += 1;
            }
            block = &mut block[xor_bytes..];
            self.state_ptr %= 64;
        }
    }
}
