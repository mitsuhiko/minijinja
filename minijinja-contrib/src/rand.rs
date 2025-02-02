use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Arc;

use minijinja::value::{Object, ObjectRepr};

#[derive(Debug)]
pub struct XorShiftRng {
    seed: u64,
}

impl Object for XorShiftRng {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }
}

impl XorShiftRng {
    pub fn new(seed: Option<u64>) -> XorShiftRng {
        XorShiftRng {
            seed: match seed {
                Some(seed) => seed,
                None => RandomState::new().build_hasher().finish(),
            },
        }
    }

    pub fn next(&mut self) -> u64 {
        let mut rv = self.seed;
        rv ^= rv << 13;
        rv ^= rv >> 7;
        rv ^= rv << 17;
        self.seed = rv;
        rv
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
