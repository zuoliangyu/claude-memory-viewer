use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::response::Json;
use serde::Deserialize;
use session_core::bookmarks::{self, Bookmark};

#[derive(Deserialize)]
pub struct ListQuery {
    pub source: Option<String>,
}

pub async fn list_bookmarks(Query(params): Query<ListQuery>) -> Json<Vec<Bookmark>> {
    Json(bookmarks::list_bookmarks(params.source.as_deref()))
}

pub async fn add_bookmark(
    Json(bookmark): Json<Bookmark>,
) -> Result<Json<Bookmark>, (StatusCode, String)> {
    bookmarks::add_bookmark(bookmark)
        .map(Json)
        .map_err(|e| (StatusCode::BAD_REQUEST, e))
}

pub async fn remove_bookmark(Path(id): Path<String>) -> Result<Json<()>, (StatusCode, String)> {
    bookmarks::remove_bookmark(&id)
        .map(Json)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}
