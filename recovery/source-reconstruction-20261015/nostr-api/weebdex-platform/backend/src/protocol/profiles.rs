//! Original signed public account metadata, never authentication or account binding.
use super::ProtocolFacade;
use crate::error::{ApiError,ApiResult};
use manga_node::{accounts::PublicAccountProfile,db::Id};
impl ProtocolFacade {
    pub async fn public_account_profile(&self,publisher:Id)->ApiResult<PublicAccountProfile>{
        self.require_schema()?;
        self.repository.public_account_profile(publisher).await.map_err(|_|ApiError::unavailable())
    }
}
