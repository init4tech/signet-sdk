macro_rules! run_tx {
    ($self:ident, $trevm:ident, $tx:expr, $sender:expr) => {{
        let trevm = $trevm.fill_tx($tx);

        let _guard = tracing::trace_span!("run_tx", block_env = ?trevm.block(), tx = ?$tx, tx_env = ?trevm.tx(), spec_id = ?trevm.spec_id()).entered();

        match trevm.run() {
            Ok(t) => {
                tracing::debug!("evm executed successfully");
                ControlFlow::Keep(t)
            },
            Err(e) => {
                if e.is_transaction_error() {
                    tracing::debug!(
                        err = %e.as_transaction_error().unwrap(),
                        "Discarding outcome due to execution error"
                    );
                    ControlFlow::Discard(e.discard_error())
                } else {
                    return Err(e.err_into());
                }
            }
        }
    }};
}

macro_rules! run_tx_early_return {
    ($self:ident, $trevm:ident, $tx:expr, $sender:expr) => {
        match run_tx!($self, $trevm, $tx, $sender) {
            ControlFlow::Discard(t) => return Ok(t),
            ControlFlow::Keep(t) => t,
        }
    };
}
