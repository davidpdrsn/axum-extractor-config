//! Extractors for axum that support runtime configuration.
//!
//! This primarily explores a possible solution to <https://github.com/tokio-rs/axum/issues/1116>.

#![warn(
    clippy::all,
    clippy::dbg_macro,
    clippy::todo,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::mem_forget,
    clippy::unused_self,
    clippy::filter_map_next,
    clippy::needless_continue,
    clippy::needless_borrow,
    clippy::match_wildcard_for_single_variants,
    clippy::if_let_mutex,
    clippy::mismatched_target_os,
    clippy::await_holding_lock,
    clippy::match_on_vec_items,
    clippy::imprecise_flops,
    clippy::suboptimal_flops,
    clippy::lossy_float_literal,
    clippy::rest_pat_in_fully_bound_structs,
    clippy::fn_params_excessive_bools,
    clippy::exit,
    clippy::inefficient_to_string,
    clippy::linkedlist,
    clippy::macro_use_imports,
    clippy::option_option,
    clippy::verbose_file_reads,
    clippy::unnested_or_patterns,
    clippy::str_to_string,
    rust_2018_idioms,
    future_incompatible,
    nonstandard_style,
    missing_debug_implementations,
    missing_docs
)]
#![deny(unreachable_pub, private_in_public)]
#![allow(elided_lifetimes_in_paths, clippy::type_complexity)]
#![forbid(unsafe_code)]
#![cfg_attr(docsrs, feature(doc_auto_cfg, doc_cfg))]
#![cfg_attr(test, allow(clippy::float_cmp))]

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
            rejection_handler: crate::RejectionToResponseFn<axum::extract::rejection::$rejection, B>,
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
            use crate::config::Config;
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
    /// use axum_extractor_config::{
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
    /// use axum_extractor_config::{
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
    /// use axum_extractor_config::{
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
