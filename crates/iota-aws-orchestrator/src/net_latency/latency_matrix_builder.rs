use super::{PerturbationSpec, TopologyLayout};
struct LatencyMatrixBuilder {
    number_of_instances: usize,
    max_latency: u16,
    min_latency: u16,
    topology_layout: TopologyLayout,
    perturbation_spec: PerturbationSpec,
    matrix: Vec<Vec<u16>>,
}

impl LatencyMatrixBuilder {
    fn new(number_of_instances: usize) -> Self {
        Self {
            number_of_instances,
            max_latency: 750,
            min_latency: 1,
            topology_layout: TopologyLayout::Geographical,
            perturbation_spec: PerturbationSpec::None,
            matrix: vec![vec![0u16; number_of_instances]; number_of_instances],
        }
    }
    fn with_topology_layout(mut self, topology_layout: TopologyLayout) -> Self {
        if let TopologyLayout::Clustered { number_of_clusters } = topology_layout {
            self.matrix = vec![vec![0u16; number_of_clusters]; number_of_clusters];
        }
        self.topology_layout = topology_layout;
        self
    }

    fn with_perturbation_spec(mut self, perturbation_spec: PerturbationSpec) -> Self {
        self.perturbation_spec = perturbation_spec;
        self
    }

    fn with_max_latency(mut self, max_latency: u16) -> Self {
        self.max_latency = max_latency;
        self
    }

    fn with_min_latency(mut self, min_latency: u16) -> Self {
        self.min_latency = min_latency;
        self
    }

    fn fill_geographical(&mut self) {
        let n = self.matrix.len();
        // Maximum possible "distance" between indices: |0 - (n-1)| = n - 1
        let max_dist = (n - 1) as f32;

        // span of allowed latencies
        let span = (self.max_latency - self.min_latency) as f32;

        // K is the latency added per 1 position of distance
        // Ensures: |0 - (n-1)| * K + min_latency = max_latency
        let k = span / max_dist;

        for i in 0..n {
            for j in 0..n {
                let dist = (i as isize - j as isize).unsigned_abs() as f32;
                let latency = self.min_latency as f32 + dist * k;

                // round to nearest integer
                self.matrix[i][j] = latency.round() as u16;
            }
        }
    }

    /// Map nodes into clusters and expand a C×C cluster matrix into an N×N node
    /// matrix.
    fn expand_clusters_to_nodes_matrix(&self) -> Vec<Vec<u16>> {
        let number_of_clusters = self.matrix.len();
        let mut matrix = vec![vec![0u16; self.number_of_instances]; self.number_of_instances];

        let c = number_of_clusters.max(1).min(self.number_of_instances);

        // Same mapping as before: spread nodes as evenly as possible over clusters.
        let cluster_of = |idx: usize| -> usize {
            idx * c / self.number_of_instances // 0..n-1 -> 0..c-1
        };
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.number_of_instances {
            let ci = cluster_of(i);

            for j in 0..self.number_of_instances {
                let cj = cluster_of(j);

                matrix[i][j] = self.matrix[ci][cj];
            }
        }
        matrix
    }

    /// Apply "broken triangle" to up to `k` triangles of the form (i, i+1,
    /// i+2). Ensures: latency(A,B) + latency(B,C) < latency(A,C)
    /// by bumping A<->C.
    fn apply_broken_triangle(&mut self, number_of_triangles: u16, added_latency: u16) {
        if self.matrix.len() < 3 {
            return;
        }

        let max_tris = self.matrix.len() - 2;
        let count = (number_of_triangles as usize).min(max_tris);

        for start in 0..count {
            let a = start;
            let b = start + 1;
            let c = start + 2;

            let ab = self.matrix[a][b];
            let bc = self.matrix[b][c];

            // direct A<->C should be slower than going through B
            let new_ac = ab
                .saturating_add(bc)
                .saturating_add(added_latency)
                .min(self.max_latency + added_latency);

            self.matrix[a][c] = new_ac;
            self.matrix[c][a] = new_ac;
        }
    }

    fn build(mut self) -> Vec<Vec<u16>> {
        self.fill_geographical();
        match self.perturbation_spec {
            PerturbationSpec::BrokenTriangle {
                number_of_triangles,
                added_latency,
            } => {
                self.apply_broken_triangle(number_of_triangles, added_latency);
            }
            PerturbationSpec::None => {}
        };
        self.expand_clusters_to_nodes_matrix()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_matrix() {
        let matrix = LatencyMatrixBuilder::new(4)
            .with_topology_layout(TopologyLayout::Geographical)
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(500)
            .with_min_latency(100)
            .build();

        assert_eq!(
            matrix,
            [
                [100, 233, 367, 500],
                [233, 100, 233, 367],
                [367, 233, 100, 233],
                [500, 367, 233, 100]
            ]
        );
    }
    #[test]
    fn test_latency_clustered() {
        let matrix = LatencyMatrixBuilder::new(12)
            .with_topology_layout(TopologyLayout::Clustered {
                number_of_clusters: 4,
            })
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(500)
            .with_min_latency(100)
            .build();

        assert_eq!(
            matrix,
            [
                [100, 100, 100, 233, 233, 233, 367, 367, 367, 500, 500, 500],
                [100, 100, 100, 233, 233, 233, 367, 367, 367, 500, 500, 500],
                [100, 100, 100, 233, 233, 233, 367, 367, 367, 500, 500, 500],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 367, 367, 367],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 367, 367, 367],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 367, 367, 367],
                [367, 367, 367, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [367, 367, 367, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [367, 367, 367, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [500, 500, 500, 367, 367, 367, 233, 233, 233, 100, 100, 100],
                [500, 500, 500, 367, 367, 367, 233, 233, 233, 100, 100, 100],
                [500, 500, 500, 367, 367, 367, 233, 233, 233, 100, 100, 100]
            ]
        );
    }

    #[test]
    fn test_apply_broken_triangle() {
        let matrix = LatencyMatrixBuilder::new(4)
            .with_topology_layout(TopologyLayout::Geographical)
            .with_perturbation_spec(PerturbationSpec::BrokenTriangle {
                number_of_triangles: 2,
                added_latency: 100,
            })
            .with_max_latency(500)
            .with_min_latency(100)
            .build();

        assert_eq!(
            matrix,
            [
                [100, 233, 566, 500],
                [233, 100, 233, 566],
                [566, 233, 100, 233],
                [500, 566, 233, 100]
            ]
        );
    }

    #[test]
    fn test_clustered_broken_triangle() {
        let matrix = LatencyMatrixBuilder::new(12)
            .with_topology_layout(TopologyLayout::Clustered {
                number_of_clusters: 4,
            })
            .with_perturbation_spec(PerturbationSpec::BrokenTriangle {
                number_of_triangles: 2,
                added_latency: 100,
            })
            .with_max_latency(500)
            .with_min_latency(100)
            .build();
        assert_eq!(
            matrix,
            [
                [100, 100, 100, 233, 233, 233, 566, 566, 566, 500, 500, 500],
                [100, 100, 100, 233, 233, 233, 566, 566, 566, 500, 500, 500],
                [100, 100, 100, 233, 233, 233, 566, 566, 566, 500, 500, 500],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 566, 566, 566],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 566, 566, 566],
                [233, 233, 233, 100, 100, 100, 233, 233, 233, 566, 566, 566],
                [566, 566, 566, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [566, 566, 566, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [566, 566, 566, 233, 233, 233, 100, 100, 100, 233, 233, 233],
                [500, 500, 500, 566, 566, 566, 233, 233, 233, 100, 100, 100],
                [500, 500, 500, 566, 566, 566, 233, 233, 233, 100, 100, 100],
                [500, 500, 500, 566, 566, 566, 233, 233, 233, 100, 100, 100]
            ]
        )
    }
}
