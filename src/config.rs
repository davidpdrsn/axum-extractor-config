use axum::{
    async_trait,
    extract::{FromRequest, RequestParts},
    http::{Request, StatusCode},
    response::{IntoResponse, Response},
    Extension,
};
use futures_util::{
    future::{Either, MapOk, TryFutureExt},
    FutureExt,
};
use std::{
    fmt,
    future::{ready, Ready},
    marker::PhantomData,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Configuration [extractor] and [layer].
///
/// [extractor]: FromRequest
/// [layer]: Layer
pub struct Config<T, B> {
    config: T,
    _marker: PhantomData<fn() -> B>,
}

impl<T, B> Config<T, B> {
    /// Create a new `Config`.
    pub fn new(config: T) -> Self {
        Self {
            config,
            _marker: PhantomData,
        }
    }

    /// Consume the config and get the inner value.
    pub fn into_inner(self) -> T {
        self.config
    }
}

impl<T, B> fmt::Debug for Config<T, B>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Config")
            .field("config", &self.config)
            .field("_marker", &self._marker)
            .finish()
    }
}

impl<T, B> Default for Config<T, B>
where
    T: Default,
{
    fn default() -> Self {
        Self {
            config: Default::default(),
            _marker: Default::default(),
        }
    }
}

impl<T, B> Copy for Config<T, B> where T: Copy {}

impl<T, B> Clone for Config<T, B>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            _marker: self._marker,
        }
    }
}

#[async_trait]
impl<T, B> FromRequest<B> for Config<T, B>
where
    T: Clone + Send + Sync + 'static,
    B: Send + 'static,
{
    type Rejection = <Extension<Self> as FromRequest<B>>::Rejection;

    async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
        let Extension(config) = req.extract::<Extension<Self>>().await?;
        Ok(config)
    }
}

impl<S, T, B> Layer<S> for Config<T, B>
where
    T: Clone,
{
    type Service = AddConfig<S, T, B>;

    fn layer(&self, inner: S) -> Self::Service {
        AddConfig {
            inner,
            config: self.config.clone(),
            _marker: self._marker,
        }
    }
}

#[allow(unreachable_pub)]
pub struct AddConfig<S, T, B> {
    inner: S,
    config: T,
    _marker: PhantomData<fn() -> B>,
}

impl<S, T, B> fmt::Debug for AddConfig<S, T, B>
where
    S: fmt::Debug,
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AddConfig")
            .field("inner", &self.inner)
            .field("config", &self.config)
            .finish()
    }
}

impl<S, T, B> Copy for AddConfig<S, T, B>
where
    S: Copy,
    T: Copy,
{
}

impl<S, T, B> Clone for AddConfig<S, T, B>
where
    S: Clone,
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
            _marker: self._marker,
        }
    }
}

impl<S, T, B> Service<Request<B>> for AddConfig<S, T, B>
where
    S: Service<Request<B>>,
    S::Response: IntoResponse,
    T: Clone + Send + Sync + 'static,
    B: Send + 'static,
{
    type Response = Response;
    type Error = S::Error;
    type Future =
        Either<MapOk<S::Future, fn(S::Response) -> Response>, Ready<Result<Response, S::Error>>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: Request<B>) -> Self::Future {
        if req.extensions().get::<Config<T, B>>().is_some() {
            ready(Ok((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "Config of type {:?} was already added. Configs can you be added once",
                    std::any::type_name::<T>()
                ),
            )
                .into_response()))
            .right_future()
        } else {
            req.extensions_mut().insert(Config::<_, B> {
                config: self.config.clone(),
                _marker: PhantomData,
            });
            self.inner
                .call(req)
                .map_ok(IntoResponse::into_response as _)
                .left_future()
        }
    }
}
