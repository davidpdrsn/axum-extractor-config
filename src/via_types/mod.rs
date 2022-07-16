//! Extractors that are configured via types.

#![allow(missing_docs)]

use axum::{
    async_trait,
    body::{Bytes, HttpBody},
    extract::{rejection::JsonRejection, FromRequest, RequestParts},
    response::IntoResponse,
    BoxError,
};
use serde::de::DeserializeOwned;
use std::{fmt, marker::PhantomData};

pub struct Json<T, C>(pub T, pub PhantomData<fn() -> C>);

#[async_trait]
impl<T, C, B> FromRequest<B> for Json<T, C>
where
    B: HttpBody<Data = Bytes> + Send + 'static,
    B::Error: Into<BoxError>,
    T: DeserializeOwned + Send,
    C: IntoResponse + From<JsonRejection>,
{
    type Rejection = C;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        match req.extract::<axum::extract::Json<T>>().await {
            Ok(axum::extract::Json(value)) => Ok(Self(value, PhantomData)),
            Err(rejection) => Err(C::from(rejection)),
        }
    }
}

impl<T, C> Clone for Json<T, C>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self(self.0.clone(), self.1)
    }
}

impl<T, C> Copy for Json<T, C> where T: Copy {}

impl<T, C> fmt::Debug for Json<T, C>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Json").field(&self.0).field(&self.1).finish()
    }
}

impl<T, C> Default for Json<T, C>
where
    T: Default,
{
    fn default() -> Self {
        Self(Default::default(), Default::default())
    }
}
