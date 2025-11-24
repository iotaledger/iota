// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::{PerturbationSpec, TopologyLayout};

// RTT table for 10 AWS regions, in milliseconds.
// Strict triangle inequality: d(i,j) + 1 <= d(i,k) + d(k,j) for all distinct
// i,j,k.
const RTT_LATENCY_TABLE: [[u16; 10]; 10] = [
    [1, 14, 96, 112, 198, 65, 68, 105, 192, 146],
    [14, 1, 95, 122, 196, 78, 67, 103, 189, 142],
    [96, 95, 1, 204, 281, 155, 29, 50, 143, 227],
    [112, 122, 204, 1, 309, 175, 176, 213, 299, 254],
    [198, 196, 281, 309, 1, 137, 254, 268, 150, 101],
    [65, 78, 155, 175, 137, 1, 127, 164, 226, 108],
    [68, 67, 29, 176, 254, 127, 1, 38, 125, 199],
    [105, 103, 50, 213, 268, 164, 38, 1, 148, 236],
    [192, 189, 143, 299, 150, 226, 125, 148, 1, 140],
    [146, 142, 227, 254, 101, 108, 199, 236, 140, 1],
];

pub struct LatencyMatrixBuilder {
    number_of_instances: usize,
    max_latency: u16,
    topology_layout: TopologyLayout,
    perturbation_spec: PerturbationSpec,
    matrix: Vec<Vec<u16>>,
}

use rand::{Rng, rng};

pub fn generate_block_matrix(n: usize, k: usize) -> Vec<Vec<bool>> {
    let k = k / 2;

    // Start with everything "true" (normal)
    let mut matrix = vec![vec![true; n]; n];

    // For symmetry, we assign blocks in round-robin pairs:
    // node i blocks nodes (i+1)%n, (i+2)%n, ..., (i+k)%n
    for i in 0..n {
        for offset in 1..=k {
            let j = (i + offset) % n;
            matrix[i][j] = false;
            matrix[j][i] = false; // enforce symmetry
        }
    }

    matrix
}
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

    fn apply_blocking(&mut self, number_of_blocking_connections: usize) {
        if self.matrix.len() < 3 {
            return;
        }
        if number_of_blocking_connections > self.matrix.len() / 2 {
            return;
        }
        let n = self.matrix.len();
        let blocking_matrix =
            generate_block_matrix(self.matrix.len(), number_of_blocking_connections);
        #[allow(clippy::needless_range_loop)]
        for i in 0..n {
            for j in 0..n {
                if !blocking_matrix[i][j] {
                    // add 10s to latency of blocked connections.
                    self.matrix[i][j] = self.matrix[i][j].saturating_add(10000);
                }
            }
        }
    }

    pub fn build(mut self) -> Vec<Vec<u16>> {
        match self.topology_layout {
            TopologyLayout::HardCoded => {
                self.matrix = RTT_LATENCY_TABLE
                    .map(|row| row.map(|x| x / 2))
                    .map(|row| row.to_vec())
                    .to_vec();
            }
            _ => self.fill_geographical(),
        };
        match self.perturbation_spec {
            PerturbationSpec::BrokenTriangle {
                number_of_triangles,
                added_latency,
            } => {
                self.apply_broken_triangle(number_of_triangles, added_latency);
            }
            PerturbationSpec::Blocking {
                number_of_blocked_connections,
            } => {
                self.apply_blocking(number_of_blocked_connections);
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
