// ResourceBuilder is a helper struct to build resources predefined in metadata.yaml.
// The ResourceBuilder is not thread-safe and must not to be used in multiple goroutines.
type ResourceBuilder struct {
	config ResourceAttributesConfig
	res    pcommon.Resource
}

// NewResourceBuilder creates a new ResourceBuilder. This method should be called on the start of the application.
func NewResourceBuilder(rac ResourceAttributesConfig) *ResourceBuilder {
	return &ResourceBuilder{
		config: rac,
		res:    pcommon.NewResource(),
	}
}

// SetDatabase sets provided value as "database" attribute.
func (rb *ResourceBuilder) SetDatabase(val string) {
	if rb.config.Database.Enabled {
		rb.res.Attributes().PutStr("database", val)
	}
}

// SetServerAddress sets provided value as "server.address" attribute.
func (rb *ResourceBuilder) SetServerAddress(val string) {
	if rb.config.ServerAddress.Enabled {
		rb.res.Attributes().PutStr("server.address", val)
	}
}

// SetServerPort sets provided value as "server.port" attribute.
func (rb *ResourceBuilder) SetServerPort(val int64) {
	if rb.config.ServerPort.Enabled {
		rb.res.Attributes().PutInt("server.port", val)
	}
}

// Emit returns the built resource and resets the internal builder state.
func (rb *ResourceBuilder) Emit() pcommon.Resource {
	r := rb.res
	rb.res = pcommon.NewResource()
	return r
}

package metadata

import (
	"go.opentelemetry.io/collector/confmap"
	"go.opentelemetry.io/collector/filter"
)

// MetricConfig provides common config for a particular metric.
type MetricConfig struct {
	Enabled bool `mapstructure:"enabled"`

	enabledSetByUser bool
}

func (ms *MetricConfig) Unmarshal(parser *confmap.Conf) error {
	if parser == nil {
		return nil
	}
	err := parser.Unmarshal(ms)
	if err != nil {
		return err
	}
	ms.enabledSetByUser = parser.IsSet("enabled")
	return nil
}

// MetricsConfig provides config for mongodb metrics.
type MetricsConfig struct {
	MongodbCacheOperations        MetricConfig `mapstructure:"mongodb.cache.operations"`
	MongodbCollectionCount        MetricConfig `mapstructure:"mongodb.collection.count"`
	MongodbConnectionCount        MetricConfig `mapstructure:"mongodb.connection.count"`
	MongodbCursorCount            MetricConfig `mapstructure:"mongodb.cursor.count"`
	MongodbCursorTimeoutCount     MetricConfig `mapstructure:"mongodb.cursor.timeout.count"`
	MongodbDataSize               MetricConfig `mapstructure:"mongodb.data.size"`
	MongodbDatabaseCount          MetricConfig `mapstructure:"mongodb.database.count"`
	MongodbDocumentOperationCount MetricConfig `mapstructure:"mongodb.document.operation.count"`
	MongodbExtentCount            MetricConfig `mapstructure:"mongodb.extent.count"`
	MongodbGlobalLockTime         MetricConfig `mapstructure:"mongodb.global_lock.time"`
	MongodbHealth                 MetricConfig `mapstructure:"mongodb.health"`
	MongodbIndexAccessCount       MetricConfig `mapstructure:"mongodb.index.access.count"`
	MongodbIndexCount             MetricConfig `mapstructure:"mongodb.index.count"`
	MongodbIndexSize              MetricConfig `mapstructure:"mongodb.index.size"`
	MongodbLockAcquireCount       MetricConfig `mapstructure:"mongodb.lock.acquire.count"`
	MongodbLockAcquireTime        MetricConfig `mapstructure:"mongodb.lock.acquire.time"`
	MongodbLockAcquireWaitCount   MetricConfig `mapstructure:"mongodb.lock.acquire.wait_count"`
	MongodbLockDeadlockCount      MetricConfig `mapstructure:"mongodb.lock.deadlock.count"`
	MongodbMemoryUsage            MetricConfig `mapstructure:"mongodb.memory.usage"`
	MongodbNetworkIoReceive       MetricConfig `mapstructure:"mongodb.network.io.receive"`
	MongodbNetworkIoTransmit      MetricConfig `mapstructure:"mongodb.network.io.transmit"`
	MongodbNetworkRequestCount    MetricConfig `mapstructure:"mongodb.network.request.count"`
	MongodbObjectCount            MetricConfig `mapstructure:"mongodb.object.count"`
	MongodbOperationCount         MetricConfig `mapstructure:"mongodb.operation.count"`
	MongodbOperationLatencyTime   MetricConfig `mapstructure:"mongodb.operation.latency.time"`
	MongodbOperationReplCount     MetricConfig `mapstructure:"mongodb.operation.repl.count"`
	MongodbOperationTime          MetricConfig `mapstructure:"mongodb.operation.time"`
	MongodbSessionCount           MetricConfig `mapstructure:"mongodb.session.count"`
	MongodbStorageSize            MetricConfig `mapstructure:"mongodb.storage.size"`
	MongodbUptime                 MetricConfig `mapstructure:"mongodb.uptime"`
}

func DefaultMetricsConfig() MetricsConfig {
	return MetricsConfig{
		MongodbCacheOperations: MetricConfig{
			Enabled: true,
		},
		MongodbCollectionCount: MetricConfig{
			Enabled: true,
		},
		MongodbConnectionCount: MetricConfig{
			Enabled: true,
		},
		MongodbCursorCount: MetricConfig{
			Enabled: true,
		},
		MongodbCursorTimeoutCount: MetricConfig{
			Enabled: true,
		},
		MongodbDataSize: MetricConfig{
			Enabled: true,
		},
		MongodbDatabaseCount: MetricConfig{
			Enabled: true,
		},
		MongodbDocumentOperationCount: MetricConfig{
			Enabled: true,
		},
		MongodbExtentCount: MetricConfig{
			Enabled: true,
		},
		MongodbGlobalLockTime: MetricConfig{
			Enabled: true,
		},
		MongodbHealth: MetricConfig{
			Enabled: false,
		},
		MongodbIndexAccessCount: MetricConfig{
			Enabled: true,
		},
		MongodbIndexCount: MetricConfig{
			Enabled: true,
		},
		MongodbIndexSize: MetricConfig{
			Enabled: true,
		},
		MongodbLockAcquireCount: MetricConfig{
			Enabled: false,
		},
		MongodbLockAcquireTime: MetricConfig{
			Enabled: false,
		},
		MongodbLockAcquireWaitCount: MetricConfig{
			Enabled: false,
		},
		MongodbLockDeadlockCount: MetricConfig{
			Enabled: false,
		},
		MongodbMemoryUsage: MetricConfig{
			Enabled: true,
		},
		MongodbNetworkIoReceive: MetricConfig{
			Enabled: true,
		},
		MongodbNetworkIoTransmit: MetricConfig{
			Enabled: true,
		},
		MongodbNetworkRequestCount: MetricConfig{
			Enabled: true,
		},
		MongodbObjectCount: MetricConfig{
			Enabled: true,
		},
		MongodbOperationCount: MetricConfig{
			Enabled: true,
		},
		MongodbOperationLatencyTime: MetricConfig{
			Enabled: false,
		},
		MongodbOperationReplCount: MetricConfig{
			Enabled: false,
		},
		MongodbOperationTime: MetricConfig{
			Enabled: true,
		},
		MongodbSessionCount: MetricConfig{
			Enabled: true,
		},
		MongodbStorageSize: MetricConfig{
			Enabled: true,
		},
		MongodbUptime: MetricConfig{
			Enabled: false,
		},
	}
}

// ResourceAttributeConfig provides common config for a particular resource attribute.
type ResourceAttributeConfig struct {
	Enabled bool `mapstructure:"enabled"`
	// Experimental: MetricsInclude defines a list of filters for attribute values.
	// If the list is not empty, only metrics with matching resource attribute values will be emitted.
	MetricsInclude []filter.Config `mapstructure:"metrics_include"`
	// Experimental: MetricsExclude defines a list of filters for attribute values.
	// If the list is not empty, metrics with matching resource attribute values will not be emitted.
	// MetricsInclude has higher priority than MetricsExclude.
	MetricsExclude []filter.Config `mapstructure:"metrics_exclude"`

	enabledSetByUser bool
}

func (rac *ResourceAttributeConfig) Unmarshal(parser *confmap.Conf) error {
	if parser == nil {
		return nil
	}
	err := parser.Unmarshal(rac)
	if err != nil {
		return err
	}
	rac.enabledSetByUser = parser.IsSet("enabled")
	return nil
}

// ResourceAttributesConfig provides config for mongodb resource attributes.
type ResourceAttributesConfig struct {
	Database      ResourceAttributeConfig `mapstructure:"database"`
	ServerAddress ResourceAttributeConfig `mapstructure:"server.address"`
	ServerPort    ResourceAttributeConfig `mapstructure:"server.port"`
}

func DefaultResourceAttributesConfig() ResourceAttributesConfig {
	return ResourceAttributesConfig{
		Database: ResourceAttributeConfig{
			Enabled: true,
		},
		ServerAddress: ResourceAttributeConfig{
			Enabled: true,
		},
		ServerPort: ResourceAttributeConfig{
			Enabled: false,
		},
	}
}

// MetricsBuilderConfig is a configuration for mongodb metrics builder.
type MetricsBuilderConfig struct {
	Metrics            MetricsConfig            `mapstructure:"metrics"`
	ResourceAttributes ResourceAttributesConfig `mapstructure:"resource_attributes"`
}

func DefaultMetricsBuilderConfig() MetricsBuilderConfig {
	return MetricsBuilderConfig{
		Metrics:            DefaultMetricsConfig(),
		ResourceAttributes: DefaultResourceAttributesConfig(),
	}
}
