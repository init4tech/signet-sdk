pub mod chain;
pub mod contracts;
pub mod evm;
pub mod orders;
pub mod specs;
pub mod users;

pub use signet_constants::test_utils as test_constants;

/// Initialize tracing for tests. This is just for local debugging purposes.
pub fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::filter::EnvFilter::from_default_env())
        .try_init()
        .unwrap();
}
