use alloy::{
    consensus::{
        constants::{ETH_TO_WEI, GWEI_TO_WEI},
        Header, ReceiptEnvelope, TxEip1559,
    },
    primitives::{Address, U256},
    signers::{local::PrivateKeySigner, Signature},
};
use signet_constants::SignetSystemConstants;
use signet_evm::SignetDriver;
use signet_extract::{Extractable, ExtractedEvent, Extracts};
use signet_test_utils::{
    chain::{fake_block, Chain, RU_CHAIN_ID},
    evm::test_signet_evm,
    specs::{make_wallet, sign_tx_with_key_pair, simple_send},
};
use signet_types::primitives::{RecoveredBlock, SealedHeader, TransactionSigned};
use signet_zenith::MINTER_ADDRESS;
use trevm::revm::database::in_memory_db::InMemoryDB;

struct TestEnv {
    pub wallets: Vec<PrivateKeySigner>,
    pub nonces: [u64; 10],
    pub sequence: u64,
}

impl TestEnv {
    fn new() -> Self {
        let wallets = (1..=10).map(make_wallet).collect::<Vec<_>>();

        Self { wallets, nonces: [0; 10], sequence: 1 }
    }

    fn driver<'a, 'b, C: Extractable>(
        &self,
        extracts: &'a mut Extracts<'b, C>,
        txns: Vec<TransactionSigned>,
    ) -> SignetDriver<'a, 'b, C> {
        let header = Header { gas_limit: 30_000_000, ..Default::default() };
        SignetDriver::new(
            extracts,
            txns.into(),
            SealedHeader::new(header),
            SignetSystemConstants::test(),
        )
    }

    fn trevm(&self) -> signet_evm::EvmNeedsBlock<InMemoryDB> {
        let mut trevm = test_signet_evm();
        for wallet in &self.wallets {
            let address = wallet.address();
            trevm.test_set_balance(address, U256::from(ETH_TO_WEI * 100));
        }
        trevm
    }

    /// Get the next zenith header in the sequence
    fn next_block(&mut self) -> RecoveredBlock {
        let block = fake_block(self.sequence);
        self.sequence += 1;
        block
    }

    fn signed_simple_send(&mut self, from: usize, to: Address, amount: U256) -> TransactionSigned {
        let wallet = &self.wallets[from];
        let tx = simple_send(to, amount, self.nonces[from], RU_CHAIN_ID);
        let tx = sign_tx_with_key_pair(wallet, tx);
        self.nonces[from] += 1;
        tx
    }
}

#[test]
fn test_simple_send() {
    let mut context = TestEnv::new();

    // Set up a simple transfer
    let to = Address::repeat_byte(2);
    let tx = context.signed_simple_send(0, to, U256::from(100));

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

    // Run the EVM
    let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    // Assert that the EVM balance increased
    assert_eq!(sealed_block.senders.len(), 1);
    assert_eq!(sealed_block.block.body.transactions().next(), Some(&tx));
    assert_eq!(receipts.len(), 1);

    assert_eq!(trevm.read_balance(to), U256::from(100));
}

#[test]
fn test_two_sends() {
    let mut context = TestEnv::new();

    // Set up a simple transfer
    let to = Address::repeat_byte(2);
    let tx1 = context.signed_simple_send(0, to, U256::from(100));

    let to2 = Address::repeat_byte(3);
    let tx2 = context.signed_simple_send(0, to2, U256::from(100));

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    let mut driver = context.driver(&mut extracts, vec![tx1.clone(), tx2.clone()]);

    // Run the EVM
    let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    // Assert that the EVM balance increased
    assert_eq!(sealed_block.senders.len(), 2);
    assert_eq!(sealed_block.block.body.transactions().collect::<Vec<_>>(), vec![&tx1, &tx2]);
    assert_eq!(receipts.len(), 2);

    assert_eq!(trevm.read_balance(to), U256::from(100));
    assert_eq!(trevm.read_balance(to2), U256::from(100));
}

#[test]
fn test_execute_two_blocks() {
    let mut context = TestEnv::new();
    let sender = context.wallets[0].address();

    let to = Address::repeat_byte(2);
    let tx = context.signed_simple_send(0, to, U256::from(100));

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

    // Run the EVM
    let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    assert_eq!(sealed_block.senders.len(), 1);
    assert_eq!(sealed_block.block.body.transactions().collect::<Vec<_>>(), vec![&tx]);
    assert_eq!(receipts.len(), 1);
    assert_eq!(trevm.read_balance(to), U256::from(100));
    assert_eq!(trevm.read_nonce(sender), 1);

    // Repeat the above for the next block
    // same recipient
    let tx = context.signed_simple_send(0, to, U256::from(100));

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    let mut driver = context.driver(&mut extracts, vec![tx.clone()]);

    // Run the EVM
    let mut trevm = trevm.drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    assert_eq!(sealed_block.senders.len(), 1);
    assert_eq!(sealed_block.block.body.transactions().collect::<Vec<_>>(), vec![&tx]);
    assert_eq!(receipts.len(), 1);
    assert_eq!(trevm.read_balance(to), U256::from(200));
}

#[test]
fn test_an_enter() {
    let mut context = TestEnv::new();
    let user = Address::repeat_byte(2);

    // Set up a fake event
    let fake_tx = fake_tx();
    let fake_receipt = ReceiptEnvelope::Eip1559(Default::default());

    let enter = signet_zenith::Passage::Enter {
        rollupChainId: U256::from(RU_CHAIN_ID),
        rollupRecipient: user,
        amount: U256::from(100),
    };

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    extracts.enters.push(ExtractedEvent {
        tx: &fake_tx,
        receipt: &fake_receipt,
        log_index: 0,
        event: enter,
    });
    let mut driver = context.driver(&mut extracts, vec![]);

    // Run the EVM
    let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    assert_eq!(sealed_block.senders.len(), 1);
    assert_eq!(
        sealed_block.block.body.transactions().collect::<Vec<_>>(),
        vec![&extracts.enters[0].make_transaction(0)]
    );
    assert_eq!(receipts.len(), 1);
    assert_eq!(trevm.read_balance(user), U256::from(100));
    assert_eq!(trevm.read_nonce(user), 0);
}

#[test]
fn test_a_transact() {
    let mut context = TestEnv::new();
    let sender = Address::repeat_byte(1);
    let recipient = Address::repeat_byte(2);

    // Set up a couple fake events
    let fake_tx = fake_tx();
    let fake_receipt = ReceiptEnvelope::Eip1559(Default::default());

    let enter = signet_zenith::Passage::Enter {
        rollupChainId: U256::from(RU_CHAIN_ID),
        rollupRecipient: sender,
        amount: U256::from(ETH_TO_WEI),
    };

    let transact = signet_zenith::Transactor::Transact {
        rollupChainId: U256::from(RU_CHAIN_ID),
        sender,
        to: recipient,
        data: Default::default(),
        value: U256::from(100),
        gas: U256::from(21_000),
        maxFeePerGas: U256::from(GWEI_TO_WEI),
    };

    // Setup the driver
    let block = context.next_block();
    let mut extracts = Extracts::<Chain>::empty(&block);
    extracts.enters.push(ExtractedEvent {
        tx: &fake_tx,
        receipt: &fake_receipt,
        log_index: 0,
        event: enter,
    });
    extracts.transacts.push(ExtractedEvent {
        tx: &fake_tx,
        receipt: &fake_receipt,
        log_index: 0,
        event: transact,
    });

    let mut driver = context.driver(&mut extracts, vec![]);

    // Run the EVM
    let mut trevm = context.trevm().drive_block(&mut driver).unwrap();
    let (sealed_block, receipts) = driver.finish();

    assert_eq!(sealed_block.senders, vec![MINTER_ADDRESS, sender]);
    assert_eq!(
        sealed_block.block.body.transactions().collect::<Vec<_>>(),
        vec![&extracts.enters[0].make_transaction(0), &extracts.transacts[0].make_transaction(0)]
    );
    assert_eq!(receipts.len(), 2);
    assert_eq!(trevm.read_balance(recipient), U256::from(100));
}

fn fake_tx() -> TransactionSigned {
    let tx = TxEip1559::default();
    let signature = Signature::test_signature();
    TransactionSigned::new_unhashed(tx.into(), signature)
}
