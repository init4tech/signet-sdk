#![recursion_limit = "256"]
//! Integration tests for [`Filler`].
use alloy::{
    consensus::{Transaction, TxEnvelope},
    eips::eip2718::Decodable2718,
    primitives::{Address, U256},
    sol_types::SolCall,
};
use chrono::Utc;
use futures_util::TryStreamExt;
use signet_orders::{
    FeePolicySubmitter, FillSubmitter, Filler, FillerError, FillerOptions, OrdersAndFills,
};
use signet_test_utils::{
    orders::{
        default_test_orders, mock_tx_builder, MockBundleSubmitter, MockFillSubmitter,
        MockOrderSource, TestOrderBuilder,
    },
    test_constants::TEST_SYS,
    users::TEST_SIGNERS,
};
use signet_types::SignedFill;
use signet_zenith::RollupOrders::{fillPermit2Call, initiatePermit2Call};

#[tokio::test]
async fn get_orders_returns_stream_from_source() {
    let original_orders = default_test_orders().await;

    let source = MockOrderSource::new(original_orders.clone());
    let submitter = MockFillSubmitter::new();
    let filler = Filler::new(&TEST_SIGNERS[1], source, submitter, TEST_SYS, FillerOptions::new());

    let orders: Vec<_> = filler.get_orders().try_collect().await.unwrap();
    assert_eq!(orders, original_orders);
}

#[tokio::test]
async fn sign_fills_creates_fills_for_orders() {
    async fn sign_fills(options: FillerOptions) -> [SignedFill; 2] {
        let original_orders = default_test_orders().await;
        let filler_key = &TEST_SIGNERS[1];
        let source = MockOrderSource::empty();
        let submitter = MockFillSubmitter::new();
        let filler = Filler::new(filler_key.clone(), source, submitter, TEST_SYS, options);

        let orders_and_fills = filler.sign_fills(original_orders.clone()).await.unwrap();

        assert_eq!(orders_and_fills.orders(), original_orders);
        assert_eq!(orders_and_fills.signer_address(), filler_key.address());
        let host_fill = orders_and_fills.fills().get(&TEST_SYS.host_chain_id()).unwrap().clone();
        let ru_fill = orders_and_fills.fills().get(&TEST_SYS.ru_chain_id()).unwrap().clone();
        [host_fill, ru_fill]
    }

    // Account for clock slewing by allowing the deadline to be within ±2 seconds of the expected
    // value.
    fn assert_deadline_within_range(actual_secs: U256, expected_secs: i64) {
        let actual = i64::try_from(actual_secs).unwrap();
        let lower_bound = expected_secs - 2;
        let upper_bound = expected_secs + 2;
        assert!(
            actual_secs > lower_bound && actual_secs < upper_bound,
            "actual deadline {actual} not in expected range ({lower_bound}, {upper_bound})"
        );
    }

    // Test using default `FillerOptions`.
    let [host_fill, ru_fill] = sign_fills(FillerOptions::new()).await;
    // With default filler options, the deadline should be `Utc::now()` + 12s, and the nonce should
    // be `Utc::now()`.
    let now = Utc::now();
    assert_deadline_within_range(host_fill.permit.permit.deadline, now.timestamp() + 12);
    assert_deadline_within_range(ru_fill.permit.permit.deadline, now.timestamp() + 12);
    let actual_host_nonce = i64::try_from(host_fill.permit.permit.nonce).unwrap();
    let lower_bound = now.timestamp_micros() - 2_000_000;
    let upper_bound = now.timestamp_micros() + 2_000_000;
    assert!(
        actual_host_nonce > lower_bound && actual_host_nonce < upper_bound,
        "actual host nonce {actual_host_nonce} not in expected range ({lower_bound}, {upper_bound})"
    );
    let actual_ru_nonce = i64::try_from(host_fill.permit.permit.nonce).unwrap();
    assert!(
        actual_ru_nonce > lower_bound && actual_ru_nonce < upper_bound,
        "actual rollup nonce {actual_ru_nonce} not in expected range ({lower_bound}, {upper_bound})"
    );

    // Test using non-default `FillerOptions`.
    let filler_options = FillerOptions::new().with_deadline_offset(100).with_nonce(200);
    let [host_fill, ru_fill] = sign_fills(filler_options).await;
    assert_eq!(host_fill.permit.permit.nonce, U256::from(200));
    assert_eq!(ru_fill.permit.permit.nonce, U256::from(200));
    assert_deadline_within_range(host_fill.permit.permit.deadline, Utc::now().timestamp() + 100);
    assert_deadline_within_range(ru_fill.permit.permit.deadline, Utc::now().timestamp() + 100);
}

#[tokio::test]
async fn fill_submits_signed_fills() {
    let orders = default_test_orders().await;
    let filler_key = TEST_SIGNERS[1].clone();

    // Create mock providers using the filler key (must match the signer used for fills).
    // MockTxBuilder pre-fills gas and nonce locally, so we only need to push block number.
    let ru_provider = mock_tx_builder(filler_key.clone(), TEST_SYS.ru_chain_id());
    let host_provider = mock_tx_builder(filler_key.clone(), TEST_SYS.host_chain_id());

    // Push block number response for target block calculation
    ru_provider.asserter().push_success(&U256::from(100));

    let bundle_submitter = MockBundleSubmitter::new();
    let fee_policy_submitter =
        FeePolicySubmitter::new(ru_provider, host_provider, bundle_submitter.clone(), TEST_SYS);

    let source = MockOrderSource::empty();
    let filler = Filler::new(
        filler_key.clone(),
        source,
        fee_policy_submitter,
        TEST_SYS,
        FillerOptions::new(),
    );

    filler.fill(orders).await.unwrap();

    let bundles = bundle_submitter.submitted_bundles();
    assert_eq!(bundles.len(), 1);
    // Bundle should have 3 rollup txs (1 fill + 2 initiates) and 1 host tx (1 fill)
    assert_eq!(bundles[0].bundle.txs.len(), 3);
    assert_eq!(bundles[0].host_txs().len(), 1);

    // Verify transaction order: fill must come before initiates
    let rollup_txs = &bundles[0].bundle.txs;
    for (i, tx_bytes) in rollup_txs.iter().enumerate() {
        let envelope = TxEnvelope::decode_2718(&mut tx_bytes.as_ref()).unwrap();
        let input = envelope.input();
        let selector: [u8; 4] = input[..4].try_into().unwrap();
        if i == 0 {
            assert_eq!(selector, fillPermit2Call::SELECTOR);
        } else {
            assert_eq!(selector, initiatePermit2Call::SELECTOR);
        }
    }

    // Verify host tx is also a fill
    let host_tx_bytes = &bundles[0].host_txs()[0];
    let host_envelope = TxEnvelope::decode_2718(&mut host_tx_bytes.as_ref()).unwrap();
    let host_selector: [u8; 4] = host_envelope.input()[..4].try_into().unwrap();
    assert_eq!(host_selector, fillPermit2Call::SELECTOR);
}

#[tokio::test]
async fn fill_with_empty_orders_returns_error() {
    let filler_key = &TEST_SIGNERS[1];
    let source = MockOrderSource::empty();
    let submitter = MockFillSubmitter::new();
    let filler = Filler::new(filler_key.clone(), source, submitter, TEST_SYS, FillerOptions::new());

    let result = filler.fill(vec![]).await;
    assert!(matches!(result, Err(FillerError::NoOrders)));
}

#[tokio::test]
async fn submission_error_propagates() {
    #[derive(Debug, Clone)]
    struct FailingSubmitter;

    #[derive(Debug, thiserror::Error)]
    #[error("fill submission failed")]
    struct FillSubmissionError;

    impl FillSubmitter for FailingSubmitter {
        type Response = ();
        type Error = FillSubmissionError;

        async fn submit_fills(&self, _: OrdersAndFills) -> Result<(), Self::Error> {
            Err(FillSubmissionError)
        }
    }

    let signer = &TEST_SIGNERS[0];

    let order = TestOrderBuilder::new()
        .with_input(Address::ZERO, U256::from(1000))
        .with_output(
            Address::repeat_byte(0x55),
            U256::from(500),
            signer.address(),
            TEST_SYS.host_chain_id(),
        )
        .sign(signer)
        .await;

    let filler = Filler::new(
        TEST_SIGNERS[1].clone(),
        MockOrderSource::empty(),
        FailingSubmitter,
        TEST_SYS,
        FillerOptions::new(),
    );

    let FillerError::Submission(inner) = filler.fill(vec![order]).await.unwrap_err() else {
        panic!("expected Submission error");
    };
    inner.downcast_ref::<FillSubmissionError>().unwrap();
}
