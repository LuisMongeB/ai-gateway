use std::pin::Pin;
use futures::Stream;
use bytes::Bytes;

use async_trait::async_trait;

pub enum ProviderError {
    Network(String),
    Parse(String),
    ProviderError {
        status: u16,
        message: String,
    },
}