use crate::send::SignetEthBundle;
use alloy::{hex, primitives::U256};
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

/// The output of the [`SignetEthBundleDriver`].
#[derive(Debug)]
pub struct DriverOutput<Db, Insp>
where
    Db: Database,
    Insp: Inspector<Ctx<Db>>,
{
    /// The host evm used to run the bundle.
    pub host_evm: Option<signet_evm::EvmNeedsTx<Db, Insp>>,

    /// Total gas used by this bundle during execution, an output of the driver.
    pub total_gas_used: u64,

    /// Beneficiary balance increase during execution, an output of the driver.
    pub beneficiary_balance_increase: U256,

    /// Running aggregate of fills during execution.
    pub bundle_fills: AggregateFills,

    /// Running aggregate of orders during execution.
    pub bundle_orders: AggregateOrders,
}

impl<Db, Insp> DriverOutput<Db, Insp>
where
    Db: Database,
    Insp: Inspector<Ctx<Db>>,
{
    /// Increase the total gas used by the given amount.
    pub const fn use_gas(&mut self, gas: u64) {
        self.total_gas_used = self.total_gas_used.saturating_add(gas);
    }

    /// Absorb fills and orders into the running totals.
    pub fn absorb(&mut self, fills: &AggregateFills, orders: &AggregateOrders) {
        self.bundle_fills.absorb(fills);
        self.bundle_orders.absorb(orders);
    }

    /// Record an increase in the beneficiary balance.
    pub const fn record_beneficiary_increase(&mut self, increase: U256) {
        self.beneficiary_balance_increase =
            self.beneficiary_balance_increase.saturating_add(increase);
    }
}

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

    /// Host simulation error.
    #[error("{0}")]
    HostSimulation(&'static str),
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
            SignetEthBundleError::HostSimulation(msg) => {
                f.debug_tuple("HostSimulationError").field(msg).finish()
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
#[derive(Debug)]
pub struct SignetEthBundleDriver<'a, 'b, Db, Insp>
where
    Db: Database,
    Insp: Inspector<Ctx<Db>>,
{
    /// The bundle to apply.
    bundle: &'a SignetEthBundle,

    /// Reference to the fill state to check against.
    pub fill_state: Cow<'b, AggregateFills>,

    /// Execution deadline for this bundle. This limits the total WALLCLOCK
    /// time spent simulating the bundle.
    deadline: std::time::Instant,

    // -- Accumulated outputs below here--
    output: DriverOutput<Db, Insp>,
}

impl<'a, 'b, Db, Insp> SignetEthBundleDriver<'a, 'b, Db, Insp>
where
    Db: Database,
    Insp: Inspector<Ctx<Db>>,
{
    /// Creates a new [`SignetEthBundleDriver`] with the given bundle and
    /// response.
    pub fn new(
        bundle: &'a SignetEthBundle,
        host_evm: signet_evm::EvmNeedsTx<Db, Insp>,
        deadline: std::time::Instant,
    ) -> Self {
        Self::new_with_fill_state(bundle, host_evm, deadline, Default::default())
    }

    /// Creates a new [`SignetEthBundleDriver`] with the given bundle,
    /// response, and aggregate fills.
    ///
    /// This is useful for testing, and for combined host-rollup simulation.
    pub fn new_with_fill_state(
        bundle: &'a SignetEthBundle,
        host_evm: signet_evm::EvmNeedsTx<Db, Insp>,
        deadline: std::time::Instant,
        fill_state: Cow<'b, AggregateFills>,
    ) -> Self {
        Self {
            bundle,
            fill_state,
            deadline,
            output: DriverOutput {
                host_evm: Some(host_evm),
                total_gas_used: 0,
                beneficiary_balance_increase: U256::ZERO,
                bundle_fills: AggregateFills::default(),
                bundle_orders: AggregateOrders::default(),
            },
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
        self.output.total_gas_used
    }

    /// Get the beneficiary balance increase for this driver during tx execution.
    pub const fn beneficiary_balance_increase(&self) -> U256 {
        self.output.beneficiary_balance_increase
    }

    /// Take the aggregate orders and fills from this driver.
    pub fn into_outputs(self) -> DriverOutput<Db, Insp> {
        self.output
    }
}

impl<RuDb, HostDb, RuInsp, HostInsp> BundleDriver<RuDb, SignetLayered<Layered<TimeLimit, RuInsp>>>
    for SignetEthBundleDriver<'_, '_, HostDb, HostInsp>
where
    RuDb: Database + DatabaseCommit,
    RuInsp: Inspector<Ctx<RuDb>>,
    HostDb: Database + DatabaseCommit,
    HostInsp: Inspector<Ctx<HostDb>>,
{
    type Error = SignetEthBundleError<RuDb>;

    fn run_bundle(
        &mut self,
        mut trevm: EvmNeedsTx<RuDb, SignetEthBundleInsp<RuInsp>>,
    ) -> DriveBundleResult<Self, RuDb, SignetEthBundleInsp<RuInsp>> {
        let bundle = &self.bundle.bundle;
        // -- STATELESS CHECKS --

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
        let host_txs = trevm_try!(self.bundle.decode_and_validate_host_txs(), trevm);
        let txs = trevm_try!(self.bundle.decode_and_validate_txs(), trevm);

        // -- STATEFUL ACTIONS --

        // Get the beneficiary address and its initial balance
        let beneficiary = trevm.beneficiary();
        let inital_beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        // -- HOST PORTION --

        // We simply run all host transactions first, accumulating their state
        // changes into the host_evm's state. If any reverts, we error out the
        // simulation.
        for tx in host_txs.into_iter() {
            self.output.host_evm = Some(trevm_try!(
                self.output
                    .host_evm
                    .take()
                    .expect("host_evm missing")
                    .run_tx(&tx)
                    .and_then(|mut htrevm| {
                        let result = htrevm.result();
                        if let Some(output) = result.output()  {
                            if !result.is_success() {
                                debug!(output = hex::encode(&output), "host transaction reverted");
                            }
                        }

                        trevm_ensure!(
                            result.is_success(),
                            htrevm,
                            EVMError::Custom(format!("host transaction reverted"))
                        );

                        // The host fills go in the bundle fills.
                        let host_fills = htrevm
                            .inner_mut_unchecked()
                            .inspector
                            .as_mut_detector()
                            .take_aggregates()
                            .0;
                        self.output.bundle_fills.absorb(&host_fills);

                        Ok(htrevm.accept_state())
                    })
                    .map_err(|err| {
                        error!(err = %err.error(), err_dbg = ?err.error(), "error while running host transaction");
                        SignetEthBundleError::HostSimulation("host simulation error")
                    }),
                trevm
            ));
        }

        // -- ROLLUP PORTION --
        for tx in txs.into_iter() {
            let _span = tracing::debug_span!("bundle_tx_loop", tx_hash = %tx.hash()).entered();

            // Update the inner deadline.
            let limit = trevm.inner_mut_unchecked().ctx_inspector().1.outer_mut().outer_mut();
            *limit = TimeLimit::new(self.deadline - std::time::Instant::now());

            let tx_hash = tx.hash();

            // Temporary rebinding of trevm within each loop iteration.
            // The type of t is `EvmTransacted`, while the type of trevm is
            // `EvmNeedsTx`.
            let mut t = trevm.run_tx(&tx).map_err(EvmErrored::err_into).inspect_err(
                |err| error!(err = %err.error(), "error while running rollup transaction"),
            )?;

            // Check the result of the transaction.
            let result = t.result();
            let gas_used = result.gas_used();

            // EVM Execution succeeded.
            // We now check if the orders are valid with the bundle's fills
            // state. If not, and the tx is not marked as revertible by the
            // bundle, we error our simulation.
            if result.is_success() {
                let (tx_fills, tx_orders) =
                    t.inner_mut_unchecked().inspector.as_mut_detector().take_aggregates();

                // These clones are inefficient. We can optimize later if
                // needed.
                let mut candidate_fills = self.output.bundle_fills.clone();
                let mut candidate_orders = self.output.bundle_orders.clone();

                // The candidate is the updated
                candidate_fills.absorb(&tx_fills);
                candidate_orders.absorb(&tx_orders);

                // Then we check that the fills are sufficient against the
                // provided fill state. This does nothing on error.
                if self.fill_state.check_ru_tx_events(&candidate_fills, &candidate_orders).is_err()
                {
                    if self.bundle.reverting_tx_hashes().contains(tx_hash) {
                        debug!("transaction marked as revertible, reverting");
                        trevm = t.reject();
                        continue;
                    } else {
                        debug!("transaction dropped due to insufficient fills, not marked as revertible");
                        return Err(t.errored(BundleError::BundleReverted.into()));
                    }
                }

                // Now we accept the fills and order candidates
                self.output.bundle_fills = candidate_fills;
                self.output.bundle_orders = candidate_orders;
            } else {
                // EVM Execution did not succeed.
                // If not success, we are in a revert or halt. If the tx is
                // not marked as revertible by the bundle, we error our
                // simulation.
                if !self.bundle.reverting_tx_hashes().contains(tx_hash) {
                    debug!("transaction reverted, not marked as revertible");
                    return Err(t.errored(BundleError::BundleReverted.into()));
                }
            }

            // If we did not shortcut return/continue, we accept the state
            // changes from this transaction.
            self.output.use_gas(gas_used);
            trevm = t.accept_state()
        }

        // -- CLEANUP --

        let beneficiary_balance =
            trevm_try!(trevm.try_read_balance(beneficiary).map_err(EVMError::Database), trevm);

        self.output.record_beneficiary_increase(
            beneficiary_balance.saturating_sub(inital_beneficiary_balance),
        );

        Ok(trevm)
    }

    fn post_bundle(
        &mut self,
        _trevm: &EvmNeedsTx<RuDb, SignetEthBundleInsp<RuInsp>>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
