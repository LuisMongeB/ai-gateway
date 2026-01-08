use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    last_updated: Instant,
    capacity: f64,
    refill_rate: f64, // tokens per second
}

impl Bucket {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            tokens: capacity,
            last_updated: Instant::now(),
            capacity,
            refill_rate,
        }
    }

    fn try_consume(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_updated).as_secs_f64();

        // Refill tokens based on time elapsed
        // tokens = min(capacity, current_tokens + (elapsed * rate))
        self.tokens = (self.tokens + (elapsed * self.refill_rate)).min(self.capacity);
        self.last_updated = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone)]
pub struct RateLimiter {
    // Outer RwLock: allows concurrent reads (checking if bucket exists)
    // Inner Mutex: allows safe mutation of a specific bucket
    buckets: Arc<RwLock<HashMap<String, Mutex<Bucket>>>>,
    default_capacity: f64,
    default_refill_rate: f64,
}

impl RateLimiter {
    pub fn new(requests_per_minute: u64) -> Self {
        let rate = requests_per_minute as f64 / 60.0;
        Self {
            buckets: Arc::new(RwLock::new(HashMap::new())),
            default_capacity: requests_per_minute as f64, // Allow full minute burst? Or maybe smaller? Let's say 2x rate or just N.
            // Commonly capacity = burst size. Let's start with capacity = requests_per_minute (allow 1 min burst)
            default_refill_rate: rate,
        }
    }

    pub fn check_key(&self, api_key: &str) -> bool {
        // 1. Fast path: Read lock to find existing bucket
        {
            let map = self.buckets.read().unwrap();
            if let Some(bucket_mutex) = map.get(api_key) {
                // Found bucket, acquire mutex for this specific key
                let mut bucket = bucket_mutex.lock().unwrap();
                return bucket.try_consume();
            }
        } // Drop read lock here

        // 2. Slow path: Write lock to insert new bucket
        // Note: Race condition possible here (another thread could have inserted between drop and acquire),
        // so we must check again.
        let mut map = self.buckets.write().unwrap();

        // Check again in case it was created while waiting for write lock
        let bucket_mutex = map.entry(api_key.to_string()).or_insert_with(|| {
            Mutex::new(Bucket::new(self.default_capacity, self.default_refill_rate))
        });

        let mut bucket = bucket_mutex.lock().unwrap();
        bucket.try_consume()
    }
}

// Middleware Boilerplate
use actix_web::dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures_util::future::LocalBoxFuture;
use std::future::{ready, Ready};

// 1. The Middleware Factory
pub struct RateLimitMiddleware {
    limiter: Arc<RateLimiter>,
}

impl RateLimitMiddleware {
    pub fn new(limiter: Arc<RateLimiter>) -> Self {
        Self { limiter }
    }
}

// 2. Transform Implementation (Middleware Factory -> Middleware Service)
impl<S, B> Transform<S, ServiceRequest> for RateLimitMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = RateLimitMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RateLimitMiddlewareService {
            service,
            limiter: self.limiter.clone(),
        }))
    }
}

// 3. The Middleware Service
pub struct RateLimitMiddlewareService<S> {
    service: S,
    limiter: Arc<RateLimiter>,
}

impl<S, B> Service<ServiceRequest> for RateLimitMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        use crate::middleware::auth::ValidatedApiKey;
        use actix_web::HttpMessage;

        let limiter = self.limiter.clone();

        // Extract API Key from extensions.
        // Assumes AuthMiddleware ran first (registered LAST in main.rs).
        let api_key = {
            let extensions = req.extensions();
            extensions.get::<ValidatedApiKey>().map(|k| k.key.clone())
        };

        if let Some(key) = api_key {
            // Check rate limit
            if !limiter.check_key(&key) {
                // Rate limit exceeded
                return Box::pin(async {
                    Err(actix_web::error::ErrorTooManyRequests(
                        "Rate limit exceeded",
                    ))
                });
            }
        }

        // If we in here, either no key (public endpoint?) or allowed.
        // Proceed to next service.
        let fut = self.service.call(req);
        Box::pin(async move {
            let res = fut.await?;
            Ok(res)
        })
    }
}
