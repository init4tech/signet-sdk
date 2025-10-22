use crate::send::SignetEthBundle;
use alloy::primitives::U256;
use signet_evm::{DriveBundleResult, EvmErrored, EvmNeedsTx, SignetInspector, SignetLayered};
use signet_types::{AggregateFills, AggregateOrders, MarketError, SignedPermitError};
use std::borrow::Cow;
use tracing::{debug, error};
use trevm::{
    helpers::Ctx,
    inspectors::{Layered, TimeLimit},
    revm::{
        context::result::EVMError, inspector::InspectorEvmTr, Database, DatabaseCommit, Inspector,
    },
    trevm_bail, trevm_ensure, trevm_try, BundleDriver, BundleError,
};

/// Inspector used in the impl of [`BundleDriver`] for
/// [`SignetEthBundleDriver`].
pub type SignetEthBundleInsp<I> = Layered<TimeLimit, I>;

/// Errors while running a [`SignetEthBundle`] on the EVM.
#[derive(thiserror::Error)]
pub enum SignetEthBundleError<Db: Database> {
    /// Bundle error.
    #[error(transparent)]
    Bundle(#[from] BundleError<Db>),

    /// SignedPermitError.
    #[error(transparent)]
    SignetPermit(#[from] SignedPermitError),

    /// Contract error.
    #[error(transparent)]
    Contract(#[from] alloy::contract::Error),

    /// Market error.
    #[error(transparent)]
    Market(#[from] MarketError),
}

impl<Db: Database> core::fmt::Debug for SignetEthBundleError<Db> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignetEthBundleError::Bundle(bundle_error) => {
                f.debug_tuple("BundleError").field(bundle_error).finish()
            }
            SignetEthBundleError::SignetPermit(signed_order_error) => {
                f.debug_tuple("SignedPermitError").field(signed_order_error).finish()
            }
            SignetEthBundleError::Contract(contract_error) => {
                f.debug_tuple("ContractError").field(contract_error).finish()
            }
            SignetEthBundleError::Market(market_error) => {
                f.debug_tuple("MarketError").field(market_error).finish()
            }
        }
    }
}

impl<Db: Database> From<EVMError<Db::Error>> for SignetEthBundleError<Db> {
    fn from(err: EVMError<Db::Error>) -> Self {
        Self::Bundle(BundleError::from(err))
    }
}

/// Driver for applying a Signet Ethereum bundle to an EVM.
#[derive(Debug, Clone)]
pub struct SignetEthBundleDriver<'a, 'b> {
    /// The bundle to apply.
    bundle: &'a SignetEthBundle,

    /// Execution deadline for this bundle. This limits the total WALLCLOCK
    /// time spent simulating the bundle.
    deadline: std::time::Instant,

    /// Reference to the fill state to check against.
    fill_state: Cow<'b, AggregateFills>,

    // -- Accumulated outputs below here--
    /// Total gas used by this bundle during execution, an output of the driver.
    total_gas_used: u64,

    /// Beneficiary balance increase during execution, an output of the driver.
    beneficiary_balance_increase: U256,

    /// Running aggregate of fills during execution.
    bundle_fills: AggregateFills,

    /// Running aggregate of orders during execution.
    bundle_orders: AggregateOrders,
}

impl<'a, 'b> SignetEthBundleDriver<'a, 'b> {
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub fn new(bundle: &'a SignetEthBundle, deadline: std::time::Instant) -> Self {
        Self::new_with_fill_state(bundle, deadline, Default::default())
    }

    /// Creates a new [`SignetEthBundleDriver`] with the given bundle,
    /// response, and aggregate fills.
    ///
    /// This is useful for testing, and for combined host-rollup simulation.
    pub fn new_with_fill_state(
        bundle: &'a SignetEthBundle,
        deadline: std::time::Instant,
        fill_state: Cow<'b, AggregateFills>,
    ) -> Self {
        Self {
            bundle,
            deadline,
            fill_state,
            total_gas_used: 0,
            beneficiary_balance_increase: U256::ZERO,
            bundle_fills: AggregateFills::default(),
            bundle_orders: AggregateOrders::default(),
        }
    }

    /// Get a reference to the bundle.
    pub const fn bundle(&self) -> &SignetEthBundle {
        self.bundle
    }

    /// Get the deadline for this driver.
    pub const fn deadline(&self) -> std::time::Instant {
        self.deadline
    }

    /// Get the total gas used by this driver during tx execution.
    pub const fn total_gas_used(&self) -> u64 {
        self.total_gas_used
    }

    /// Get the beneficiary balance increase for this driver during tx execution.
    pub const fn beneficiary_balance_increase(&self) -> U256 {
        self.beneficiary_balance_increase
    }

    /// Take the aggregate orders and fills from this driver.
    pub fn take_aggregates(&mut self) -> (AggregateFills, AggregateOrders) {
        (std::mem::take(&mut self.bundle_fills), std::mem::take(&mut self.bundle_orders))
    }
}

impl<Db, Insp> BundleDriver<Db, SignetLayered<Layered<TimeLimit, Insp>>>
    for SignetEthBundleDriver<'_, '_>
where
    Db: Database + DatabaseCommit,
    Insp: Inspector<Ctx<Db>>,
{
    type Error = SignetEthBundleError<Db>;

    fn run_bundle(
        &mut self,
        mut trevm: EvmNeedsTx<Db, SignetEthBundleInsp<Insp>>,
    ) -> DriveBundleResult<Self, Db, SignetEthBundleInsp<Insp>> {
        let bundle = &self.bundle.bundle;

        // Reset the total gas used and beneficiary balance increase
        // to 0 before running the bundle.
        self.total_gas_used = 0;
        self.beneficiary_balance_increase = U256::ZERO;

        // Get the beneficiary address and its initial balance
        let beneficiary = trevm.beneficiary();
        let inital_beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        // Ensure that the bundle has transactions
        trevm_ensure!(!bundle.txs.is_empty(), trevm, BundleError::BundleEmpty.into());

        // Check if the block we're in is valid for this bundle. Both must match
        trevm_ensure!(
            trevm.block_number().to::<u64>() == bundle.block_number,
            trevm,
            BundleError::BlockNumberMismatch.into()
        );

        // Check if the state block number is valid (not 0, and not a tag)
        let timestamp = trevm.block_timestamp();
        trevm_ensure!(
            self.bundle.is_valid_at_timestamp(timestamp.to()),
            trevm,
            BundleError::TimestampOutOfRange.into()
        );

        // Decode and validate the transactions in the bundle
        let txs = trevm_try!(self.bundle.decode_and_validate_txs(), trevm);

        // We'll maintain running aggregates of fills and orders across
        // all transactions in the bundle.
        let mut bundle_fills = AggregateFills::default();
        let mut bundle_orders = AggregateOrders::default();

        for tx in txs.into_iter() {
            let _span = tracing::debug_span!("bundle_tx_loop", tx_hash = %tx.hash()).entered();

            // Update the inner deadline.
            let limit = trevm.inner_mut_unchecked().ctx_inspector().1.outer_mut().outer_mut();
            *limit = TimeLimit::new(self.deadline - std::time::Instant::now());

            let tx_hash = tx.hash();

            // Temporary rebinding of trevm within each loop iteration.
            // The type of t is `EvmTransacted`, while the type of trevm is
            // `EvmNeedsTx`.
            let mut t = trevm
                .run_tx(&tx)
                .map_err(EvmErrored::err_into)
                .inspect_err(|err| error!(err = %err.error(), "error while running transaction"))?;

            // Check the result of the transaction.
            let result = t.result();

            let gas_used = result.gas_used();

            // EVM Execution succeeded.
            // We now check if the orders are valid with the bundle's fills
            // state. If not, and the tx is not marked as revertible by the
            // bundle, we error our simulation.
            if result.is_success() {
                // Taking these clears the context for reuse.
                let (tx_fills, tx_orders) =
                    t.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

                // We accumulate the orders and fills from each transaction into
                // our running bundle aggregates.
                bundle_orders.absorb(&tx_orders);
                bundle_fills.absorb(&tx_fills);

                // Then we check that the fills are sufficient against the
                // provided fill state.
                if self.fill_state.check_ru_tx_events(&bundle_fills, &bundle_orders).is_err() {
                    debug!("transaction dropped due to insufficient fills");
                    if self.bundle.reverting_tx_hashes().contains(tx_hash) {
                        trevm = t.reject();
                        continue;
                    } else {
                        return Err(t.errored(BundleError::BundleReverted.into()));
                    }
                }

                self.total_gas_used = self.total_gas_used.saturating_add(gas_used);
            } else {
                // EVM Execution did not succeed.
                // If not success, we are in a revert or halt. If the tx is
                // not marked as revertible by the bundle, we error our
                // simulation.
                if !self.bundle.reverting_tx_hashes().contains(tx_hash) {
                    debug!("transaction reverted, not marked as revertible");
                    return Err(t.errored(BundleError::BundleReverted.into()));
                }
                self.total_gas_used = self.total_gas_used.saturating_add(gas_used);
            }

            // If we did not shortcut return/continue, we accept the state
            // changes from this transaction.
            trevm = t.accept_state()
        }

        let beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        self.beneficiary_balance_increase =
            beneficiary_balance.saturating_sub(inital_beneficiary_balance);

        Ok(trevm)
    }

    fn post_bundle(
        &mut self,
        _trevm: &EvmNeedsTx<Db, SignetEthBundleInsp<Insp>>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
