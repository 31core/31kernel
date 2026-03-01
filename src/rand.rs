/*!
 *  Random generator abstract and implementations.
 */

use core::{mem::MaybeUninit, ops::Range};

const N: usize = 624;
const M: usize = 397;
const W: u32 = 32;
const R: u32 = 31;
const UMASK: u32 = 0xffffffff << R;
const LMASK: u32 = 0xffffffff >> (W - R);
const A: u32 = 0x9908b0df;
const U: usize = 11;
const S: usize = 7;
const T: usize = 15;
const L: usize = 18;
const B: u32 = 0x9d2c5680;
const C: u32 = 0xefc60000;
const F: u32 = 1812433253;

pub static mut GLOBAL_RNG: MaybeUninit<MT19937> = MaybeUninit::uninit();

const SEED: u32 = 0;

pub fn rand_init() {
    unsafe { GLOBAL_RNG = MaybeUninit::new(MT19937::new(SEED)) };
}

pub trait RandomGenerator {
    /**
     * Update seed.
     */
    fn seed(&mut self, seed: u32);
    fn random_uint32(&mut self) -> u32;
    /**
     * Generate a random with a range.
     */
    fn range_uint32(&mut self, range: Range<u32>) -> u32 {
        let rand = self.random_uint32();
        let range_delta = range.end - range.start;

        range.start + rand % range_delta
    }
    /**
     * Generate random bytes.
     */
    fn gen_bytes(&mut self, buf: &mut [u8]) {
        let mut ptr = 0;
        let buf_size = buf.len();

        while ptr < buf_size {
            let end = core::cmp::min(buf_size, ptr + 4);
            buf[ptr..end].copy_from_slice(&self.random_uint32().to_be_bytes()[..end - ptr]);
            ptr += 4;
        }
    }
}

/**
 * An MT19937 random generator
*/
pub struct MT19937 {
    /** the array for the state vector */
    state_array: [u32; N],
    /** index into state vector array, 0 <= state_index <= n-1 always */
    state_index: usize,
}

impl MT19937 {
    pub fn new(seed: u32) -> Self {
        let mut state = Self {
            state_array: [0; N],
            state_index: 0,
        };

        state.seed(seed);

        state
    }
}

impl RandomGenerator for MT19937 {
    fn seed(&mut self, mut seed: u32) {
        self.state_array[0] = seed;

        for i in 1..N {
            seed = F * (seed ^ (seed >> (W - 2))) + i as u32; // Knuth TAOCP Vol2. 3rd Ed. P.106 for multiplier.
            self.state_array[i] = seed;
        }
    }
    fn random_uint32(&mut self) -> u32 {
        let mut k = self.state_index; // point to current state location

        let y = (self.state_array[k] & UMASK) | (self.state_array[(k + 1) % N] & LMASK);

        let a = if y & 1 == 1 { (y >> 1) ^ A } else { y >> 1 };

        let y = self.state_array[(k + M) % N] ^ a; // compute next value in the state
        self.state_array[k] = y; // update new state value

        k += 1;
        self.state_index = k % N;

        /* tempering */
        let mut y = y ^ (y >> U);
        y = y ^ ((y << S) & B);
        y = y ^ ((y << T) & C);
        y ^ (y >> L)
    }
}
