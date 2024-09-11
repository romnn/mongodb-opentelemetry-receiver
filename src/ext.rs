pub trait NumDatapoints {
    fn num_datapoints(&self) -> Option<usize>;
}

impl NumDatapoints for opentelemetry_sdk::metrics::data::Metric {
    fn num_datapoints(&self) -> Option<usize> {
        if let Some(sum) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        {
            return Some(sum.data_points.len());
        }
        if let Some(sum) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<i64>>()
        {
            return Some(sum.data_points.len());
        }
        None
    }
}
