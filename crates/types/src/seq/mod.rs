mod req;
pub use req::SignRequest;

mod resp;
pub use resp::SignResponse;

/// A [`RequestSigner`] signs [`SignRequest`]s by delegating to an
/// [`alloy::signers::Signer`].
pub trait RequestSigner {
    /// Signs a [`SignRequest`] and returns the [`alloy::primitives::Signature`].
    fn sign_request(
        &self,
        request: &SignRequest,
    ) -> impl std::future::Future<
        Output = Result<alloy::primitives::PrimitiveSignature, alloy::signers::Error>,
    > + Send;
}

impl<T> RequestSigner for T
where
    T: alloy::signers::Signer + Send + Sync,
{
    async fn sign_request(
        &self,
        request: &SignRequest,
    ) -> Result<alloy::primitives::PrimitiveSignature, alloy::signers::Error> {
        let hash = request.signing_hash();
        self.sign_hash(&hash).await
    }
}
