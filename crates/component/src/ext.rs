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

pub trait MetricAttributes {
    fn attributes(&self) -> Box<dyn Iterator<Item = &Vec<opentelemetry::KeyValue>> + '_>;
}

impl MetricAttributes for opentelemetry_sdk::metrics::data::Metric {
    fn attributes(&self) -> Box<dyn Iterator<Item = &Vec<opentelemetry::KeyValue>> + '_> {
        if let Some(sum) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Sum<i64>>()
        {
            return Box::new(sum.data_points.iter().map(|d| &d.attributes));
        }
        if let Some(gauge) = self
            .data
            .as_any()
            .downcast_ref::<opentelemetry_sdk::metrics::data::Gauge<i64>>()
        {
            return Box::new(gauge.data_points.iter().map(|d| &d.attributes));
        }
        Box::new(std::iter::empty())
    }
}
