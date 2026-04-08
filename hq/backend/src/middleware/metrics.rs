use axum::http::{Method, Request, Response};
use opentelemetry::{
    KeyValue,
    global,
    metrics::{Counter, Histogram},
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};
use tower::{Layer, Service};

#[derive(Clone)]
struct Meters {
    requests_total: Counter<u64>,
    request_duration: Histogram<f64>,
}

impl Meters {
    fn new() -> Self {
        let meter = global::meter("hq");
        Self {
            requests_total: meter
                .u64_counter("hq_http_requests_total")
                .with_description("Total HTTP requests")
                .build(),
            request_duration: meter
                .f64_histogram("hq_http_request_duration_seconds")
                .with_description("HTTP request duration in seconds")
                .with_unit("s")
                .build(),
        }
    }
}

#[derive(Clone)]
pub struct MetricsLayer {
    meters: Arc<Meters>,
}

impl MetricsLayer {
    pub fn new() -> Self {
        Self {
            meters: Arc::new(Meters::new()),
        }
    }
}

impl<S> Layer<S> for MetricsLayer {
    type Service = MetricsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MetricsMiddleware {
            inner,
            meters: self.meters.clone(),
        }
    }
}

#[derive(Clone)]
pub struct MetricsMiddleware<S> {
    inner: S,
    meters: Arc<Meters>,
}

impl<S, ReqBody, ResBody> Service<Request<ReqBody>> for MetricsMiddleware<S>
where
    S: Service<Request<ReqBody>, Response = Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    ReqBody: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<ReqBody>) -> Self::Future {
        let method = req.method().clone();
        let path = req.uri().path().to_string();
        let meters = self.meters.clone();

        // Clone inner so we can move it into the async block
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);

        Box::pin(async move {
            let start = Instant::now();
            let response = inner.call(req).await?;
            let duration = start.elapsed().as_secs_f64();

            let status = response.status().as_u16().to_string();
            let method_str = method_to_str(&method);

            let labels = [
                KeyValue::new("method", method_str),
                KeyValue::new("path", path.clone()),
                KeyValue::new("status", status.clone()),
            ];
            meters.requests_total.add(1, &labels);

            let duration_labels = [
                KeyValue::new("method", method_to_str(&method)),
                KeyValue::new("path", path),
            ];
            meters.request_duration.record(duration, &duration_labels);

            Ok(response)
        })
    }
}

fn method_to_str(method: &Method) -> &'static str {
    match *method {
        Method::GET => "GET",
        Method::POST => "POST",
        Method::PUT => "PUT",
        Method::PATCH => "PATCH",
        Method::DELETE => "DELETE",
        Method::HEAD => "HEAD",
        Method::OPTIONS => "OPTIONS",
        _ => "OTHER",
    }
}
