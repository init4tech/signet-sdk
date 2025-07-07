use crate::{Events, Extractable, ExtractedEvent, HasTxns};
use alloy::{
    consensus::{BlockHeader, TxReceipt},
    primitives::Log,
};
use signet_types::constants::SignetSystemConstants;

/// The extract step trait defines an object that can extract data from logs in
/// transaction receipts.
pub trait ExtractStep<C: Extractable> {
    type Extract: Sized + 'static;

    /// Check if the expected data can be extracted from the log, and return
    /// the extracted data if it can.
    fn extract_log(&self, log: &Log) -> Option<Self::Extract>;

    /// Extracts the expected data from a transaction receipt, returning an
    /// iterator over the extracted data. The iterator yields tuples
    /// containing the index of the log in the receipt and the extracted data.
    fn extract_receipt<'a, 'b, 'c>(
        &'a self,
        receipt: &'b C::Receipt,
    ) -> impl Iterator<Item = (usize, Self::Extract)> + 'c
    where
        'a: 'c,
        'b: 'c,
    {
        receipt.logs().iter().enumerate().filter_map(|(i, log)| {
            let log = self.extract_log(log)?;
            Some((i, log))
        })
    }

    /// Extracts the expected data from a block, returning an iterator over the
    /// extracted data. The iterator yields tuples containing the index of the
    /// log in the block and the extracted data.
    fn extract_block<'a, 'b, 'c>(
        &'a self,
        block: &'b C::Block,
        receipts: &'b [C::Receipt],
    ) -> impl Iterator<Item = ExtractedEvent<'c, C::Receipt, Self::Extract>>
    where
        'a: 'c,
        'b: 'c,
    {
        block.transactions().iter().zip(receipts.iter()).flat_map(|(tx, receipt)| {
            self.extract_receipt(receipt).map(move |(log_index, event)| ExtractedEvent {
                tx,
                receipt,
                log_index,
                event,
            })
        })
    }

    /// Extracts the expected data from a chain, returning an iterator over the
    /// extracted data. The iterator yields tuples containing the block and an
    /// iterator over events extracted from that block.
    fn extract<'a, 'b, 'c>(
        &'a self,
        extractable: &'b C,
    ) -> impl Iterator<
        Item = (&'b C::Block, impl Iterator<Item = ExtractedEvent<'c, C::Receipt, Self::Extract>>),
    >
    where
        C: HasTxns,
        'a: 'c,
        'b: 'c,
    {
        extractable
            .blocks_and_receipts()
            .map(|(block, receipts)| (block, self.extract_block(block, receipts)))
    }
}

impl<C> ExtractStep<C> for SignetSystemConstants
where
    C: Extractable,
{
    type Extract = Events;

    fn extract_log(&self, log: &Log) -> Option<Self::Extract> {
        if log.address == self.host_zenith() {
            return Events::decode_zenith(log, self.ru_chain_id());
        }
        if log.address == self.host_orders() {
            return Events::decode_orders(log, self.ru_chain_id());
        }
        if log.address == self.host_transactor() {
            return Events::decode_transactor(log, self.ru_chain_id());
        }
        if log.address == self.host_passage() {
            return Events::decode_passage(log, self.ru_chain_id());
        }
        None
    }

    fn extract<'a, 'b, 'c>(
        &'a self,
        extractable: &'b C,
    ) -> impl Iterator<
        Item = (&'b C::Block, impl Iterator<Item = ExtractedEvent<'c, C::Receipt, Self::Extract>>),
    >
    where
        C: HasTxns,
        'a: 'c,
        'b: 'c,
    {
        extractable
            .blocks_and_receipts()
            .filter(|(host_block, _)| host_block.number() > self.host_deploy_height())
            .map(|(host_block, receipts)| {
                (host_block, ExtractStep::<C>::extract_block(self, host_block, receipts))
            })
    }
}
