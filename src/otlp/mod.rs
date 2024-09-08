use color_eyre::eyre;
use opentelemetry_otlp::{Compression, MetricsExporterBuilder, Protocol, WithExportConfig};
use opentelemetry_sdk::metrics::data::Metric;
use opentelemetry_sdk::resource::Resource;
use opentelemetry_sdk::{
    metrics::{
        data::{ResourceMetrics, ScopeMetrics},
        exporter::PushMetricsExporter,
        reader::{DefaultAggregationSelector, DefaultTemporalitySelector},
    },
    Scope,
};
use std::collections::HashMap;

#[derive(Debug)]
pub struct MetricsExporter {}

// // State defines an ownership state of pmetric.Metrics, plog.Logs or ptrace.Traces.
// type State int32
//
// const (
// 	// StateMutable indicates that the data is exclusive to the current consumer.
// 	StateMutable State = iota
//
// 	// StateReadOnly indicates that the data is shared with other consumers.
// 	StateReadOnly
// )
//
// // AssertMutable panics if the state is not StateMutable.
// func (state *State) AssertMutable() {
// 	if *state != StateMutable {
// 		panic("invalid access to shared data")
// 	}
// }

async fn test() -> eyre::Result<()> {
    let metrics_exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_compression(Compression::Gzip)
        .with_protocol(Protocol::Grpc)
        .build_metrics_exporter(
            Box::new(DefaultAggregationSelector::new()),
            Box::new(DefaultTemporalitySelector::new()),
        )?;
    let library = opentelemetry_sdk::InstrumentationLibrary::builder("my-crate")
        .with_version(env!("CARGO_PKG_VERSION"))
        .with_schema_url("https://opentelemetry.io/schemas/1.17.0")
        .build();
    // let metrics = ResourceMetrics {
    //     resource: Resource::new(HashMap::new()),
    //     scope_metrics: vec![ScopeMetrics {
    //         scope: library,
    //         metrics: vec![Metric {
    //         }],
    //     }],
    // };
    // metrics_exporter.export(&mut metrics).await?;

    // let test = MetricsExporterBuilder::Tonic()::build_metrics_exporter((), temporality_selector, aggregation_selector)
    //    if e.clientConn, err = e.config.ClientConfig.ToClientConn(ctx, host, e.settings, grpc.WithUserAgent(e.userAgent)); err != nil {
    // 	return err
    // }
    // e.traceExporter = ptraceotlp.NewGRPCClient(e.clientConn)
    // e.metricExporter = pmetricotlp.NewGRPCClient(e.clientConn)
    // e.logExporter = plogotlp.NewGRPCClient(e.clientConn)
    // headers := map[string]string{}
    // for k, v := range e.config.ClientConfig.Headers {
    // 	headers[k] = string(v)
    // }
    // e.metadata = metadata.New(headers)
    // e.callOptions = []grpc.CallOption{
    // 	grpc.WaitForReady(e.config.ClientConfig.WaitForReady),
    // }
    //
    // return
    Ok(())
}

// obsReport, err := newExporter(obsReportSettings{exporterID: set.ID, exporterCreateSettings: set, dataType: signal})
// 	if err != nil {
// 		return nil, err
// 	}
//
// 	be := &baseExporter{
// 		signal: signal,
//
// 		batchSender:   &baseRequestSender{},
// 		queueSender:   &baseRequestSender{},
// 		obsrepSender:  osf(obsReport),
// 		retrySender:   &baseRequestSender{},
// 		timeoutSender: &timeoutSender{cfg: NewDefaultTimeoutSettings()},
//
// 		set:    set,
// 		obsrep: obsReport,
// 	}
//
// 	for _, op := range options {
// 		err = multierr.Append(err, op(be))
// 	}
// 	if err != nil {
// 		return nil, err
// 	}
//
// 	if be.batcherCfg.Enabled {
// 		bs := newBatchSender(be.batcherCfg, be.set, be.batchMergeFunc, be.batchMergeSplitfunc)
// 		for _, opt := range be.batcherOpts {
// 			err = multierr.Append(err, opt(bs))
// 		}
// 		if bs.mergeFunc == nil || bs.mergeSplitFunc == nil {
// 			err = multierr.Append(err, fmt.Errorf("WithRequestBatchFuncs must be provided for the batcher applied to the request-based exporters"))
// 		}
// 		be.batchSender = bs
// 	}
//
// 	if be.queueCfg.Enabled {
// 		set := exporterqueue.Settings{
// 			DataType:         be.signal,
// 			ExporterSettings: be.set,
// 		}
// 		be.queueSender = newQueueSender(be.queueFactory(context.Background(), set, be.queueCfg), be.set, be.queueCfg.NumConsumers, be.exportFailureMessage, be.obsrep)
// 		for _, op := range options {
// 			err = multierr.Append(err, op(be))
// 		}
// 	}
//
// 	if err != nil {
// 		return nil, err
// 	}
//
// 	be.connectSenders()
//
// 	if bs, ok := be.batchSender.(*batchSender); ok {
// 		// If queue sender is enabled assign to the batch sender the same number of workers.
// 		if qs, ok := be.queueSender.(*queueSender); ok {
// 			bs.concurrencyLimit = int64(qs.numConsumers)
// 		}
// 		// Batcher sender mutates the data.
// 		be.consumerOptions = append(be.consumerOptions, consumer.WithCapabilities(consumer.Capabilities{MutatesData: true}))
// 	}
//
// 	return be, nil

// // send sends the request using the first sender in the chain.
// func (be *baseExporter) send(ctx context.Context, req Request) error {
// 	err := be.queueSender.send(ctx, req)
// 	if err != nil {
// 		be.set.Logger.Error("Exporting failed. Rejecting data."+be.exportFailureMessage,
// 			zap.Error(err), zap.Int("rejected_items", req.ItemsCount()))
// 	}
// 	return err
// }
//
// // connectSenders connects the senders in the predefined order.
// func (be *baseExporter) connectSenders() {
// 	be.queueSender.setNextSender(be.batchSender)
// 	be.batchSender.setNextSender(be.obsrepSender)
// 	be.obsrepSender.setNextSender(be.retrySender)
// 	be.retrySender.setNextSender(be.timeoutSender)
// }
//
// func (be *baseExporter) Start(ctx context.Context, host component.Host) error {
// 	// First start the wrapped exporter.
// 	if err := be.StartFunc.Start(ctx, host); err != nil {
// 		return err
// 	}
//
// 	// If no error then start the batchSender.
// 	if err := be.batchSender.Start(ctx, host); err != nil {
// 		return err
// 	}
//
// 	// Last start the queueSender.
// 	return be.queueSender.Start(ctx, host)
// }
//
// func (be *baseExporter) Shutdown(ctx context.Context) error {
// 	return multierr.Combine(
// 		// First shutdown the retry sender, so the queue sender can flush the queue without retries.
// 		be.retrySender.Shutdown(ctx),
// 		// Then shutdown the batch sender
// 		be.batchSender.Shutdown(ctx),
// 		// Then shutdown the queue sender.
// 		be.queueSender.Shutdown(ctx),
// 		// Last shutdown the wrapped exporter itself.
// 		be.ShutdownFunc.Shutdown(ctx))
// }
