use tower_http::cors::CorsLayer;

/// CORS permissivo para desarrollo local y UIs externas.
pub fn cors() -> CorsLayer {
    CorsLayer::permissive()
}
