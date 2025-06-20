use crate::{
    r#trait::{Extractable, HasTxns},
    Events, ExtractedEvent, Extracts,
};
use alloy::{
    consensus::{BlockHeader, TxReceipt},
    primitives::Log,
};
use signet_types::{constants::SignetSystemConstants, AggregateFills};
use tracing::debug_span;

/// Extracts Zenith events from a chain.
///
/// The extractor is a newtype around the [`SignetSystemConstants`], which
/// contain all necessary information for extracting events from a chain.
///
/// The extractor contains a series of inner iterators that traverse chains,
/// blocks, and receipts to extract signet-relevant events. These events are
/// represented as [`ExtractedEvent`] objects containing [`Events`]. One
/// [`Extracts`] will be produced for each block in the input chain, provided
/// that Signet was deployed at that height.
#[derive(Debug, Clone, Copy)]
pub struct Extractor {
    constants: SignetSystemConstants,
}

impl Extractor {
    /// Create a new [`Extractor`] from system constants.
    pub const fn new(constants: SignetSystemConstants) -> Self {
        Self { constants }
    }

    /// Get the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Extract a [`Events`] from a log, checking the chain ID.
    fn extract_log(&self, log: &Log) -> Option<Events> {
        if log.address == self.constants.host_zenith() {
            return Events::decode_zenith(log, self.constants.ru_chain_id());
        }
        if log.address == self.constants.host_orders() {
            return Events::decode_orders(log, self.constants.ru_chain_id());
        }
        if log.address == self.constants.host_transactor() {
            return Events::decode_transactor(log, self.constants.ru_chain_id());
        }
        if log.address == self.constants.host_passage() {
            return Events::decode_passage(log, self.constants.ru_chain_id());
        }
        None
    }

    /// Extract Zenith events from a receipt.
    fn extract_receipt<'a: 'c, 'b: 'c, 'c, R: TxReceipt<Log = Log>>(
        &'a self,
        receipt: &'b R,
    ) -> impl Iterator<Item = (usize, Events)> + 'c {
        receipt.logs().iter().enumerate().filter_map(|(i, log)| {
            let log = self.extract_log(log)?;
            Some((i, log))
        })
    }

    /// Extract blocks from a chain.
    fn produce_event_extracts<'a: 'c, 'b: 'c, 'c, C: Extractable>(
        &'a self,
        block: &'b C::Block,
        receipts: &'b [C::Receipt],
    ) -> impl Iterator<Item = ExtractedEvent<'c, C::Receipt>> {
        block.transactions().iter().zip(receipts.iter()).flat_map(|(tx, receipt)| {
            self.extract_receipt(receipt).map(move |(log_index, event)| ExtractedEvent {
                tx,
                receipt,
                log_index,
                event,
            })
        })
    }

    /// Get the Zenith outputs from a chain. This function does the following:
    /// - Filter blocks at or before the host deploy height.
    /// - For each unfiltered block:
    ///     - Extract the Zenith events from the block.
    ///     - Accumulate the fills.
    ///     - Associate each event with block, tx and receipt references.
    ///     - Yield the extracted block info.
    pub fn extract_signet<'a: 'c, 'b: 'c, 'c, C: Extractable>(
        &'a self,
        chain: &'b C,
    ) -> impl Iterator<Item = Extracts<'c, C>> {
        chain
            .blocks_and_receipts()
            .filter(|(block, _)| block.number() > self.constants.host_deploy_height())
            .map(move |(block, receipts)| {
                let height = block.number();
                let ru_height = self.constants.host_block_to_rollup_block_num(height).unwrap();
                let host_block = block;
                let mut context = AggregateFills::new();
                let mut enters = vec![];
                let mut transacts = vec![];
                let mut enter_tokens = vec![];
                let mut submitted = None;

                for event in self.produce_event_extracts::<C>(block, receipts) {
                    let _span = debug_span!(
                        "tx_loop",
                        height,
                        tx_hash = %event.tx_hash(),
                    );

                    match event.event {
                        Events::Enter(_) => {
                            enters.push(event.try_into_enter().expect("checked by match guard"))
                        }
                        Events::Filled(fill) => {
                            // Fill the swap, ignoring overflows
                            // host swaps are pre-filtered to only include the
                            // host chain, so no need to check the chain id
                            context.add_fill(self.constants.host_chain_id(), &fill);
                        }
                        Events::Transact(_) => transacts
                            .push(event.try_into_transact().expect("checked by match guard")),
                        Events::BlockSubmitted(_) => {
                            submitted = Some(
                                event.try_into_block_submitted().expect("checked by match guard"),
                            );
                        }
                        Events::EnterToken(enter) => {
                            if self.constants.is_host_token(enter.token) {
                                enter_tokens.push(
                                    event.try_into_enter_token().expect("checked by match guard"),
                                );
                            }
                        }
                    }
                }

                Extracts {
                    host_block,
                    chain_id: self.constants.ru_chain_id(),
                    ru_height,
                    submitted,
                    enters,
                    transacts,
                    enter_tokens,
                    context,
                }
            })
    }
}
