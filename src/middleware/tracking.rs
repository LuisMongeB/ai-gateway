use crate::middleware::auth::ValidatedApiKey;
use crate::tracking::RequestTracker;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use std::future::{ready, Ready};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use std::time::Instant;
use tracing::info;

#[derive(Clone)]

pub struct TrackingMiddleware {
    tracker: Arc<RwLock<RequestTracker>>,
}

impl TrackingMiddleware {
    pub fn new(tracker: Arc<RwLock<RequestTracker>>) -> Self {
        Self { tracker }
    }
}

impl<S, B> Transform<S, ServiceRequest> for TrackingMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = TrackingMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(TrackingMiddlewareService {
            service,
            tracker: self.tracker.clone(),
        }))
    }
}

pub struct TrackingMiddlewareService<S> {
    service: S,
    tracker: Arc<RwLock<RequestTracker>>,
}

impl<S, B> Service<ServiceRequest> for TrackingMiddlewareService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let api_key = req
            .extensions()
            .get::<ValidatedApiKey>()
            .map(|k| k.key.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let tracker = self.tracker.clone();

        let start = Instant::now();

        // Clone the tracker Arc?

        // call the next service
        let fut = self.service.call(req);

        Box::pin(async move {
            let response = fut.await?;
            let latency = start.elapsed().as_millis() as u64;
            let is_error = response.status().is_server_error();

            tracker
                .write()
                .unwrap()
                .record_request(&api_key, latency, is_error);
            info!(
                api_key = %api_key,
                latency_ms = latency,
                is_error = is_error,
                "Tracked request"
            );
            Ok(response)
        })
    }
}
