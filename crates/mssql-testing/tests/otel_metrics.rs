//! Emission test for OpenTelemetry operation metrics.
//!
//! README claims "Query + pool lifecycle" metrics via the `otel` feature.
//! This proves the query half actually emits **real values** — duration
//! histogram, operations counter, errors counter — by capturing them with an
//! in-memory exporter while a client runs queries against the mock server.
//!
//! Runs in normal CI (the test matrix uses `--all-features`); no live SQL
//! Server required.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config, metric_names};
use mssql_testing::mock_server::{MockResponse, MockTdsServer};
use opentelemetry::global;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};

#[tokio::test]
async fn test_query_metrics_emit_real_values() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    // Must be installed before `Client::connect` — instruments bind to the
    // global meter at connection setup.
    global::set_meter_provider(provider.clone());

    let server = MockTdsServer::builder()
        .with_response("SELECT 7", MockResponse::scalar_int(7))
        .with_response("SELECT boom", MockResponse::error(50000, "boom"))
        .build()
        .await
        .expect("mock starts");

    let config = Config::from_connection_string(&format!(
        "Server=127.0.0.1,{};User Id=sa;Password=t;Encrypt=no_tls;ConnectRetryCount=0",
        server.port()
    ))
    .expect("config parses");

    let mut client = Client::connect(config).await.expect("connect");
    for _ in 0..3 {
        let rows = client.query("SELECT 7", &[]).await.expect("query");
        assert_eq!(rows.into_iter().count(), 1);
    }
    assert!(
        client.query("SELECT boom", &[]).await.is_err(),
        "server error expected"
    );

    provider.force_flush().expect("flush");
    let finished = exporter.get_finished_metrics().expect("finished metrics");

    let mut operations_total = 0u64;
    let mut errors_total = 0u64;
    let mut duration_count = 0u64;
    let mut duration_sum = 0.0f64;

    for resource_metrics in &finished {
        for scope in resource_metrics.scope_metrics() {
            for metric in scope.metrics() {
                use opentelemetry_sdk::metrics::data::MetricData;
                match metric.data() {
                    AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                        let total: u64 = sum.data_points().map(|dp| dp.value()).sum();
                        if metric.name() == metric_names::DB_CLIENT_OPERATIONS_TOTAL {
                            operations_total += total;
                        } else if metric.name() == metric_names::DB_CLIENT_ERRORS_TOTAL {
                            errors_total += total;
                        }
                    }
                    AggregatedMetrics::F64(MetricData::Histogram(hist)) => {
                        if metric.name() == metric_names::DB_CLIENT_OPERATION_DURATION {
                            for dp in hist.data_points() {
                                duration_count += dp.count();
                                duration_sum += dp.sum();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    assert_eq!(
        operations_total, 4,
        "three successful queries plus one failed query must be counted"
    );
    assert_eq!(errors_total, 1, "the failed query must count as an error");
    assert_eq!(
        duration_count, 4,
        "every operation must record a duration sample"
    );
    assert!(
        duration_sum > 0.0,
        "durations must carry real (non-zero) values"
    );

    let _ = client.close().await;
    server.stop();
}
