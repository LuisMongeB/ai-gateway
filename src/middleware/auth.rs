use actix_web::error::ErrorUnauthorized;
use actix_web::{
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpMessage,
};
use std::future::{ready, Ready};
use std::pin::Pin;
use std::task::{Context, Poll};

use log::info;

#[derive(Debug, Clone)]
pub enum ApiKeyRole {
    User,
    Admin,
}

#[derive(Clone)]
pub struct ValidatedApiKey {
    pub key: String,
    pub role: ApiKeyRole,
}

pub struct AuthMiddleware {
    api_keys: Vec<String>,
    admin_keys: Vec<String>,
}

impl AuthMiddleware {
    pub fn new(api_keys: Vec<String>, admin_keys: Vec<String>) -> Self {
        Self {
            api_keys: api_keys,
            admin_keys,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for AuthMiddleware
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Transform = AuthMiddlewareService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddlewareService {
            service,
            api_keys: self.api_keys.clone(),
            admin_keys: self.admin_keys.clone(),
        }))
    }
}

pub struct AuthMiddlewareService<S> {
    service: S,
    api_keys: Vec<String>,
    admin_keys: Vec<String>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddlewareService<S>
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
        let auth_header = req.headers().get("Authorization");

        let token_str = auth_header.and_then(|h| h.to_str().ok()).unwrap_or("None");
        info!("Middleware received header: {}", token_str);
        info!("Middleware expects one of: {:?}", self.api_keys);

        let token = auth_header
            .and_then(|h| h.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .map(|t| t.to_string());

        let role = token.as_ref().and_then(|t| {
            if self.admin_keys.contains(t) {
                Some(ApiKeyRole::Admin)
            } else if self.api_keys.contains(t) {
                Some(ApiKeyRole::User)
            } else {
                None
            }
        });

        match role {
            Some(r) => {
                info!("Auth Success! Role: {:?}", r);
                req.extensions_mut().insert(ValidatedApiKey {
                    key: token.unwrap(),
                    role: r,
                });
                let fut = self.service.call(req);
                Box::pin(async move { fut.await })
            }
            None => {
                info!("Auth Failed. Token extracted: {:?}", token);
                Box::pin(async move { Err(ErrorUnauthorized("Invalid or missing API key")) })
            }
        }
    }
}
