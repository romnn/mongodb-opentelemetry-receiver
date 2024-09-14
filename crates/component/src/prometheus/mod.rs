use color_eyre::eyre;
use serde::Deserialize;
use std::collections::HashMap;
use std::time::Duration;

// // ServerConfig defines settings for creating an HTTP server.
// type ServerConfig struct {
// 	// Endpoint configures the listening address for the server.
// 	Endpoint string `mapstructure:"endpoint"`
//
// 	// TLSSetting struct exposes TLS client configuration.
// 	TLSSetting *configtls.ServerConfig `mapstructure:"tls"`
//
// 	// CORS configures the server for HTTP cross-origin resource sharing (CORS).
// 	CORS *CORSConfig `mapstructure:"cors"`
//
// 	// Auth for this receiver
// 	Auth *AuthConfig `mapstructure:"auth"`
//
// 	// MaxRequestBodySize sets the maximum request body size in bytes. Default: 20MiB.
// 	MaxRequestBodySize int64 `mapstructure:"max_request_body_size"`
//
// 	// IncludeMetadata propagates the client metadata from the incoming requests to the downstream consumers
// 	IncludeMetadata bool `mapstructure:"include_metadata"`
//
// 	// Additional headers attached to each HTTP response sent to the client.
// 	// Header values are opaque since they may be sensitive.
// 	ResponseHeaders map[string]configopaque.String `mapstructure:"response_headers"`
//
// 	// CompressionAlgorithms configures the list of compression algorithms the server can accept. Default: ["", "gzip", "zstd", "zlib", "snappy", "deflate"]
// 	CompressionAlgorithms []string `mapstructure:"compression_algorithms"`
//
// 	// ReadTimeout is the maximum duration for reading the entire
// 	// request, including the body. A zero or negative value means
// 	// there will be no timeout.
// 	//
// 	// Because ReadTimeout does not let Handlers make per-request
// 	// decisions on each request body's acceptable deadline or
// 	// upload rate, most users will prefer to use
// 	// ReadHeaderTimeout. It is valid to use them both.
// 	ReadTimeout time.Duration `mapstructure:"read_timeout"`
//
// 	// ReadHeaderTimeout is the amount of time allowed to read
// 	// request headers. The connection's read deadline is reset
// 	// after reading the headers and the Handler can decide what
// 	// is considered too slow for the body. If ReadHeaderTimeout
// 	// is zero, the value of ReadTimeout is used. If both are
// 	// zero, there is no timeout.
// 	ReadHeaderTimeout time.Duration `mapstructure:"read_header_timeout"`
//
// 	// WriteTimeout is the maximum duration before timing out
// 	// writes of the response. It is reset whenever a new
// 	// request's header is read. Like ReadTimeout, it does not
// 	// let Handlers make decisions on a per-request basis.
// 	// A zero or negative value means there will be no timeout.
// 	WriteTimeout time.Duration `mapstructure:"write_timeout"`
//
// 	// IdleTimeout is the maximum amount of time to wait for the
// 	// next request when keep-alives are enabled. If IdleTimeout
// 	// is zero, the value of ReadTimeout is used. If both are
// 	// zero, there is no timeout.
// 	IdleTimeout time.Duration `mapstructure:"idle_timeout"`
// }

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub namespace: Option<String>,
    #[serde(default)]
    pub const_labels: HashMap<String, String>,
    pub send_timestamps: Option<bool>,
    pub metric_expiration: Option<Duration>,
    pub enable_open_metrics: Option<bool>,
    pub add_metric_suffixes: Option<bool>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            namespace: None,
            const_labels: HashMap::new(),
            send_timestamps: Some(false),
            metric_expiration: Some(Duration::from_secs(5 * 60)),
            enable_open_metrics: Some(false),
            add_metric_suffixes: Some(true),
        }
    }
}

#[derive(Debug)]
pub struct Collector {
    // send_timestamps: bool,
    // addMetricSuffixes: bool
    // namespace: String;
    // constLabels: HashMap<String, String>,
}

impl prometheus::core::Collector for Collector {
    fn desc(&self) -> Vec<&prometheus::core::Desc> {
        todo!()
    }

    fn collect(&self) -> Vec<prometheus::proto::MetricFamily> {
        todo!()
    }
}

#[derive(Debug)]
pub struct PrometheusExporter {}

impl PrometheusExporter {
    pub fn new() -> eyre::Result<Self> {
        let collector = Collector {};
        let registry = prometheus::Registry::new();
        registry.register(Box::new(collector))?;
        Ok(Self {})
    }
}

// return &prometheusExporter{
// 	config:       *config,
// 	name:         set.ID.String(),
// 	endpoint:     addr,
// 	collector:    collector,
// 	registry:     registry,
// 	shutdownFunc: func() error { return nil },
// 	handler: promhttp.HandlerFor(
// 		registry,
// 		promhttp.HandlerOpts{
// 			ErrorHandling:     promhttp.ContinueOnError,
// 			ErrorLog:          newPromLogger(set.Logger),
// 			EnableOpenMetrics: config.EnableOpenMetrics,
// 		},
// 	),
// 	settings: set.TelemetrySettings,
// }, nil

// func createDefaultConfig() component.Config {
// 	return &Config{
// 		ConstLabels:       map[string]string{},
// 		SendTimestamps:    false,
// 		MetricExpiration:  time.Minute * 5,
// 		EnableOpenMetrics: false,
// 		AddMetricSuffixes: true,
// 	}
// }
