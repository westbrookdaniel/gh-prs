use crate::http::{MiddlewareFuture, Next, Request, Response};
use opentelemetry::KeyValue;
use opentelemetry::global;
use opentelemetry::propagation::{Extractor, Injector};
use opentelemetry::trace::{SpanKind, TraceContextExt, TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::collections::HashMap;
use std::env;
use std::io;
use std::sync::OnceLock;
use tracing::Instrument;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};

const DEFAULT_OTLP_ENDPOINT: &str = "http://127.0.0.1:4318";
const DEFAULT_SERVICE_NAME: &str = "gh-prs";

static TELEMETRY_STATE: OnceLock<TelemetryState> = OnceLock::new();

pub struct TelemetryGuard {
    provider: Option<SdkTracerProvider>,
}

struct TelemetryState {
    exports_enabled: bool,
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(provider) = self.provider.take() {
            let _ = provider.shutdown();
        }
    }
}

pub fn init_tracing() -> io::Result<TelemetryGuard> {
    global::set_text_map_propagator(TraceContextPropagator::new());

    let exports_enabled = otel_exports_enabled();
    let provider = if exports_enabled {
        Some(build_tracer_provider()?)
    } else {
        None
    };

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = fmt::layer().with_target(false).with_thread_ids(false);
    let registry = tracing_subscriber::registry().with(filter).with(fmt_layer);

    if let Some(provider) = &provider {
        let tracer = provider.tracer(DEFAULT_SERVICE_NAME);
        registry
            .with(tracing_opentelemetry::layer().with_tracer(tracer))
            .try_init()
            .map_err(io::Error::other)?;
    } else {
        registry.try_init().map_err(io::Error::other)?;
    }

    let _ = TELEMETRY_STATE.set(TelemetryState { exports_enabled });

    if exports_enabled {
        tracing::info!(
            otel.endpoint = %otel_endpoint(),
            "opentelemetry request tracing enabled"
        );
    } else {
        tracing::info!(
            "opentelemetry exporter disabled; set OTEL_EXPORTER_OTLP_ENDPOINT to send traces to a viewer"
        );
    }

    Ok(TelemetryGuard { provider })
}

pub fn request_tracing() -> impl Fn(Request, Next) -> MiddlewareFuture + Send + Sync + 'static {
    |request: Request, next: Next| {
        Box::pin(async move {
            let parent_context = global::get_text_map_propagator(|propagator| {
                propagator.extract(&RequestHeaderExtractor::new(&request.headers))
            });
            let method = request.method.clone();
            let path = request.path.clone();
            let matched_route = request.matched_route().map(str::to_owned);
            let version = request.version.clone();
            let request_id = request
                .header("x-request-id")
                .unwrap_or("missing")
                .to_string();
            let span = tracing::info_span!(
                "http.server.request",
                otel.kind = ?SpanKind::Server,
                otel.name = tracing::field::Empty,
                http.request.method = %method,
                http.route = tracing::field::Empty,
                url.path = %path,
                url.query = tracing::field::Empty,
                network.protocol.version = %version,
                user_agent.original = tracing::field::Empty,
                http.request_id = %request_id,
                http.response.status_code = tracing::field::Empty,
                otel.trace_id = tracing::field::Empty,
            );
            let span_route = matched_route.as_deref().unwrap_or(&path);
            span.record("otel.name", format_args!("{method} {span_route}"));
            if let Some(route) = matched_route.as_deref() {
                span.record("http.route", route);
            }
            if let Some(query) = &request.query {
                span.record("url.query", query.as_str());
            }
            if let Some(user_agent) = request.header("user-agent") {
                span.record("user_agent.original", user_agent);
            }
            span.set_parent(parent_context);

            let mut response: Response = next.run(request).instrument(span.clone()).await;
            let status_code = response.status_code();
            span.record("http.response.status_code", status_code as i64);

            let trace_id = span.context().span().span_context().trace_id().to_string();
            span.record("otel.trace_id", trace_id.as_str());
            response.insert_header("X-Trace-Id", trace_id.clone());

            if telemetry_exports_enabled() {
                let context = span.context();
                global::get_text_map_propagator(|propagator| {
                    propagator
                        .inject_context(&context, &mut ResponseHeaderInjector::new(&mut response));
                });
            }

            response
        })
    }
}

fn build_tracer_provider() -> io::Result<SdkTracerProvider> {
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(otel_endpoint())
        .build()
        .map_err(io::Error::other)?;

    Ok(SdkTracerProvider::builder()
        .with_simple_exporter(exporter)
        .with_resource(
            Resource::builder_empty()
                .with_attributes([
                    KeyValue::new("service.name", otel_service_name()),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ])
                .build(),
        )
        .build())
}

fn telemetry_exports_enabled() -> bool {
    TELEMETRY_STATE
        .get()
        .is_some_and(|state| state.exports_enabled)
}

fn otel_exports_enabled() -> bool {
    env::var("OTEL_SDK_DISABLED")
        .map(|value| !value.eq_ignore_ascii_case("true"))
        .unwrap_or(true)
}

fn otel_endpoint() -> String {
    env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| DEFAULT_OTLP_ENDPOINT.to_string())
}

fn otel_service_name() -> String {
    env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| DEFAULT_SERVICE_NAME.to_string())
}

struct RequestHeaderExtractor<'a> {
    headers: &'a HashMap<String, String>,
}

impl<'a> RequestHeaderExtractor<'a> {
    fn new(headers: &'a HashMap<String, String>) -> Self {
        Self { headers }
    }
}

impl Extractor for RequestHeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.headers
            .get(&key.to_ascii_lowercase())
            .map(String::as_str)
    }

    fn keys(&self) -> Vec<&str> {
        self.headers.keys().map(String::as_str).collect()
    }
}

struct ResponseHeaderInjector<'a> {
    response: &'a mut Response,
}

impl<'a> ResponseHeaderInjector<'a> {
    fn new(response: &'a mut Response) -> Self {
        Self { response }
    }
}

impl Injector for ResponseHeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.response.insert_header(key, value);
    }
}
