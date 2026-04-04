use axum::{
    extract::State,
    http::Request,
    middleware::Next,
    response::Response,
};
use crate::state::AppState;
use crate::error::AppError;
use std::sync::Arc;
use axum_extra::headers::Authorization;
use axum_extra::headers::authorization::Bearer;
use axum_extra::TypedHeader;

pub async fn auth_middleware<B>(
    State(state): State<Arc<AppState>>,
    maybe_auth: Option<TypedHeader<Authorization<Bearer>>>,
    request: Request<B>,
    next: Next<B>,
) -> Result<Response, AppError> {
    let auth = maybe_auth.ok_or(AppError::Unauthorized)?;

    if auth.token() != state.config.api_token {
        return Err(AppError::Unauthorized);
    }

    Ok(next.run(request).await)
}
