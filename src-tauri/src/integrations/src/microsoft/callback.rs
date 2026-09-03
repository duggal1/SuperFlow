use crate::error::IntegrationError;
use crate::oauth::LoopbackCallback;

pub(crate) async fn bind() -> Result<LoopbackCallback, IntegrationError> {
    LoopbackCallback::bind("localhost").await
}
