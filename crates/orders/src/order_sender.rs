use crate::OrderSubmitter;
use alloy::signers::Signer;
use signet_constants::SignetSystemConstants;
use signet_types::{SignedOrder, SigningError, UnsignedOrder};
use signet_zenith::RollupOrders::Order;

/// Errors returned by [`OrderSender`].
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum OrderSenderError {
    /// Order signing failed.
    #[error("order signing error: {0}")]
    Signing(#[from] SigningError),
    /// Order submission failed.
    #[error("order submission error: {0}")]
    Submission(#[source] Box<dyn core::error::Error + Send + Sync>),
}

/// Sends signed orders to a backend.
///
/// `OrderSender` is generic over:
/// - `Sign`: A [`Signer`] for signing orders
/// - `Submit
///`: An [`OrderSubmitter`] for submitting signed orders to a backend
#[derive(Debug, Clone)]
pub struct OrderSender<Sign, Submit> {
    signer: Sign,
    submitter: Submit,
    constants: SignetSystemConstants,
}

impl<Sign, Submit> OrderSender<Sign, Submit> {
    /// Create a new order sender instance.
    pub const fn new(signer: Sign, submitter: Submit, constants: SignetSystemConstants) -> Self {
        Self { signer, submitter, constants }
    }

    /// Get a reference to the signer.
    pub const fn signer(&self) -> &Sign {
        &self.signer
    }

    /// Get a reference to the submitter.
    pub const fn submitter(&self) -> &Submit {
        &self.submitter
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }
}

impl<Sign, Submit> OrderSender<Sign, Submit>
where
    Sign: Signer,
{
    /// Sign an [`Order`] and return a [`SignedOrder`].
    pub async fn sign_order(&self, order: &Order) -> Result<SignedOrder, OrderSenderError>
    where
        Submit: OrderSubmitter,
    {
        self.sign_unsigned_order(UnsignedOrder::from(order)).await
    }

    /// Sign an [`UnsignedOrder`] and return a [`SignedOrder`].
    pub async fn sign_unsigned_order(
        &self,
        order: UnsignedOrder<'_>,
    ) -> Result<SignedOrder, OrderSenderError>
    where
        Submit: OrderSubmitter,
    {
        order.with_chain(&self.constants).sign(&self.signer).await.map_err(Into::into)
    }
}

impl<Sign, Submit> OrderSender<Sign, Submit>
where
    Submit: OrderSubmitter + Send + Sync,
{
    /// Submit a signed order to the backend.
    pub async fn send_order(&self, order: SignedOrder) -> Result<(), OrderSenderError> {
        self.submitter
            .submit_order(order)
            .await
            .map_err(|error| OrderSenderError::Submission(Box::new(error)))
    }
}

impl<Sign, Submit> OrderSender<Sign, Submit>
where
    Sign: Signer + Send + Sync,
    Submit: OrderSubmitter + Send + Sync,
{
    /// Sign and submit an order to the backend, returning the signed order.
    pub async fn sign_and_send_order(&self, order: Order) -> Result<SignedOrder, OrderSenderError> {
        let signed = self.sign_order(&order).await?;
        self.send_order(signed.clone()).await?;
        Ok(signed)
    }
}
