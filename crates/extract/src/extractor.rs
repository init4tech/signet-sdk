use crate::{r#trait::Extractable, ExtractStep, Extracts};
use alloy::consensus::BlockHeader;
use signet_types::constants::SignetSystemConstants;

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
#[derive(Debug, Clone)]
pub struct Extractor {
    constants: SignetSystemConstants,
}

impl From<SignetSystemConstants> for Extractor {
    fn from(constants: SignetSystemConstants) -> Self {
        Self { constants }
    }
}

impl From<Extractor> for SignetSystemConstants {
    fn from(extractor: Extractor) -> Self {
        extractor.constants
    }
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
        self.constants.extract(chain).map(move |(host_block, events)| {
            let host_height = host_block.number();
            let ru_height = self
                .constants
                .host_block_to_rollup_block_num(host_height)
                .expect("checked by filter");

            let mut extracts = Extracts::new(
                self.constants.host_chain_id(),
                host_block,
                self.constants.ru_chain_id(),
                ru_height,
            );

            events.for_each(|e| {
                extracts.ingest_event(e);
            });

            extracts
        })
    }
}
