use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};

use minijinja::State;

#[derive(Debug)]
pub struct XorShiftRng {
    seed: u64,
}

impl XorShiftRng {
    pub fn for_state<'a>(state: &'a mut State<'_, '_>) -> &'a mut XorShiftRng {
        if state.get_extension::<XorShiftRng>().is_none() {
            let seed = state
                .lookup("RAND_SEED")
                .and_then(|x| u64::try_from(x).ok());
            state.get_or_insert_extension(XorShiftRng::new(seed));
        }
        state.get_extension_mut().unwrap()
    }

    pub fn new(seed: Option<u64>) -> XorShiftRng {
        XorShiftRng {
            seed: seed.unwrap_or_else(|| RandomState::new().build_hasher().finish()),
        }
    }

    pub fn next(&mut self) -> u64 {
        self.seed ^= self.seed << 13;
        self.seed ^= self.seed >> 7;
        self.seed ^= self.seed << 17;
        self.seed
    }

    pub fn next_usize(&mut self, max: usize) -> usize {
        (self.random() * max as f64) as usize
    }

    pub fn random(&mut self) -> f64 {
        (self.next() as f64) / (u64::MAX as f64)
    }

    pub fn random_range(&mut self, lower: i64, upper: i64) -> i64 {
        (self.random() * (upper - lower) as f64) as i64 + lower
    }
}
