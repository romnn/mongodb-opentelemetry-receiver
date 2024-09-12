use std::collections::HashSet;

#[derive(Debug)]
pub struct Bfs<'a, N, E, D, Ix>
where
    D: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    visited: HashSet<Ix>,
    // visited: HashSet<(EdgeIndex, NodeIndex)>,
    stack: Vec<Ix>,
    // stack: Vec<(EdgeIndex, NodeIndex)>,
    // stack: Vec<(EdgeIndex, NodeIndex)>,
    // path: Path,
    graph: &'a petgraph::Graph<N, E, D, Ix>,
}

impl<'a, N, E, D, Ix> Bfs<'a, N, E, D, Ix>
where
    D: petgraph::EdgeType,
    Ix: petgraph::graph::IndexType,
{
    #[must_use]
    pub fn new(graph: &'a petgraph::Graph<N, E, D, Ix>) -> Self {
        // let mut dominator_stack = Vec::new();
        let mut stack = Vec::new();
        //
        // if let WarpNode::Branch { .. } = graph[root_node_idx] {
        //     dominator_stack.push(root_node_idx);
        // }
        //
        // for (outgoing_edge_idx, next_node_idx) in graph.outgoing_neigbors(root_node_idx) {
        //     stack.push((outgoing_edge_idx, next_node_idx));
        // }

        Self {
            graph,
            // dominator_stack,
            visited: HashSet::new(),
            // path: Path::new(),
            stack,
        }
    }
}
