use crate::{
    chain::Chain,
    specs::{NotificationSpec, NotificationWithSidecars, RuBlockSpec},
};
use alloy::{
    consensus::{
        constants::GWEI_TO_WEI, BlobTransactionSidecar, Header, Receipt, ReceiptEnvelope,
        TxEip1559, TxEip4844,
    },
    primitives::{Address, Bytes, FixedBytes, Log, LogData, B256, U256},
    signers::Signature,
};
use signet_evm::ExecutionOutcome;
use signet_extract::{Events, Extractable, Extracts};
use signet_types::primitives::{
    BlockBody, RecoveredBlock, SealedBlock, SealedHeader, Transaction, TransactionSigned,
};
use signet_types::{
    constants::{KnownChains, ParseChainError, SignetSystemConstants},
    AggregateFills,
};
use signet_zenith::{Passage, RollupOrders, Transactor};
use std::{
    borrow::Borrow,
    str::FromStr,
    sync::atomic::{AtomicU64, Ordering},
};

/// A block spec for the host chain.
///
/// Typically this should be used as follows:
///
/// 1. Instantiate with a [`SignetSystemConstants`] object via [`Self::new`].
/// 2. Add Signet events to the block with  [`Self::enter`],
///    [`Self::enter_token`] [`Self::transact`], and [`Self::fill`].
/// 3. (optional) Add a [`RuBlockSpec`] with [`Self::submit_block`].
/// 4. Convert to a [`Chain`] with [`Self::to_chain`], or a
///    [`NotificationSpec`] with [`Self::to_commit_notification_spec`].
#[derive(Debug)]
pub struct HostBlockSpec {
    /// The system constants for the block.
    pub constants: SignetSystemConstants,

    /// The Zenith-event receipts in the block.
    pub receipts: Vec<ReceiptEnvelope>,
    /// The Ru block associated with this host block (if any).
    pub ru_block: Option<RuBlockSpec>,
    /// The sidecar associated with the Ru block (if any).
    pub sidecar: Option<BlobTransactionSidecar>,
    /// The receipt for the Ru block (if any).
    pub ru_block_receipt: Option<ReceiptEnvelope>,
    /// The block number. This will be overridden when making chains of blocks.
    pub block_number: AtomicU64,

    /// The events that were used to create this block.
    pub events: Vec<Events>,
}

impl Clone for HostBlockSpec {
    fn clone(&self) -> Self {
        Self {
            constants: self.constants,
            receipts: self.receipts.clone(),
            ru_block: self.ru_block.clone(),
            sidecar: self.sidecar.clone(),
            ru_block_receipt: self.ru_block_receipt.clone(),
            block_number: self.block_number().into(),
            events: self.events.clone(),
        }
    }
}

impl From<SignetSystemConstants> for HostBlockSpec {
    fn from(constants: SignetSystemConstants) -> Self {
        Self::new(constants)
    }
}

impl HostBlockSpec {
    /// Make a new block spec
    pub const fn new(constants: SignetSystemConstants) -> Self {
        Self {
            constants,
            receipts: vec![],
            ru_block: None,
            sidecar: None,
            ru_block_receipt: None,
            block_number: AtomicU64::new(0),
            events: vec![],
        }
    }

    /// Make a new block spec with pecorino constants.
    pub const fn pecorino() -> Self {
        Self::new(SignetSystemConstants::pecorino())
    }

    /// Make a new block spec with test constants.
    pub const fn test() -> Self {
        Self::new(SignetSystemConstants::test())
    }

    /// Set the block number.
    pub fn with_block_number(self, block_number: u64) -> Self {
        self.block_number.store(block_number, Ordering::Relaxed);
        self
    }

    /// Add an enter to the host block.
    pub fn enter_token(mut self, recipient: Address, amount: usize, token: Address) -> Self {
        let e = Passage::EnterToken {
            rollupChainId: U256::from(self.constants.ru_chain_id()),
            rollupRecipient: recipient,
            amount: U256::from(amount),
            token,
        };
        self.receipts.push(to_receipt(self.constants.host_passage(), &e));
        self.events.push(e.into());

        self
    }

    /// Add an ignored enter token to the host block
    pub fn ingnored_enter_token(mut self, recipient: Address, amount: u64, token: Address) -> Self {
        self.receipts.push(to_receipt(
            self.constants.host_passage(),
            &Passage::EnterToken {
                rollupChainId: U256::ZERO,
                rollupRecipient: recipient,
                amount: U256::from(amount),
                token,
            },
        ));
        self
    }

    /// Add an iter of enter tokens to the host block
    pub fn enter_tokens<'a, T>(mut self, enter_tokens: impl IntoIterator<Item = T>) -> Self
    where
        T: Borrow<(Address, usize, Address)> + 'a,
    {
        for item in enter_tokens {
            let enter_token = item.borrow();
            self = self.enter_token(enter_token.0, enter_token.1, enter_token.2);
        }
        self
    }

    /// Add an enter to the host block.
    pub fn enter(mut self, recipient: Address, amount: usize) -> Self {
        let e = Passage::Enter {
            rollupChainId: U256::from(self.constants.ru_chain_id()),
            rollupRecipient: recipient,
            amount: U256::from(amount),
        };
        self.receipts.push(to_receipt(self.constants.host_passage(), &e));
        self.events.push(e.into());

        self
    }

    /// Add an enter to the host block that is ignored by the Ru chain because
    /// it has a mismatched RU chain ID.
    pub fn ignored_enter(mut self, recipient: Address, amount: u64) -> Self {
        self.receipts.push(to_receipt(
            self.constants.host_passage(),
            &Passage::Enter {
                rollupChainId: U256::ZERO,
                rollupRecipient: recipient,
                amount: U256::from(amount),
            },
        ));
        self
    }

    /// Add several enters to the host block.
    pub fn enters<'a, T>(mut self, enters: impl IntoIterator<Item = T>) -> Self
    where
        T: Borrow<(Address, usize)> + 'a,
    {
        for item in enters {
            let enter = item.borrow();
            self = self.enter(enter.0, enter.1);
        }
        self
    }

    /// Add a transact to the host block
    pub fn transact(mut self, t: Transactor::Transact) -> Self {
        self.receipts.push(to_receipt(self.constants.host_transactor(), &t));
        self.events.push(t.into());
        self
    }

    /// Add a simple transact to the host block.
    pub fn simple_transact(
        self,
        sender: Address,
        target: Address,
        data: impl AsRef<[u8]>,
        value: usize,
    ) -> Self {
        let transact = Transactor::Transact {
            rollupChainId: U256::from(self.constants.ru_chain_id()),
            sender,
            to: target,
            data: Bytes::copy_from_slice(data.as_ref()),
            value: U256::from(value),
            gas: U256::from(100_000),
            maxFeePerGas: U256::from(GWEI_TO_WEI),
        };
        self.transact(transact)
    }

    /// Add a fill to the host block
    pub fn fill(mut self, token: Address, recipient: Address, amount: u64) -> Self {
        let e = RollupOrders::Filled {
            outputs: vec![RollupOrders::Output {
                chainId: self.constants.ru_chain_id() as u32,
                token,
                recipient,
                amount: U256::from(amount),
            }],
        };
        self.receipts.push(to_receipt(self.constants.host_orders(), &e));
        self.events.push(e.into());
        self
    }

    /// Add a fill to the host block that is ignored by the Ru chain because
    /// it has a mismatched RU chain ID.
    pub fn ignored_fill(mut self, token: Address, recipient: Address, amount: u64) -> Self {
        self.receipts.push(to_receipt(
            self.constants.host_orders(),
            &RollupOrders::Filled {
                outputs: vec![RollupOrders::Output {
                    chainId: 0,
                    token,
                    recipient,
                    amount: U256::from(amount),
                }],
            },
        ));
        self
    }

    /// Add a block submitted to the host block
    pub fn submit_block(mut self, ru_block: RuBlockSpec) -> Self {
        let (bs, sidecar) = ru_block.to_block_submitted();

        self.ru_block = Some(ru_block);
        self.ru_block_receipt = Some(to_receipt(self.constants.host_zenith(), &bs));
        self.sidecar = Some(sidecar);
        self
    }

    /// Make a blob txn
    fn blob_txn(&self) -> Option<TransactionSigned> {
        let sidecar = self.sidecar.as_ref()?;

        Some(TransactionSigned::new_unhashed(
            Transaction::Eip4844(TxEip4844 {
                chain_id: self.constants.host_chain_id(),
                nonce: 0,
                gas_limit: 100_000,
                max_fee_per_gas: 100_000,
                max_priority_fee_per_gas: 10_000,
                to: self.constants.host_zenith(),
                value: U256::ZERO,
                access_list: Default::default(),
                blob_versioned_hashes: sidecar.versioned_hashes().collect(),
                max_fee_per_blob_gas: 100_000,
                input: Bytes::default(),
            }),
            Signature::test_signature(),
        ))
    }

    /// Make dummy txns. The blob txn will always be at the end of the block
    fn make_txns(&self) -> Vec<TransactionSigned> {
        self.receipts
            .iter()
            .map(|_| {
                Some(
                    alloy::consensus::Signed::new_unhashed(
                        TxEip1559::default(),
                        Signature::test_signature(),
                    )
                    .into(),
                )
            })
            .chain(std::iter::once(self.blob_txn()))
            .flatten()
            .collect()
    }

    /// Get the block number
    pub fn block_number(&self) -> u64 {
        self.block_number.load(Ordering::Relaxed)
    }

    /// Set the block number
    pub fn set_block_number(&self, block_number: u64) {
        self.block_number.store(block_number, Ordering::Relaxed);
    }

    /// Make a header
    ///
    /// This function is a little weird because reth @ 1.2.0 rejiggered the
    /// block structs in odd ways.
    pub fn header(&self) -> SealedHeader {
        let header = Header {
            difficulty: U256::from(0x4000_0000),
            number: self.block_number(),
            mix_hash: B256::repeat_byte(0xed),
            nonce: FixedBytes::repeat_byte(0xbe),
            timestamp: 1716555586, // the time when i wrote this function lol
            excess_blob_gas: Some(0),
            ..Default::default()
        };
        SealedHeader::new(header)
    }

    /// Make a block
    pub fn sealed_block(&self) -> SealedBlock {
        let header = self.header();

        let body = BlockBody { transactions: self.make_txns(), ommers: vec![], withdrawals: None };

        SealedBlock::new_unchecked(header, body)
    }

    /// Make a block with senders
    ///
    /// This function is a little weird because reth @ 1.2.0 rejiggered the
    /// block structs in odd ways.
    pub fn recovered_block(&self) -> RecoveredBlock {
        let block = self.sealed_block();
        let senders = block.body.transactions().map(|_| Address::ZERO).collect::<Vec<_>>();

        RecoveredBlock::new(block, senders)
    }

    /// Make an execution outcome
    pub fn execution_outcome(&self) -> ExecutionOutcome {
        let mut receipts = vec![self.receipts.clone()];
        if let Some(receipt) = self.ru_block_receipt.clone() {
            receipts.first_mut().unwrap().push(receipt);
        }

        ExecutionOutcome::new(Default::default(), receipts, self.block_number())
    }

    /// Make a chain
    pub fn to_chain(&self) -> (Chain, Option<BlobTransactionSidecar>) {
        let execution_outcome = self.execution_outcome();

        let chain = Chain::from_block(self.recovered_block(), execution_outcome);

        (chain, self.sidecar.clone())
    }

    /// Make a commit notification spec
    pub fn to_commit_notification_spec(&self) -> NotificationSpec {
        NotificationSpec { old: vec![], new: vec![self.clone()] }
    }

    /// Make a reorg notification with sidecars
    pub fn to_notification_with_sidecar(&self) -> NotificationWithSidecars {
        self.to_commit_notification_spec().to_exex_notification()
    }

    /// Make a revert notification spec
    pub fn to_revert_notification_spec(&self) -> NotificationSpec {
        NotificationSpec { old: vec![self.clone()], new: vec![] }
    }

    /// Assert that the block conforms to the spec
    pub fn assert_conforms<C: Extractable>(&self, extracts: &Extracts<'_, C>) {
        if let Some(ru_block) = &self.ru_block {
            ru_block.assert_conforms(extracts);
        }

        let mut enters = extracts.enters();
        let mut enter_tokens = extracts.enter_tokens();
        let mut transacts = extracts.transacts();

        let mut context = AggregateFills::new();

        for event in self.events.iter() {
            match event {
                Events::Enter(e) => {
                    assert_eq!(&(enters.next().expect("iter ended early")), e);
                }
                Events::EnterToken(e) => {
                    assert_eq!(&(enter_tokens.next().expect("iter ended early")), e);
                }
                Events::Transact(e) => {
                    assert_eq!(transacts.next().expect("iter ended early"), e);
                }
                Events::Filled(e) => {
                    context.add_fill(self.constants.host_chain_id(), e);
                }
                Events::BlockSubmitted(_) => {}
            }
        }
        assert!(enters.next().is_none());
        assert!(enter_tokens.next().is_none());
        assert!(transacts.next().is_none());
        assert_eq!(extracts.aggregate_fills(), context);
    }
}

impl FromStr for HostBlockSpec {
    type Err = ParseChainError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let chain: KnownChains = s.parse()?;
        match chain {
            KnownChains::Pecorino => Ok(Self::pecorino()),
            KnownChains::Test => Ok(Self::test()),
        }
    }
}

fn to_receipt<T>(address: Address, t: &T) -> ReceiptEnvelope
where
    for<'a> &'a T: Into<LogData>,
{
    let log = Log { address, data: t.into() };
    ReceiptEnvelope::Eip1559(
        Receipt { status: true.into(), cumulative_gas_used: 30_000, logs: vec![log] }.into(),
    )
}
