//! The `random` filter as well as the `randrange` and `lipsum` functions
//! use a simple pseudo random number generator.  It can be seeded with the
//! `RAND_SEED` global.  Otherwise the seed is derived from the random keys of
//! the standard library's hash maps or a custom [seed source](set_seed_source).
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::RwLock;

use minijinja::State;

static SEED_SOURCE: RwLock<Option<fn() -> u64>> = RwLock::new(None);

/// Sets a function that provides seeds when `RAND_SEED` is not set.
///
/// By default seeds are derived from the random keys of the standard
/// library's hash maps.  On platforms without a source of randomness
/// (such as `wasm32-unknown-unknown`) these are not random, so this can be
/// used to provide a better source.  Pass `None` to restore the default.
pub fn set_seed_source(source: Option<fn() -> u64>) {
    *SEED_SOURCE.write().unwrap_or_else(|err| err.into_inner()) = source;
}

fn default_seed() -> u64 {
    match *SEED_SOURCE.read().unwrap_or_else(|err| err.into_inner()) {
        Some(source) => source(),
        None => RandomState::new().build_hasher().finish(),
    }
}

#[derive(Debug)]
pub(crate) struct XorShiftRng {
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
            seed: seed.unwrap_or_else(default_seed),
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
