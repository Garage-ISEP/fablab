use axum::extract::Request;
use axum::http::header::HeaderName;
use axum::middleware::Next;
use axum::response::Response;

pub async fn security_headers_middleware(request: Request, next: Next) -> Response
{
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    headers.insert(
        "X-Frame-Options",
        "DENY".parse().expect("valid header value"),
    );
    headers.insert(
        "X-Content-Type-Options",
        "nosniff".parse().expect("valid header value"),
    );
    headers.insert(
        "Referrer-Policy",
        "strict-origin-when-cross-origin".parse().expect("valid header value"),
    );
    headers.insert(
        "Permissions-Policy",
        "camera=(), microphone=(), geolocation=()".parse().expect("valid header value"),
    );
    let csp_name: HeaderName = "content-security-policy".parse().expect("valid header name");
    if !headers.contains_key(&csp_name)
    {
        headers.insert(
            csp_name,
            "default-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none'"
                .parse()
                .expect("valid header value"),
        );
    }
    headers.insert(
        "Strict-Transport-Security",
        "max-age=31536000; includeSubDomains".parse().expect("valid header value"),
    );

    response
}
