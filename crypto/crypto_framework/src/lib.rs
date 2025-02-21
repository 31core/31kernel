#![no_std]

pub trait BlockCipher {
    fn block_size() -> usize;
    fn key_size() -> usize;
    fn set_key(&mut self, key: &[u8]);
    fn block_encrypt(&self, block: &mut [u8]);
    fn block_decrypt(&self, block: &mut [u8]);
}

pub trait Hash {
    fn digest_length() -> usize;
    fn update(&mut self, message: &[u8]);
    fn digest(&self, sum: &mut [u8]);
}
