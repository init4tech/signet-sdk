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
    Submission(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Sends signed orders to a backend.
///
/// `OrderSender` is generic over:
/// - `SignerT`: A [`Signer`] for signing orders
/// - `SubmitterT`: An [`OrderSubmitter`] for submitting signed orders to a backend
#[derive(Debug, Clone)]
pub struct OrderSender<SignerT, SubmitterT> {
    signer: SignerT,
    submitter: SubmitterT,
    constants: SignetSystemConstants,
}

impl<SignerT, SubmitterT> OrderSender<SignerT, SubmitterT> {
    /// Create a new order sender instance.
    pub const fn new(
        signer: SignerT,
        submitter: SubmitterT,
        constants: SignetSystemConstants,
    ) -> Self {
        Self { signer, submitter, constants }
    }

    /// Get a reference to the signer.
    pub const fn signer(&self) -> &SignerT {
        &self.signer
    }

    /// Get a reference to the submitter.
    pub const fn submitter(&self) -> &SubmitterT {
        &self.submitter
    }

    /// Get a reference to the system constants.
    pub const fn constants(&self) -> &SignetSystemConstants {
        &self.constants
    }
}

impl<SignerT, SubmitterT> OrderSender<SignerT, SubmitterT>
where
    SignerT: Signer,
{
    /// Sign an [`Order`] and return a [`SignedOrder`].
    pub async fn sign_order(&self, order: &Order) -> Result<SignedOrder, OrderSenderError>
    where
        SubmitterT: OrderSubmitter,
    {
        self.sign_unsigned_order(UnsignedOrder::from(order)).await
    }

    /// Sign an [`UnsignedOrder`] and return a [`SignedOrder`].
    pub async fn sign_unsigned_order(
        &self,
        order: UnsignedOrder<'_>,
    ) -> Result<SignedOrder, OrderSenderError>
    where
        SubmitterT: OrderSubmitter,
    {
        order.with_chain(&self.constants).sign(&self.signer).await.map_err(Into::into)
    }
}

impl<SignerT, SubmitterT> OrderSender<SignerT, SubmitterT>
where
    SubmitterT: OrderSubmitter + Send + Sync,
{
    /// Submit a signed order to the backend.
    pub async fn send_order(&self, order: SignedOrder) -> Result<(), OrderSenderError> {
        self.submitter
            .submit_order(order)
            .await
            .map_err(|error| OrderSenderError::Submission(Box::new(error)))
    }
}

impl<SignerT, SubmitterT> OrderSender<SignerT, SubmitterT>
where
    SignerT: Signer + Send + Sync,
    SubmitterT: OrderSubmitter + Send + Sync,
{
    /// Sign and submit an order to the backend, returning the signed order.
    pub async fn sign_and_send_order(&self, order: Order) -> Result<SignedOrder, OrderSenderError> {
        let signed = self.sign_order(&order).await?;
        self.send_order(signed.clone()).await?;
        Ok(signed)
    }
}
