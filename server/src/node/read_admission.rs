//! One outer permit covers HTTP authentication, policy lookup and handler reads.
//! The permit ends when owned handler work terminates, even after an early
//! response or disconnect. Long-lived bodies own independent byte/count credits.
use super::*;
pub(super) async fn guard(
    State(a): State<Shared>,
    request: Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, axum::response::Response> {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str())
        .unwrap_or("");
    let is_read = matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    ) || request.method() == axum::http::Method::POST
        && matches!(route, "/library/read" | "/reconcile");
    let class = if matches!(
        route,
        "/libraries/{key}"
            | "/libraries/{key}/entries"
            | "/library/read"
            | "/library/blobs/{hash}"
            | "/federation/libraries/{key}"
            | "/federation/library-routing/{hash}"
            | "/federation/library/blobs/{hash}"
    ) {
        crate::read_admission::Class::LibraryHead
    } else if matches!(route, "/" | "/federation/relay")
        && request.headers().contains_key("upgrade")
    {
        crate::read_admission::Class::Live
    } else {
        crate::read_admission::Class::Catalogue
    };
    let permit = if is_read {
        Some(a.reads.acquire(class).await.map_err(|_| {
            use axum::response::IntoResponse;
            (
                StatusCode::TOO_MANY_REQUESTS,
                [(axum::http::header::RETRY_AFTER, "1")],
                Json(json!({"error":"read_slots_busy"})),
            )
                .into_response()
        })?)
    } else {
        None
    };
    let Some(permit) = permit else {
        return Ok(next.run(request).await);
    };
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let observation = permit.observation();
    // Only admitted reads spawn work. Disconnect drops the receiver, never the
    // handler, its pooled SQL futures, or the admission credits they own.
    crate::node::spawn_owned_scope(RESPONSE.scope(
        std::sync::Mutex::new(Some(sender)),
        crate::read_diagnostics::scope(observation, async move {
            let _permit = permit;
            let response = next.run(request).await;
            send_response(response);
        }),
    ));
    Ok(receiver.await.unwrap_or_else(|_| {
        use axum::response::IntoResponse;
        unavailable().into_response()
    }))
}

tokio::task_local! {
    static RESPONSE: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<axum::response::Response>>>;
}

fn send_response(response: axum::response::Response) {
    let _ = RESPONSE.try_with(|sender| {
        if let Some(sender) = sender.lock().expect("read response mutex poisoned").take() {
            let _ = sender.send(response);
        }
    });
}

/// A response deadline does not cancel admitted database work. Send the error
/// now, then drain the bounded operation under its original handler permit.
/// Callers receive Err after draining and cannot begin another operation.
pub(super) async fn deadline<F: std::future::Future>(
    duration: Duration,
    future: F,
    error: ApiError,
) -> Result<F::Output, ApiError> {
    use axum::response::IntoResponse;
    tokio::pin!(future);
    match tokio::time::timeout(duration, &mut future).await {
        Ok(result) => Ok(result),
        Err(_) => {
            crate::read_diagnostics::draining();
            send_response((error.0, Json(error.1.0.clone())).into_response());
            let _ = future.await;
            Err(error)
        }
    }
}
