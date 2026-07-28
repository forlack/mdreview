use axum::{
    body::Body,
    extract::Request,
    http::{Response, StatusCode, header},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "web/dist/"]
struct WebAssets;

pub async fn serve(request: Request) -> Response<Body> {
    let path = request.uri().path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path).or_else(|| WebAssets::get("index.html")) {
        Some(asset) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .header(header::CACHE_CONTROL, "no-store")
                .header(header::REFERRER_POLICY, "no-referrer")
                .header(
                    header::CONTENT_SECURITY_POLICY,
                    "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'",
                )
                .body(Body::from(asset.data))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("Not found"))
            .unwrap(),
    }
}
