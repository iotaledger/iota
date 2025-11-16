pub mod latency_matrix_builder;

pub enum TopologyLayout {
    /// All Nodes are distributed with their own latencies, no clusters
    Geographical,
    /// Nodes are distributed in number_of_clusters clusters
    Clustered { number_of_clusters: usize },
}

pub enum PerturbationSpec {
    /// No Perturbation introduced
    None,
    /// Broken Triangle introduced for number_of_triangles Triangles of nodes
    /// latency(A,B) + latency(B,C) + added_latency =  Latency(A.C)
    BrokenTriangle {
        number_of_triangles: u16,
        added_latency: u16,
    },
}
