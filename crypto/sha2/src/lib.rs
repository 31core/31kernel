#![no_std]

use crypto_framework::Hash;

const SHA256_DIGEST_LEN: usize = 32;

const K_TABLE: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

#[inline]
fn ch(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (!x & z)
}

#[inline]
fn maj(x: u32, y: u32, z: u32) -> u32 {
    (x & y) ^ (x & z) ^ (y & z)
}

#[inline]
fn bsig0(x: u32) -> u32 {
    (x.rotate_right(2)) ^ (x.rotate_right(13)) ^ (x.rotate_right(22))
}

#[inline]
fn bsig1(x: u32) -> u32 {
    (x.rotate_right(6)) ^ (x.rotate_right(11)) ^ (x.rotate_right(25))
}

#[inline]
fn ssig0(x: u32) -> u32 {
    (x.rotate_right(7)) ^ (x.rotate_right(18)) ^ (x >> 3)
}

#[inline]
fn ssig1(x: u32) -> u32 {
    (x.rotate_right(17)) ^ (x.rotate_right(19)) ^ (x >> 10)
}

pub struct Sha256 {
    message_len: usize,
    w_table: [u32; 64],
    bytes_remain: [u8; 64],
    bytes_remain_len: usize,
    h: [u32; 8],
}

impl Default for Sha256 {
    fn default() -> Self {
        Self {
            message_len: 0,
            w_table: [0; 64],
            bytes_remain: [0; 64],
            bytes_remain_len: 0,
            #[rustfmt::skip]
            h: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
        }
    }
}

impl Hash for Sha256 {
    fn digest_length() -> usize {
        SHA256_DIGEST_LEN
    }
    fn update(&mut self, mut message: &[u8]) {
        self.message_len += 8 * message.len();

        if self.bytes_remain_len > 0 {
            if self.bytes_remain_len + message.len() >= 64 {
                let mut chunk = [0; 64];
                chunk[..self.bytes_remain_len]
                    .copy_from_slice(&self.bytes_remain[..self.bytes_remain_len]);
                chunk[self.bytes_remain_len..]
                    .copy_from_slice(&message[..64 - self.bytes_remain_len]);
                self.update_w_table(&chunk);
                self.update_h();
                message = &message[..64 - self.bytes_remain_len];
                self.bytes_remain_len = 0;
            } else {
                self.bytes_remain[self.bytes_remain_len..self.bytes_remain_len + message.len()]
                    .copy_from_slice(message);
                self.bytes_remain_len += message.len();
                return;
            }
        }

        while message.len() >= 64 {
            self.update_w_table(&message[..64]);
            self.update_h();
            message = &message[64..];
        }

        if !message.is_empty() {
            self.bytes_remain[..message.len()].copy_from_slice(message);
            self.bytes_remain_len = message.len();
        }
    }
    fn digest(&mut self, sum: &mut [u8]) {
        if self.bytes_remain_len > 64 - 9 {
            let mut chunk = [0; 64];
            chunk[..self.bytes_remain_len]
                .copy_from_slice(&self.bytes_remain[..self.bytes_remain_len]);
            chunk[self.bytes_remain_len] = 0x80;
            self.update_w_table(&chunk);
            self.update_h();

            let mut chunk = [0; 64];
            chunk[56..64].copy_from_slice(&(self.message_len as u64).to_be_bytes());
            self.update_w_table(&chunk);
            self.update_h();
        } else {
            let mut chunk = [0; 64];
            chunk[..self.bytes_remain_len]
                .copy_from_slice(&self.bytes_remain[..self.bytes_remain_len]);
            chunk[self.bytes_remain_len] = 0x80;
            chunk[56..64].copy_from_slice(&(self.message_len as u64).to_be_bytes());
            self.update_w_table(&chunk);
            self.update_h();
        }

        for (i, word) in self.h.iter().enumerate() {
            sum[4 * i..4 * (i + 1)].copy_from_slice(&word.to_be_bytes());
        }
    }
}

impl Sha256 {
    fn update_w_table(&mut self, mut message: &[u8]) {
        for word in self.w_table.iter_mut().take(16) {
            *word = u32::from_be_bytes(message[..4].try_into().unwrap());
            message = &message[4..];
        }

        for w_table_ptr in 16..64 {
            self.w_table[w_table_ptr] = self.w_table[w_table_ptr - 16]
                + ssig0(self.w_table[w_table_ptr - 15])
                + self.w_table[w_table_ptr - 7]
                + ssig1(self.w_table[w_table_ptr - 2]);
        }
    }
    fn update_h(&mut self) {
        let mut h = self.h;
        for (round, k_r) in K_TABLE.iter().enumerate() {
            let ch = ch(h[4], h[5], h[6]);
            let temp1 = h[7] + bsig1(h[4]) + ch + k_r + self.w_table[round];
            let maj = maj(h[0], h[1], h[2]);
            let temp2 = bsig0(h[0]) + maj;

            h[7] = h[6];
            h[6] = h[5];
            h[5] = h[4];
            h[4] = h[3] + temp1;
            h[3] = h[2];
            h[2] = h[1];
            h[1] = h[0];
            h[0] = temp1 + temp2;
        }

        for (i, h_i) in self.h.iter_mut().enumerate() {
            *h_i += h[i];
        }
    }
}
