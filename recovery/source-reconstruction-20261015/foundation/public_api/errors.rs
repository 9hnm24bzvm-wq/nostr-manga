//! RECONSTRUCTED, UNVERIFIED safe public errors. Reconcile existing internal ApiError bridge.
use axum::{http::StatusCode,response::{IntoResponse,Response},Json};
use serde::Serialize;
use uuid::Uuid;

pub type PublicApiResult<T> = Result<T,PublicApiError>;
#[derive(Debug)] pub struct PublicApiError {
    pub status:StatusCode,
    pub title:&'static str,
    pub detail:String,
}
impl PublicApiError {
    pub fn new(status:StatusCode,title:&'static str,detail:impl Into<String>)->Self {
        Self{status,title,detail:detail.into()}
    }
    pub fn bad_request(detail:impl Into<String>)->Self {
        Self::new(StatusCode::BAD_REQUEST,"Bad Request",detail)
    }
    pub fn unauthorized(detail:impl Into<String>)->Self {
        Self::new(StatusCode::UNAUTHORIZED,"Unauthorized",detail)
    }
    pub fn forbidden(detail:impl Into<String>)->Self {
        Self::new(StatusCode::FORBIDDEN,"Forbidden",detail)
    }
    pub fn not_found(detail:impl Into<String>)->Self {
        Self::new(StatusCode::NOT_FOUND,"Not Found",detail)
    }
    pub fn conflict(detail:impl Into<String>)->Self {
        Self::new(StatusCode::CONFLICT,"Conflict",detail)
    }
    pub fn unavailable()->Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE,"Service Unavailable","Resource projection is unavailable")
    }
    pub fn unsupported()->Self {
        Self::new(StatusCode::NOT_IMPLEMENTED,"Not Implemented","This operation is not implemented")
    }
    pub fn internal()->Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR,"Internal Server Error","The request could not be completed")
    }
}
impl std::fmt::Display for PublicApiError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {f.write_str(&self.detail)}
}
impl std::error::Error for PublicApiError {}
#[derive(Serialize)] struct ErrorEnvelope { result:&'static str, errors:Vec<ErrorObject> }
#[derive(Serialize)] struct ErrorObject { id:Uuid,status:u16,title:&'static str,detail:String }
impl IntoResponse for PublicApiError {
    fn into_response(self)->Response {
        (self.status,Json(ErrorEnvelope{result:"error",errors:vec![ErrorObject{
            id:Uuid::new_v4(),status:self.status.as_u16(),title:self.title,detail:self.detail,
        }]})).into_response()
    }
}
