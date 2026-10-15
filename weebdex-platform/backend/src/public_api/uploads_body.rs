//! Exact raw multipart receipt with constant-memory hashing and bounded disk use.
//! The unlinked file is private to this request and disappears on cancellation,
//! decoder failure or process death. No request proof is promoted into a session.
use super::{PublicApiError, PublicApiResult};
use crate::{AppState, domains::uploads::storage};
use axum::{
    body::{Body, HttpBody},
    extract::{FromRequest, Multipart, Request},
    http::{HeaderMap, StatusCode, header},
};
use manga_node::http_auth::AdmittedHttpCapability;
use sha2::{Digest, Sha256};
use std::{pin::Pin, sync::{Arc, OnceLock}, time::Duration};
use tokio::{io::{AsyncSeekExt, AsyncWriteExt}, sync::{OwnedSemaphorePermit, Semaphore}};
use tokio_util::io::ReaderStream;

pub(super) const BODY_LIMIT:usize=96*1024*1024;
static INTAKES:OnceLock<Arc<Semaphore>>=OnceLock::new();

pub(super) async fn receive(state:&AppState, uri:&axum::http::Uri, headers:&HeaderMap, request:Request)
    -> PublicApiResult<(AdmittedHttpCapability,Multipart,OwnedSemaphorePermit)> {
    let values=headers.get_all(header::CONTENT_TYPE).iter().collect::<Vec<_>>();
    if values.len()!=1 || values[0].to_str().ok().and_then(|v|v.split(';').next())
        .is_none_or(|v|!v.trim().eq_ignore_ascii_case("multipart/form-data")) {
        return Err(PublicApiError::new(StatusCode::UNSUPPORTED_MEDIA_TYPE,"Unsupported Media Type","Expected multipart/form-data"));
    }
    // This only checks the signed header scope/admission; it cannot mutate,
    // consume replay state, bind a native account or validate the body digest.
    super::auth::admitted_preflight(state,uri,headers,"POST").await?;
    let permit=INTAKES.get_or_init(||Arc::new(Semaphore::new(2))).clone().try_acquire_owned()
        .map_err(|_|PublicApiError::new(StatusCode::TOO_MANY_REQUESTS,"Too Many Requests","Upload intake capacity is busy"))?;
    let (parts,body)=request.into_parts();
    let timeout=Duration::from_secs(state.config.upload_body_timeout_seconds);
    let (file,hash,empty)=tokio::time::timeout(timeout,spool(&state.config.uploads_dir,body)).await
        .map_err(|_|PublicApiError::bad_request("Multipart request body timeout"))??;
    let capability=super::auth::admitted_request_digest(state,uri,headers,"POST",hash,empty).await?;
    let request=Request::from_parts(parts,Body::from_stream(ReaderStream::with_capacity(file,64*1024)));
    let multipart=Multipart::from_request(request,state).await
        .map_err(|_|PublicApiError::bad_request("Invalid multipart boundary"))?;
    Ok((capability,multipart,permit))
}

async fn spool(root:&std::path::Path, mut body:Body) -> PublicApiResult<(tokio::fs::File,[u8;32],bool)> {
    let directory=root.join("temporary");
    storage::ensure_directory(&directory).await?;
    let path=directory.join(hex::encode(rand::random::<[u8;32]>()));
    let output=tokio::fs::OpenOptions::new().read(true).write(true).create_new(true).mode(0o600).open(&path).await
        .map_err(|_|PublicApiError::unavailable())?;
    let lease=output.into_std().await;
    let locked=lease.try_lock().map_err(|_|PublicApiError::unavailable())
        .and_then(|_|storage::verify_named_lease(&lease,&path).map_err(Into::into));
    // Removing the name first also covers cancellation during the first write.
    let removed=tokio::fs::remove_file(&path).await;
    locked?; removed.map_err(|_|PublicApiError::unavailable())?;
    let mut output=tokio::fs::File::from_std(lease);
    let mut hash=Sha256::new(); let mut observed=0usize;
    while let Some(frame)=std::future::poll_fn(|cx|Pin::new(&mut body).poll_frame(cx)).await {
        let frame=frame.map_err(|_|PublicApiError::bad_request("Invalid or oversized multipart stream"))?;
        if let Ok(data)=frame.into_data() {
            observed=observed.checked_add(data.len()).filter(|n|*n<=BODY_LIMIT)
                .ok_or_else(||PublicApiError::new(StatusCode::PAYLOAD_TOO_LARGE,"Payload Too Large","Multipart request exceeds 96 MiB"))?;
            hash.update(&data);
            output.write_all(&data).await.map_err(|_|PublicApiError::unavailable())?;
        }
    }
    output.flush().await.map_err(|_|PublicApiError::unavailable())?;
    output.rewind().await.map_err(|_|PublicApiError::unavailable())?;
    Ok((output,hash.finalize().into(),observed==0))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn receipt_hashes_actual_bytes_and_leaves_no_named_file() {
        let root=tempfile::tempdir().unwrap();
        let bytes=b"--boundary\r\nactual multipart bytes\0\xff";
        let (mut file,hash,empty)=spool(root.path(),Body::from(bytes.as_slice())).await.unwrap();
        assert!(!empty); assert_eq!(hash,Sha256::digest(bytes).as_slice());
        let mut actual=Vec::new(); tokio::io::AsyncReadExt::read_to_end(&mut file,&mut actual).await.unwrap();
        assert_eq!(actual,bytes);
        assert_eq!(std::fs::read_dir(root.path().join("temporary")).unwrap().count(),0);
    }
}
