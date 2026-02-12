use crate::{FillSubmitter, OrderSource};
use alloy::{primitives::Address, signers::Signer};
use chrono::Utc;
use futures_util::{Stream, StreamExt};
use signet_constants::SignetSystemConstants;
use signet_types::{SignedFill, SignedOrder, SigningError, UnsignedFill};
use std::collections::HashMap;
use tracing::instrument;

/// Errors returned by [`Filler`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum FillerError {
    /// Order source error.
    #[error("failed to get orders: {0}")]
    Source(#[source] Box<dyn core::error::Error + Send + Sync>),
    /// No orders to fill.
    #[error("no orders to fill")]
    NoOrders,
    /// Failed to sign fills for orders.
    #[error("failed to sign fills: {0}")]
    Signing(#[from] SigningError),
    /// Fill submission failed.
    #[error("failed to submit fills: {0}")]
    Submission(#[source] Box<dyn core::error::Error + Send + Sync>),
}

/// Options for configuring the [`Filler`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FillerOptions {
    /// Optional deadline offset in seconds for fills.
    pub deadline_offset: Option<u64>,
    /// Optional nonce to use for permit2 signatures.
    pub nonce: Option<u64>,
}

impl FillerOptions {
    /// Create a new [`FillerOptions`] with default values.
    pub const fn new() -> Self {
        Self { deadline_offset: None, nonce: None }
    }

    /// Set the deadline offset.
    pub const fn with_deadline_offset(mut self, offset: u64) -> Self {
        self.deadline_offset = Some(offset);
        self
    }

    /// Set the nonce.
    pub const fn with_nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }
}

/// A small struct to ensure the relevant orders remain paired with the fills generated from them
/// and with the signer's address.
#[derive(Debug, Clone)]
pub struct OrdersAndFills {
    pub(crate) orders: Vec<SignedOrder>,
    pub(crate) fills: HashMap<u64, SignedFill>,
    pub(crate) signer_address: Address,
}

impl OrdersAndFills {
    /// Get the orders.
    pub fn orders(&self) -> &[SignedOrder] {
        &self.orders
    }

    /// Get the fills.
    pub const fn fills(&self) -> &HashMap<u64, SignedFill> {
        &self.fills
    }

    /// Get the signer address.
    pub const fn signer_address(&self) -> Address {
        self.signer_address
    }
}

/// Fills orders by fetching from a source, signing fills, and submitting them.
///
/// `Filler` is generic over:
/// - `Sign`: A [`Signer`] for signing fills
/// - `Source`: An [`OrderSource`] for fetching orders
/// - `Submit`: A [`FillSubmitter`] for submitting signed fills
#[derive(Debug, Clone)]
pub struct Filler<Sign, Source, Submit> {
    signer: Sign,
    order_source: Source,
    submitter: Submit,
    constants: SignetSystemConstants,
    options: FillerOptions,
}

impl<Sign, Source, Submit> Filler<Sign, Source, Submit> {
    /// Create a new filler instance.
    pub const fn new(
        signer: Sign,
        order_source: Source,
        submitter: Submit,
        constants: SignetSystemConstants,
        options: FillerOptions,
    ) -> Self {
        Self { signer, order_source, submitter, constants, options }
    }

    /// Get a reference to the signer.
    pub const fn signer(&self) -> &Sign {
        &self.signer
    }

    /// Get a reference to the order source.
    pub const fn order_source(&self) -> &Source {
        &self.order_source
    }

    /// Get a reference to the submitter.
    pub const fn submitter(&self) -> &Submit {
        &self.submitter
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }

    /// Get a reference to the filler options.
    pub const fn options(&self) -> &FillerOptions {
        &self.options
    }
}

impl<Sign, Source, Submit> Filler<Sign, Source, Submit>
where
    Source: OrderSource + Send + Sync,
{
    /// Query the source for signed orders.
    pub fn get_orders(
        &self,
    ) -> impl Stream<Item = Result<SignedOrder, FillerError>> + Send + use<'_, Sign, Source, Submit>
    {
        self.order_source
            .get_orders()
            .map(|result| result.map_err(|e| FillerError::Source(Box::new(e))))
    }
}

impl<Sign, Source, Submit> Filler<Sign, Source, Submit>
where
    Sign: Signer + Send + Sync,
{
    /// Sign fills for the given orders.
    ///
    /// Returns a map of chain ID to signed fill for each target chain.
    pub async fn sign_fills(
        &self,
        orders: Vec<SignedOrder>,
    ) -> Result<OrdersAndFills, FillerError> {
        let mut unsigned_fill = UnsignedFill::new().with_chain(self.constants.clone());

        if let Some(deadline_offset) = self.options.deadline_offset {
            let deadline = Utc::now().timestamp() as u64 + deadline_offset;
            unsigned_fill = unsigned_fill.with_deadline(deadline);
        }

        if let Some(nonce) = self.options.nonce {
            unsigned_fill = unsigned_fill.with_nonce(nonce);
        }

        for order in &orders {
            unsigned_fill = unsigned_fill.fill(order);
        }

        let fills = unsigned_fill.sign(&self.signer).await?;
        let signer_address = self.signer.address();
        Ok(OrdersAndFills { orders, fills, signer_address })
    }
}

impl<Sign, Source, Submit> Filler<Sign, Source, Submit>
where
    Sign: Signer + Send + Sync,
    Submit: FillSubmitter + Send + Sync,
{
    /// Fill one or more orders.
    ///
    /// Signs fills for all orders and submits them via the [`FillSubmitter`].
    ///
    /// Returns an error if `orders` is empty, or if signing or submission fails.
    #[instrument(skip_all, fields(order_count = orders.len()))]
    pub async fn fill(&self, orders: Vec<SignedOrder>) -> Result<Submit::Response, FillerError> {
        if orders.is_empty() {
            return Err(FillerError::NoOrders);
        }

        let orders_and_fills = self.sign_fills(orders).await?;
        self.submitter
            .submit_fills(orders_and_fills)
            .await
            .map_err(|error| FillerError::Submission(Box::new(error)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filler_options_default() {
        let opts = FillerOptions::default();
        assert!(opts.deadline_offset.is_none());
        assert!(opts.nonce.is_none());
    }

    #[test]
    fn filler_options_new() {
        let opts = FillerOptions::new();
        assert!(opts.deadline_offset.is_none());
        assert!(opts.nonce.is_none());
    }

    #[test]
    fn filler_options_with_deadline_offset() {
        let opts = FillerOptions::new().with_deadline_offset(60);
        assert_eq!(opts.deadline_offset, Some(60));
        assert!(opts.nonce.is_none());
    }

    #[test]
    fn filler_options_with_nonce() {
        let opts = FillerOptions::new().with_nonce(12345);
        assert!(opts.deadline_offset.is_none());
        assert_eq!(opts.nonce, Some(12345));
    }

    #[test]
    fn filler_options_chained() {
        let opts = FillerOptions::new().with_deadline_offset(30).with_nonce(999);
        assert_eq!(opts.deadline_offset, Some(30));
        assert_eq!(opts.nonce, Some(999));
    }

    #[test]
    fn filler_error_display_no_orders() {
        let err = FillerError::NoOrders;
        assert_eq!(err.to_string(), "no orders to fill");
    }

    #[test]
    fn orders_and_fills_accessors() {
        use alloy::primitives::Address;
        use std::collections::HashMap;

        let orders = vec![];
        let fills = HashMap::new();
        let signer_address = Address::repeat_byte(0x42);

        let oaf = OrdersAndFills { orders, fills, signer_address };

        assert!(oaf.orders().is_empty());
        assert!(oaf.fills().is_empty());
        assert_eq!(oaf.signer_address(), Address::repeat_byte(0x42));
    }
}
