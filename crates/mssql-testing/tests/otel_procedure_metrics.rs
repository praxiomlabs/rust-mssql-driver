//! Emission test for OpenTelemetry operation metrics on the procedure and
//! named/multi-result paths (issue #146).
//!
//! `call_procedure`, the `procedure()` builder, `query_named`, `execute_named`,
//! and `query_multiple` were previously uninstrumented. This proves each now
//! records a real operation metric, matching the coverage of `query`.
//!
//! Lives in its own test binary (separate process) so the process-global meter
//! provider it installs cannot collide with `otel_metrics.rs` under
//! `cargo test`/`cargo llvm-cov`, which — unlike nextest — runs tests within a
//! binary in one process.
//!
//! Runs in normal CI (the test matrix uses `--all-features`); no live SQL
//! Server required.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use mssql_client::{Client, Config, metric_names};
use mssql_testing::mock_server::MockTdsServer;
use opentelemetry::global;
use opentelemetry_sdk::metrics::data::AggregatedMetrics;
use opentelemetry_sdk::metrics::{InMemoryMetricExporter, PeriodicReader, SdkMeterProvider};

#[tokio::test]
async fn test_procedure_and_named_paths_emit_metrics() {
    let exporter = InMemoryMetricExporter::default();
    let reader = PeriodicReader::builder(exporter.clone()).build();
    let provider = SdkMeterProvider::builder().with_reader(reader).build();
    // Must be installed before `Client::connect` — instruments bind to the
    // global meter at connection setup.
    global::set_meter_provider(provider.clone());

    let server = MockTdsServer::builder().build().await.expect("mock starts");

    let config = Config::from_connection_string(&format!(
        "Server=127.0.0.1,{};User Id=sa;Password=t;Encrypt=no_tls;ConnectRetryCount=0",
        server.port()
    ))
    .expect("config parses");

    let mut client = Client::connect(config).await.expect("connect");

    // Each call exercises a distinct newly-instrumented path. The mock returns
    // its default (empty) response to all of them.
    let _ = client
        .call_procedure("dbo.DoThing", &[])
        .await
        .expect("call_procedure");
    let _ = client
        .procedure("dbo.DoThing")
        .expect("builder")
        .execute()
        .await
        .expect("builder execute");
    let _ = client
        .query_named("SELECT 1", &[])
        .await
        .expect("query_named");
    client
        .execute_named("SELECT 1", &[])
        .await
        .expect("execute_named");
    let _ = client
        .query_multiple("SELECT 1", &[])
        .await
        .expect("query_multiple");

    provider.force_flush().expect("flush");
    let finished = exporter.get_finished_metrics().expect("finished metrics");

    let mut operations_total = 0u64;
    let mut duration_count = 0u64;

    for resource_metrics in &finished {
        for scope in resource_metrics.scope_metrics() {
            for metric in scope.metrics() {
                use opentelemetry_sdk::metrics::data::MetricData;
                match metric.data() {
                    AggregatedMetrics::U64(MetricData::Sum(sum)) => {
                        if metric.name() == metric_names::DB_CLIENT_OPERATIONS_TOTAL {
                            operations_total += sum.data_points().map(|dp| dp.value()).sum::<u64>();
                        }
                    }
                    AggregatedMetrics::F64(MetricData::Histogram(hist)) => {
                        if metric.name() == metric_names::DB_CLIENT_OPERATION_DURATION {
                            for dp in hist.data_points() {
                                duration_count += dp.count();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    assert_eq!(
        operations_total, 5,
        "each of the five instrumented paths must count one operation"
    );
    assert_eq!(
        duration_count, 5,
        "each instrumented path must record a duration sample"
    );

    let _ = client.close().await;
    server.stop();
}
