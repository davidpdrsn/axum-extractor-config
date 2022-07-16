//! Extractors that are configured via request extensions.

use axum::response::Response;
use std::sync::Arc;

mod config;
pub use config::Config;

type RejectionToResponseFn<T, B> =
    Option<Arc<dyn Fn(T, &axum::extract::RequestParts<B>) -> Response + Send + Sync>>;

macro_rules! make_deserialize_wrapper {
    (
        $(#[$m:meta])*
        $ident:ident,
        $rejection:ident,
        $config:ident $(,)?
    ) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug)]
        pub struct $ident<T>(pub T);

        #[doc = concat!("Config type for `", stringify!($ident), "`")]
        pub struct $config<B> {
            rejection_handler: RejectionToResponseFn<axum::extract::rejection::$rejection, B>,
        }

        impl<B> $config<B> {
            #[doc = concat!("Create a new `", stringify!($config), "`")]
            pub fn new() -> Self {
                Self::default()
            }

            /// Set the rejection handler function.
            pub fn rejection_handler<F, R>(mut self, f: F) -> Self
            where
                F: Fn(axum::extract::rejection::$rejection, &axum::extract::RequestParts<B>) -> R + Send + Sync + 'static,
                R: axum::response::IntoResponse,
            {
                self.rejection_handler = Some(Arc::new(move |rejection, req| {
                    f(rejection, req).into_response()
                }));
                self
            }
        }

        impl<B> Clone for $config<B> {
            fn clone(&self) -> Self {
                Self {
                    rejection_handler: self.rejection_handler.clone(),
                }
            }
        }

        impl<B> Default for $config<B> {
            fn default() -> Self {
                Self {
                    rejection_handler: None,
                }
            }
        }

        const _: () = {
            use axum::{
                async_trait,
                body::{Bytes, HttpBody},
                extract::{FromRequest, RequestParts},
                response::{IntoResponse, Response},
                BoxError,
            };
            use serde::de::DeserializeOwned;
            use std::fmt;

            #[async_trait]
            impl<T, B> FromRequest<B> for $ident<T>
            where
                B: HttpBody<Data = Bytes> + Send + 'static,
                B::Error: Into<BoxError>,
                T: DeserializeOwned + Send,
            {
                type Rejection = Response;

                async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                    match req.extract::<axum::extract::$ident<T>>().await {
                        Ok(axum::extract::$ident(value)) => Ok(Self(value)),
                        Err(rejection) => {
                            let config =
                                req.extract::<Config<$config<B>, B>>()
                                    .await
                                    .unwrap_or_default()
                                    .into_inner();

                            if let Some(rejection_handler) = &config.rejection_handler {
                                Err(rejection_handler(rejection, req))
                            } else {
                                Err(rejection.into_response())
                            }
                        }
                    }
                }
            }

            impl<B> fmt::Debug for $config<B> {
                fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.debug_struct(stringify!($config)).finish()
                }
            }

            impl<S, B> tower_layer::Layer<S> for $config<B> {
                type Service = <Config<Self, B> as tower_layer::Layer<S>>::Service;

                fn layer(&self, inner: S) -> Self::Service {
                    let config: Config::<_, B> = Config::new(self.clone());
                    config.layer(inner)
                }
            }
        };
    };
}

#[cfg(feature = "json")]
make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Json` and supports runtime configuration.
    ///
    /// Can be configured using [`JsonConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_extensions::{
    ///     // make sure to use this `Json`, and not the one in axum
    ///     Json,
    ///     JsonConfig,
    /// };
    /// use axum::{
    ///     Router,
    ///     routing::post,
    ///     extract::{RequestParts, rejection::JsonRejection},
    ///     response::{IntoResponse, Response},
    ///     http::StatusCode,
    /// };
    /// use serde::Deserialize;
    /// use serde_json::{json, Value};
    ///
    /// #[derive(Deserialize)]
    /// struct Payload {}
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Json(payload): Json<Payload>) {}
    ///
    /// fn rejection_handler<B>(rejection: JsonRejection, req: &RequestParts<B>) -> (StatusCode, Json<Value>) {
    ///     (
    ///         StatusCode::BAD_REQUEST,
    ///         Json(json!({ "error": rejection.to_string() })),
    ///     )
    /// }
    ///
    /// let app = Router::new()
    ///     .route("/", post(handler))
    ///     .layer(JsonConfig::new().rejection_handler(rejection_handler));
    /// # let _: Router = app;
    /// ```
    Json,
    JsonRejection,
    JsonConfig,
}

#[cfg(feature = "json")]
impl<T> axum::response::IntoResponse for Json<T>
where
    T: serde::Serialize,
{
    fn into_response(self) -> axum::response::Response {
        axum::Json(self.0).into_response()
    }
}

#[cfg(feature = "query")]
make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Query` and supports runtime configuration.
    ///
    /// Can be configured using [`QueryConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_extensions::{
    ///     // make sure to use this `Query`, and not the one in axum
    ///     Query,
    ///     QueryConfig,
    /// };
    /// use axum::{
    ///     Router,
    ///     Json,
    ///     routing::get,
    ///     extract::{RequestParts, rejection::QueryRejection},
    ///     response::{IntoResponse, Response},
    ///     http::StatusCode,
    /// };
    /// use serde::Deserialize;
    /// use serde_json::{json, Value};
    ///
    /// #[derive(Deserialize)]
    /// struct Pagination {
    ///     page: u32,
    ///     per_page: u32,
    /// }
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Query(payload): Query<Pagination>) {}
    ///
    /// fn rejection_handler<B>(rejection: QueryRejection, req: &RequestParts<B>) -> (StatusCode, Json<Value>) {
    ///     (
    ///         StatusCode::BAD_REQUEST,
    ///         Json(json!({ "error": rejection.to_string() })),
    ///     )
    /// }
    ///
    /// let app = Router::new()
    ///     .route("/", get(handler))
    ///     .layer(QueryConfig::new().rejection_handler(rejection_handler));
    /// # let _: Router = app;
    /// ```
    Query,
    QueryRejection,
    QueryConfig,
}

#[cfg(feature = "form")]
make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Form` and supports runtime configuration.
    ///
    /// Can be configured using [`FormConfig`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_extensions::{
    ///     // make sure to use this `Form`, and not the one in axum
    ///     Form,
    ///     FormConfig,
    /// };
    /// use axum::{
    ///     Router,
    ///     Json,
    ///     routing::post,
    ///     extract::{RequestParts, rejection::FormRejection},
    ///     response::{IntoResponse, Response},
    ///     http::StatusCode,
    /// };
    /// use serde::Deserialize;
    /// use serde_json::{json, Value};
    ///
    /// #[derive(Deserialize)]
    /// struct Payload {}
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Form(payload): Form<Payload>) {}
    ///
    /// fn rejection_handler<B>(rejection: FormRejection, req: &RequestParts<B>) -> (StatusCode, Json<Value>) {
    ///     (
    ///         StatusCode::BAD_REQUEST,
    ///         Json(json!({ "error": rejection.to_string() })),
    ///     )
    /// }
    ///
    /// let app = Router::new()
    ///     .route("/", post(handler))
    ///     .layer(FormConfig::new().rejection_handler(rejection_handler));
    /// # let _: Router = app;
    /// ```
    Form,
    FormRejection,
    FormConfig,
}

#[cfg(feature = "form")]
impl<T> axum::response::IntoResponse for Form<T>
where
    T: serde::Serialize,
{
    fn into_response(self) -> axum::response::Response {
        axum::Form(self.0).into_response()
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::*;
    use axum::{
        body::Body,
        extract::{rejection::JsonRejection, RequestParts},
        http::{Method, Request, StatusCode},
        response::IntoResponse,
        routing::post,
        Router,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use tower::Service;

    #[derive(Deserialize)]
    struct Payload {
        #[allow(dead_code)]
        id: u32,
    }

    fn app() -> Router<Body> {
        async fn handler(Json(_): Json<Payload>) {}

        Router::new().route("/", post(handler))
    }

    #[tokio::test]
    async fn json_ok() {
        let mut app = app();

        let body = json!({ "id": 123 }).to_string();
        let res = app
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(body, "");
    }

    #[tokio::test]
    async fn json_default_rejection() {
        let mut app = app();

        let body = json!({ "id": "foo" }).to_string();
        let res = app
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body,
            "Failed to deserialize the JSON body into the target type: \
            invalid type: string \"foo\", expected u32 at line 1 column 11"
        );
    }

    #[tokio::test]
    async fn json_custom_rejection() {
        fn rejection_handler<B>(rejection: JsonRejection, _req: &RequestParts<B>) -> Response {
            #[derive(Serialize)]
            struct Error {
                message: String,
                details: Option<String>,
            }

            let message = rejection.to_string();
            let source = rejection.source().and_then(|source| source.source());
            let details = source.map(|s| s.to_string());

            let default_response = rejection.into_response();
            let default_status = default_response.status();

            (default_status, Json(Error { message, details })).into_response()
        }

        let mut app = app().layer(JsonConfig::new().rejection_handler(rejection_handler));

        let body = json!({ "id": "foo" }).to_string();
        let res = app
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(res.headers()["content-type"], "application/json");
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = serde_json::from_slice::<Value>(&body[..]).unwrap();
        assert_eq!(
            body,
            json!({
                "message": "Failed to deserialize the JSON body into the target type",
                "details": "invalid type: string \"foo\", expected u32 at line 1 column 11",
            })
        );
    }

    #[tokio::test]
    async fn error_on_duplicate_config() {
        let mut app = app().layer(JsonConfig::new()).layer(JsonConfig::new());

        let res = app
            .call(
                Request::builder()
                    .method(Method::POST)
                    .uri("/")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(res.headers()["content-type"], "text/plain; charset=utf-8");
        let body = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body,
            "Config of type \"axum_extractor_config::via_extensions::\
            JsonConfig<hyper::body::body::Body>\" was already added. \
            Configs can you be added once"
        );
    }
}
