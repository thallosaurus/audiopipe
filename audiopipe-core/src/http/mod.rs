use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};

/// The JSON RPC API
/// 
/// coming coon
struct AxumState {

}

pub fn get_axum_app() -> Router {
    let shared_state = Arc::new(AxumState {});
    Router::new()
        .route("/", get(|state: State<Arc<AxumState>>| async {

        }))
        .with_state(shared_state)
}


fn handler(State(state): State<Arc<AxumState>>) {
    // ...
}