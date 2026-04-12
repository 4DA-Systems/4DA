//! In-memory per-IP rate limiter as a tower Layer/Service.

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};
use tower::{Layer, Service};

/// Per-IP request tracking: (count, window_start).
type RateMap = HashMap<IpAddr, (u64, Instant)>;

/// Shared rate limit state.
#[derive(Clone)]
pub struct RateLimitState {
    map: Arc<Mutex<RateMap>>,
    max_requests: u64,
    window: Duration,
}

impl RateLimitState {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            map: Arc::new(Mutex::new(HashMap::new())),
            max_requests,
            window: Duration::from_secs(window_secs),
        }
    }

    /// Check if the IP is allowed; returns true if allowed, false if rate-limited.
    fn check(&self, ip: IpAddr) -> bool {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();

        let entry = map.entry(ip).or_insert((0, now));

        // Window expired — reset
        if now.duration_since(entry.1) >= self.window {
            entry.0 = 1;
            entry.1 = now;
            return true;
        }

        entry.0 += 1;
        entry.0 <= self.max_requests
    }

    /// Remove expired entries. Call periodically from a background task.
    pub fn cleanup(&self) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        map.retain(|_, (_, window_start)| now.duration_since(*window_start) < self.window);
    }
}

// ---------- Tower Layer ----------

#[derive(Clone)]
pub struct RateLimitLayer {
    state: RateLimitState,
}

impl RateLimitLayer {
    pub fn new(max_requests: u64, window_secs: u64) -> Self {
        Self {
            state: RateLimitState::new(max_requests, window_secs),
        }
    }

    /// Get a handle to the state for background cleanup.
    pub fn state(&self) -> RateLimitState {
        self.state.clone()
    }
}

impl<S> Layer<S> for RateLimitLayer {
    type Service = RateLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RateLimitService {
            inner,
            state: self.state.clone(),
        }
    }
}

// ---------- Tower Service ----------

#[derive(Clone)]
pub struct RateLimitService<S> {
    inner: S,
    state: RateLimitState,
}

impl<S> Service<Request<Body>> for RateLimitService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        // Extract IP from connection info or X-Forwarded-For
        let ip = extract_ip(&req);

        if !self.state.check(ip) {
            let response = Response::builder()
                .status(StatusCode::TOO_MANY_REQUESTS)
                .header("content-type", "application/json")
                .header("retry-after", "60")
                .body(Body::from(
                    r#"{"error":"Rate limit exceeded. Try again later."}"#,
                ))
                .unwrap_or_else(|_| Response::new(Body::from("rate limited")));

            return Box::pin(async move { Ok(response) });
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

/// Best-effort IP extraction from request headers/connection.
fn extract_ip<B>(req: &Request<B>) -> IpAddr {
    // Try X-Forwarded-For first (for reverse proxies)
    if let Some(forwarded) = req.headers().get("x-forwarded-for") {
        if let Ok(val) = forwarded.to_str() {
            if let Some(first) = val.split(',').next() {
                if let Ok(ip) = first.trim().parse::<IpAddr>() {
                    return ip;
                }
            }
        }
    }

    // Try X-Real-IP
    if let Some(real_ip) = req.headers().get("x-real-ip") {
        if let Ok(val) = real_ip.to_str() {
            if let Ok(ip) = val.trim().parse::<IpAddr>() {
                return ip;
            }
        }
    }

    // Fallback: loopback (connection info not available in this context)
    IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
}

/// Spawn a periodic cleanup task for expired rate limit windows.
pub fn spawn_rate_limit_cleanup(state: RateLimitState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            state.cleanup();
        }
    });
}
