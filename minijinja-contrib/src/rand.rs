use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use minijinja::value::{Object, ObjectRepr};
use minijinja::State;

#[derive(Debug)]
pub struct XorShiftRng {
    seed: AtomicU64,
}

impl Object for XorShiftRng {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }
}

impl XorShiftRng {
    pub fn for_state(state: &State) -> Arc<XorShiftRng> {
        state.get_or_set_temp_object("minijinja-contrib-rng", || {
            XorShiftRng::new(
                state
                    .lookup("RAND_SEED")
                    .and_then(|x| u64::try_from(x).ok()),
            )
        })
    }

    pub fn new(seed: Option<u64>) -> XorShiftRng {
        XorShiftRng {
            seed: AtomicU64::from(
                seed.unwrap_or_else(|| RandomState::new().build_hasher().finish()),
            ),
        }
    }

    pub fn next(&self) -> u64 {
        let mut rv = self.seed.load(Ordering::Relaxed);
        rv ^= rv << 13;
        rv ^= rv >> 7;
        rv ^= rv << 17;
        self.seed.store(rv, Ordering::Relaxed);
        rv
    }

    pub fn next_usize(&self, max: usize) -> usize {
        (self.random() * max as f64) as usize
    }

    pub fn random(&self) -> f64 {
        (self.next() as f64) / (u64::MAX as f64)
    }

    pub fn random_range(&self, lower: i64, upper: i64) -> i64 {
        (self.random() * (upper - lower) as f64) as i64 + lower
    }
}
