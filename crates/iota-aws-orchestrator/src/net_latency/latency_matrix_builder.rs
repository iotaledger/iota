// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::{PerturbationSpec, TopologyLayout};
pub struct LatencyMatrixBuilder {
    number_of_instances: usize,
    max_latency: u16,
    topology_layout: TopologyLayout,
    perturbation_spec: PerturbationSpec,
    matrix: Vec<Vec<u16>>,
}

use rand::{Rng, rng};

impl LatencyMatrixBuilder {
    pub fn new(number_of_instances: usize) -> Self {
        Self {
            number_of_instances,
            max_latency: 300,
            topology_layout: TopologyLayout::Geographical,
            perturbation_spec: PerturbationSpec::None,
            matrix: vec![vec![0u16; number_of_instances]; number_of_instances],
        }
    }
    pub fn with_topology_layout(mut self, topology_layout: TopologyLayout) -> Self {
        if let TopologyLayout::Clustered { number_of_clusters } = topology_layout {
            self.matrix = vec![vec![0u16; number_of_clusters]; number_of_clusters];
        }
        self.topology_layout = topology_layout;
        self
    }

    pub fn with_perturbation_spec(mut self, perturbation_spec: PerturbationSpec) -> Self {
        self.perturbation_spec = perturbation_spec;
        self
    }

    pub fn with_max_latency(mut self, max_latency: u16) -> Self {
        self.max_latency = max_latency;
        self
    }

    fn cylinder_distance(&self, a: (f64, f64), b: (f64, f64)) -> u16 {
        // wrap around for X
        let mut dx = (a.0 - b.0).abs();
        if dx > 0.5 {
            dx = 1.0 - dx;
        }
        // do not wrap for Y ( no cables going over poles)
        let dy = (a.1 - b.1).abs();

        ((dx * dx + dy * dy).sqrt() * 0.447 * self.max_latency as f64) as u16
    }

    fn fill_geographical(&mut self) {
        let mut rng = rng();
        let n = self.matrix.len();

        let positions: Vec<(f64, f64)> = (0..n)
            .map(|_| (rng.random::<f64>(), rng.random::<f64>()))
            .collect();

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    self.matrix[i][j] = 0;
                    continue;
                }
                self.matrix[i][j] = self.cylinder_distance(positions[i], positions[j]);
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
    /// i+2). Ensures: latency(A,B) + latency(B,C) + added_latency =
    /// latency(A,C)
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

    pub fn build(mut self) -> Vec<Vec<u16>> {
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
    #[ignore]
    fn test_latency_matrix() {
        let matrix = LatencyMatrixBuilder::new(4)
            .with_topology_layout(TopologyLayout::Geographical)
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(500)
            .build();
        println!("{:?}", matrix);
    }
    #[test]
    #[ignore]
    fn test_latency_clustered() {
        let matrix = LatencyMatrixBuilder::new(12)
            .with_topology_layout(TopologyLayout::Clustered {
                number_of_clusters: 4,
            })
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(500)
            .build();
        println!("{:?}", matrix);
    }

    #[test]
    #[ignore]
    fn test_apply_broken_triangle() {
        let matrix = LatencyMatrixBuilder::new(4)
            .with_topology_layout(TopologyLayout::Geographical)
            .with_perturbation_spec(PerturbationSpec::BrokenTriangle {
                number_of_triangles: 2,
                added_latency: 100,
            })
            .with_max_latency(500)
            .build();
        println!("{:?}", matrix);
    }

    #[test]
    #[ignore]
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
            .build();
        println!("{:?}", matrix);
    }
}
