use reth::revm::precompile::{secp256r1, Precompiles};
use std::sync::OnceLock;

static PRECOMPILES: OnceLock<Precompiles> = OnceLock::new();

/// Create a set of precompiles for the Signet EVM.
pub fn signet_precompiles() -> &'static Precompiles {
    PRECOMPILES.get_or_init(|| {
        let mut precompiles = Precompiles::prague().clone();

        precompiles.extend([secp256r1::P256VERIFY]);

        precompiles
    })
}
