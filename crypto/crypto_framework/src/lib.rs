#![no_std]

pub trait BlockCipher {
    /** block size in bytes */
    fn block_size() -> usize
    where
        Self: Sized;
    /** key size in bytes */
    fn key_size() -> usize
    where
        Self: Sized;
    fn set_key(&mut self, key: &[u8]);
    fn block_encrypt(&self, block: &mut [u8]);
    fn block_decrypt(&self, block: &mut [u8]);
}

pub trait Hash {
    /** digest length in bytes */
    fn digest_length() -> usize
    where
        Self: Sized;
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
        assert_eq!(sum.len(), H::digest_length());

        self.ihasher.digest(sum);
        self.ohasher.update(sum);
        self.ohasher.digest(sum);
    }
}
