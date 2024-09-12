use crate::pipeline::Processor;

pub trait GraphExt<N, E, Ix> {
    // fn add_unique_edge(&mut self, a: NodeIndex<Ix>, b: NodeIndex<Ix>, weight: E) -> EdgeIndex<Ix>
    // where
    //     E: PartialEq;
    //
    // fn find_node<W>(&self, weight: &W) -> Option<NodeIndex<Ix>>
    // where
    //     W: PartialEq<N>;

    fn get_or_insert_node(&mut self, weight: N) -> petgraph::graph::NodeIndex<Ix>
    where
        N: PartialEq;
}

impl<N, E, D, Ix> GraphExt<N, E, Ix> for petgraph::Graph<N, E, D, Ix>
where
    D: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    fn get_or_insert_node(&mut self, weight: N) -> petgraph::graph::NodeIndex<Ix>
    where
        N: PartialEq,
    {
        let found = self
            .node_indices()
            .find(|idx| match self.node_weight(*idx) {
                Some(node) => weight == *node,
                None => false,
            });
        match found {
            Some(found) => found,
            None => petgraph::Graph::<N, E, D, Ix>::add_node(self, weight),
        }
    }
}

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
