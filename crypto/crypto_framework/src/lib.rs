#![no_std]

pub trait BlockCipher {
    /** block size in bytes */
    const BLOCK_SIZE: usize;
    /** key size in bytes */
    const KEY_SIZE: usize;
    fn set_key(&mut self, key: &[u8]);
    fn block_encrypt(&mut self, block: &mut [u8]);
    fn block_decrypt(&mut self, block: &mut [u8]);
}

pub struct CbcCipher<B: BlockCipher, const S: usize> {
    block_cipher: B,
    prev_block: [u8; S],
}

impl<B, const S: usize> Default for CbcCipher<B, S>
where
    B: BlockCipher + Default,
{
    fn default() -> Self {
        Self {
            block_cipher: B::default(),
            prev_block: [0; S],
        }
    }
}

impl<B, const S: usize> CbcCipher<B, S>
where
    B: BlockCipher + Default,
{
    pub fn set_iv(&mut self, iv: &[u8]) {
        self.prev_block.copy_from_slice(iv);
    }
}

impl<B, const S: usize> BlockCipher for CbcCipher<B, S>
where
    B: BlockCipher + Default,
{
    const BLOCK_SIZE: usize = B::BLOCK_SIZE;
    const KEY_SIZE: usize = B::KEY_SIZE;
    fn set_key(&mut self, key: &[u8]) {
        self.block_cipher.set_key(key);
    }
    fn block_encrypt(&mut self, block: &mut [u8]) {
        for (i, byte) in block.iter_mut().enumerate() {
            *byte ^= self.prev_block[i];
        }
        self.block_cipher.block_encrypt(block);
        self.prev_block = block.try_into().unwrap();
    }
    fn block_decrypt(&mut self, block: &mut [u8]) {
        let prev_block = block.try_into().unwrap();
        self.block_cipher.block_decrypt(block);
        for (i, byte) in block.iter_mut().enumerate() {
            *byte ^= self.prev_block[i];
        }
        self.prev_block = prev_block;
    }
}

pub trait StreamCipher {
    /** key size in bytes */
    const KEY_SIZE: usize;
    /** nonce size in bytes */
    const NONCE_SIZE: usize;
    fn set_key(&mut self, key: &[u8]);
    fn set_nonce(&mut self, key: &[u8]);
    fn encrypt(&mut self, block: &mut [u8]);
    fn decrypt(&mut self, block: &mut [u8]) {
        self.encrypt(block);
    }
}

pub trait CryptoRandgen {
    /** seed size in bytes */
    const SEED_SIZE: usize;
    fn reseed(&mut self, seed: &[u8]);
    fn gen_bytes(&mut self, buf: &mut [u8]);
}

impl<T: StreamCipher> CryptoRandgen for T {
    const SEED_SIZE: usize = T::KEY_SIZE + T::NONCE_SIZE;
    fn reseed(&mut self, seed: &[u8]) {
        self.set_key(&seed[..T::KEY_SIZE]);
        self.set_nonce(&seed[T::KEY_SIZE..]);
    }
    fn gen_bytes(&mut self, buf: &mut [u8]) {
        self.encrypt(buf);
    }
}

pub trait Hash {
    /** digest length in bytes */
    const DIGEST_LEN: usize;
    fn update(&mut self, message: &[u8]);
    fn digest(&mut self, sum: &mut [u8]);
}

/**
 * Keyed-Hashing for Message Authentication defined in <https://datatracker.ietf.org/doc/html/rfc2104>.
*/
pub struct Hmac<H>
where
    H: Hash,
{
    ihasher: H,
    ohasher: H,
}

impl<H> Default for Hmac<H>
where
    H: Hash + Default,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<H> Hmac<H>
where
    H: Hash + Default,
{
    pub fn new() -> Self {
        Self {
            ihasher: H::default(),
            ohasher: H::default(),
        }
    }
    /**
     * * `key`: length of key must be equal to digest length.
     */
    pub fn set_key(&mut self, key: &[u8]) {
        for byte in key {
            let ipad = 0x36 ^ *byte;
            self.ihasher.update(&[ipad]);

            let opad = 0x5c ^ *byte;
            self.ohasher.update(&[opad]);
        }
    }
    pub fn update(&mut self, message: &[u8]) {
        self.ihasher.update(message);
    }
    pub fn digest(&mut self, sum: &mut [u8]) {
        assert_eq!(sum.len(), H::DIGEST_LEN);

        self.ihasher.digest(sum);
        self.ohasher.update(sum);
        self.ohasher.digest(sum);
    }
}
