//! Extractors that are configured via types.

use axum::{
    async_trait,
    body::{Bytes, HttpBody},
    extract::{
        rejection::{FormRejection, JsonRejection, QueryRejection},
        FromRequest, RequestParts,
    },
    response::{IntoResponse, Response},
    BoxError,
};
use serde::{de::DeserializeOwned, Serialize};
use std::{convert::Infallible, fmt, marker::PhantomData};

/// Trait for converting rejections into custom responses.
///
/// See [`Json`], [`Query`], or [`Form`] for examples.
#[async_trait]
pub trait IntoResponseFromRejection<T, B> {
    /// The response the rejection is converted into.
    type Response: IntoResponse;

    /// Create the response from a rejection.
    async fn into_response_from_rejection(rejection: T, req: &mut RequestParts<B>) -> Self::Response;
}

macro_rules! make_deserialize_wrapper {
    (
        $(#[$m:meta])*
        $ident:ident,
        $rejection:ident, $(,)?
    ) => {
        #[async_trait]
        impl<B> IntoResponseFromRejection<$rejection, B> for $rejection
        where
            B: Send,
        {
            type Response = $rejection;

            async fn into_response_from_rejection(
                rejection: $rejection,
                _req: &mut RequestParts<B>,
            ) -> Self::Response {
                rejection
            }
        }

        $(#[$m])*
        pub struct $ident<T, C>(pub T, pub PhantomData<fn() -> C>);

        #[async_trait]
        impl<T, C, B> FromRequest<B> for $ident<T, C>
        where
            B: HttpBody<Data = Bytes> + Send + 'static,
            B::Error: Into<BoxError>,
            T: DeserializeOwned + Send,
            C: IntoResponseFromRejection<$rejection, B>,
        {
            type Rejection = C::Response;

            async fn from_request(req: &mut RequestParts<B>) -> Result<Self, Self::Rejection> {
                match req.extract::<axum::extract::$ident<T>>().await {
                    Ok(axum::extract::$ident(value)) => Ok(Self(value, PhantomData)),
                    Err(rejection) => Err(C::into_response_from_rejection(rejection, req).await),
                }
            }
        }

        impl<T, C> Clone for $ident<T, C>
        where
            T: Clone,
        {
            fn clone(&self) -> Self {
                Self(self.0.clone(), self.1)
            }
        }

        impl<T, C> Copy for $ident<T, C> where T: Copy {}

        impl<T, C> fmt::Debug for $ident<T, C>
        where
            T: fmt::Debug,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($ident)).field(&self.0).finish()
            }
        }

        impl<T, C> Default for $ident<T, C>
        where
            T: Default,
        {
            fn default() -> Self {
                Self(Default::default(), Default::default())
            }
        }
    };
}

make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Json` and supports compile time configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_types::{
    ///     // make sure to use this `Json`, and not the one in axum
    ///     Json,
    ///     IntoResponseFromRejection,
    /// };
    /// use axum::{
    ///     async_trait,
    ///     Router,
    ///     routing::post,
    ///     extract::{RequestParts, rejection::JsonRejection},
    ///     response::{IntoResponse, Response},
    ///     http::StatusCode,
    /// };
    /// use serde::Deserialize;
    /// use serde_json::{json, Value};
    ///
    /// struct CustomRejection;
    ///
    /// #[async_trait]
    /// impl<B> IntoResponseFromRejection<JsonRejection, B> for CustomRejection
    /// where
    ///     B: Send,
    /// {
    ///     type Response = Response;
    ///
    ///     async fn into_response_from_rejection(
    ///         rejection: JsonRejection,
    ///         _req: &mut RequestParts<B>,
    ///     ) -> Self::Response {
    ///         (
    ///             StatusCode::BAD_REQUEST,
    ///             Json::new(json!({ "error": rejection.to_string() })),
    ///         ).into_response()
    ///     }
    /// }
    ///
    /// #[derive(Deserialize)]
    /// struct Payload {}
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Json(payload, _): Json<Payload, CustomRejection>) {}
    ///
    /// let app = Router::new().route("/", post(handler));
    /// # let _: Router = app;
    /// ```
    Json,
    JsonRejection,
}

impl<T> Json<T, Infallible> {
    /// Create a new `Json` that will implement `IntoResponse`.
    pub fn new(value: T) -> Self {
        Self(value, PhantomData)
    }
}

impl<T, C> IntoResponse for Json<T, C>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Query` and supports compile time configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_types::{
    ///     // make sure to use this `Query`, and not the one in axum
    ///     Query,
    ///     IntoResponseFromRejection,
    /// };
    /// use axum::{
    ///     async_trait,
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
    /// struct CustomRejection(Response);
    ///
    /// #[async_trait]
    /// impl<B> IntoResponseFromRejection<QueryRejection, B> for CustomRejection
    /// where
    ///     B: Send,
    /// {
    ///     type Response = Response;
    ///
    ///     async fn into_response_from_rejection(
    ///         rejection: QueryRejection,
    ///         _req: &mut RequestParts<B>,
    ///     ) -> Self::Response {
    ///         (
    ///             StatusCode::BAD_REQUEST,
    ///             Json(json!({ "error": rejection.to_string() })),
    ///         ).into_response()
    ///     }
    /// }
    ///
    /// #[derive(Deserialize)]
    /// struct Pagination {
    ///     page: u32,
    ///     per_page: u32,
    /// }
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Query(pagination, _): Query<Pagination, CustomRejection>) {}
    ///
    /// let app = Router::new().route("/", get(handler));
    /// # let _: Router = app;
    /// ```
    Query,
    QueryRejection,
}

make_deserialize_wrapper! {
    /// Extractor that wraps `axum::extract::Form` and supports compile time configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use axum_extractor_config::via_types::{
    ///     // make sure to use this `Form`, and not the one in axum
    ///     Form,
    ///     IntoResponseFromRejection,
    /// };
    /// use axum::{
    ///     async_trait,
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
    /// struct CustomRejection(Response);
    ///
    /// #[async_trait]
    /// impl<B> IntoResponseFromRejection<FormRejection, B> for CustomRejection
    /// where
    ///     B: Send,
    /// {
    ///     type Response = Response;
    ///
    ///     async fn into_response_from_rejection(
    ///         rejection: FormRejection,
    ///         _req: &mut RequestParts<B>,
    ///     ) -> Self::Response {
    ///         (
    ///             StatusCode::BAD_REQUEST,
    ///             Json(json!({ "error": rejection.to_string() })),
    ///         ).into_response()
    ///     }
    /// }
    ///
    /// #[derive(Deserialize)]
    /// struct Payload {}
    ///
    /// #[axum::debug_handler]
    /// async fn handler(Form(payload, _): Form<Payload, CustomRejection>) {}
    ///
    /// let app = Router::new().route("/", post(handler));
    /// # let _: Router = app;
    /// ```
    Form,
    FormRejection,
}

impl<T> Form<T, Infallible> {
    /// Create a new `Form` that will implement `IntoResponse`.
    pub fn new(value: T) -> Self {
        Self(value, PhantomData)
    }
}

impl<T, C> IntoResponse for Form<T, C>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        axum::Form(self.0).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::{rejection::JsonRejection, RequestParts},
        http::{Method, Request, StatusCode},
        response::{IntoResponse, Response},
        routing::post,
        Router,
    };
    use serde::{Deserialize, Serialize};
    use serde_json::{json, Value};
    use std::error::Error;
    use tower::Service;

    #[derive(Deserialize)]
    struct Payload {
        #[allow(dead_code)]
        id: u32,
    }

    fn app<T>() -> Router<Body>
    where
        T: IntoResponseFromRejection<JsonRejection, Body> + 'static,
    {
        async fn handler<T>(Json(_payload, _): Json<Payload, T>) {}

        Router::new().route("/", post(handler::<T>))
    }

    #[tokio::test]
    async fn json_ok() {
        let mut app = app::<JsonRejection>();

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
        let mut app = app::<JsonRejection>();

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
        struct CustomRejection(Response);

        #[async_trait]
        impl<B> IntoResponseFromRejection<JsonRejection, B> for CustomRejection
        where
            B: Send,
        {
            type Response = Response;

            async fn into_response_from_rejection(
                rejection: JsonRejection,
                _req: &mut RequestParts<B>,
            ) -> Self::Response {
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

                (default_status, Json::new(Error { message, details })).into_response()
            }
        }

        let mut app = app::<CustomRejection>();

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
}
