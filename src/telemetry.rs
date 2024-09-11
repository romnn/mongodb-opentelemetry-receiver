#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, strum::IntoStaticStr)]
pub enum LogLevel {
    /// None indicates that no telemetry data should be collected.
    None,
    /// Basic is the recommended and covers the basics of the service telemetry.
    Basic,
    /// Normal adds some other indicators on top of basic.
    Normal,
    /// Detailed adds dimensions and views to the previous levels.
    Detailed,
}

impl LogLevel {
    pub fn as_str(&self) -> &'static str {
        self.into()
    }
}

/// TelemetrySettings provides components with APIs to report telemetry.
pub struct TelemetrySettings {
    // /// Logger
    // Logger *zap.Logger
    /// TracerProvider that the factory can pass to other instrumented third-party libraries.
    tracer_provider: opentelemetry_sdk::trace::TracerProvider,

    /// MeterProvider that the factory can pass to other instrumented third-party libraries.
    meter_provider: opentelemetry_sdk::metrics::SdkMeterProvider,
    // LeveledMeterProvider func(level configtelemetry.Level) metric.MeterProvider
    /// Rhe configuration value set when the collector
    /// is configured. Components may use this level to decide whether it is
    /// appropriate to avoid computationally expensive calculations.
    metrics_level: LogLevel,

    /// Resource contains the resource attributes for the collector's telemetry.
    resource: opentelemetry_sdk::Resource,
}
