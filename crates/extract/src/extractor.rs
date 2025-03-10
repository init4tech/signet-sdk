use crate::{Events, ExtractedEvent, Extracts};
use alloy::primitives::{Log, LogData};
use reth::{
    primitives::{Block, Receipt, RecoveredBlock},
    providers::Chain,
};
use signet_types::{MarketContext, SignetSystemConstants};
use tracing::debug_span;
use zenith_types::Passage;

/// Extracts Zenith events from a chain.
#[derive(Debug, Clone, Copy)]
pub struct Extractor {
    constants: SignetSystemConstants,
}

impl Extractor {
    /// Create a new [`Extractor`] from system constants.
    pub const fn new(constants: &SignetSystemConstants) -> Self {
        Self { constants: *constants }
    }

    /// Extract a [`Events`] from a log, checking the chain ID.
    fn extract_log(&self, log: &Log<LogData>) -> Option<Events> {
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
    fn extract_receipt<'a: 'c, 'b: 'c, 'c>(
        &'a self,
        receipt: &'b Receipt,
    ) -> impl Iterator<Item = (usize, Events)> + 'c {
        receipt.logs.iter().enumerate().filter_map(|(i, log)| {
            let log = self.extract_log(log)?;
            Some((i, log))
        })
    }

    /// Extract blocks from a chain.
    fn produce_event_extracts<'a: 'c, 'b: 'c, 'c>(
        &'a self,
        block: &'b RecoveredBlock<Block>,
        receipts: &'b [Receipt],
    ) -> impl Iterator<Item = ExtractedEvent<'c>> {
        block.body().transactions.iter().zip(receipts.iter()).flat_map(|(tx, receipt)| {
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
    pub fn get_zenith_outputs<'a: 'c, 'b: 'c, 'c>(
        &'a self,
        chain: &'b Chain,
    ) -> impl Iterator<Item = Extracts<'c>> {
        chain
            .blocks_and_receipts()
            .filter(|(block, _)| block.number > self.constants.host_deploy_height())
            .map(move |(block, receipts)| {
                let height = block.number;
                let ru_height = self.constants.host_block_to_rollup_block_num(height).unwrap();
                let host_block = block;
                let mut context = MarketContext::new();
                let mut enters = vec![];
                let mut transacts = vec![];
                let mut enter_tokens: Vec<ExtractedEvent<'c, Passage::EnterToken>> = vec![];
                let mut submitted = None;

                for event in self.produce_event_extracts(block, receipts) {
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
                            tracing::debug!("filling host swap");
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
