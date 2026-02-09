// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use super::{PerturbationSpec, TopologyLayout};

// Mainnet validator region indices (0-9 correspond to RTT_LATENCY_TABLE
// rows/cols). Extracted from current IOTA Mainnet validator list
// and checking IP geolocation.
// Distribution: 20x US-East, 4x US-West, 2x Canada, 21x EU-West,
// 13x EU-North, 8x AP-Southeast, 1x AP-South, 1x AP-Northeast
const MAINNET_NODE_REGIONS: [usize; 70] = [
    5, 9, 0, 8, 8, 3, 8, 3, 0, 5, 3, 5, 3, 3, 8, 5, 0, 1, 3, 1, 0, 5, 3, 5, 0, 5, 0, 0, 5, 5, 3, 5,
    3, 0, 3, 5, 3, 3, 3, 8, 0, 3, 3, 3, 2, 7, 0, 1, 0, 2, 0, 5, 0, 8, 3, 1, 0, 8, 0, 0, 3, 3, 8, 0,
    3, 0, 3, 5, 0, 0,
];

// RTT table for 10 AWS regions, in milliseconds.
// Based on actual AWS inter-region latency measurements from cloudping.co
// Enforces triangle inequality: d(i,j) <= d(i,k) + d(k,j) for all distinct
// i,j,k Regions: 0=us-east-1, 1=us-west-1, 2=ca-central-1, 3=eu-west-1,
//          4=eu-south-1, 5=eu-north-1, 6=sa-east-1, 7=ap-south-1,
//          8=ap-southeast-1, 9=ap-northeast-1
const RTT_LATENCY_TABLE: [[u16; 10]; 10] = [
    [0, 67, 15, 68, 101, 107, 111, 187, 215, 147], // us-east-1
    [67, 0, 79, 129, 161, 168, 170, 230, 173, 107], // us-west-1
    [15, 79, 0, 68, 101, 104, 123, 188, 223, 155], // ca-central-1
    [68, 129, 68, 0, 36, 39, 176, 121, 173, 203],  // eu-west-1
    [101, 161, 101, 36, 0, 31, 210, 111, 155, 218], // eu-south-1
    [107, 168, 104, 39, 31, 0, 215, 139, 179, 242], // eu-north-1
    [111, 170, 123, 176, 210, 215, 0, 294, 325, 257], // sa-east-1
    [187, 230, 188, 121, 111, 139, 294, 0, 61, 129], // ap-south-1
    [215, 173, 223, 173, 155, 179, 325, 61, 0, 68], // ap-southeast-1
    [147, 107, 155, 203, 218, 242, 257, 129, 68, 0], // ap-northeast-1
];

pub struct LatencyMatrixBuilder {
    number_of_instances: usize,
    max_latency: u16,
    topology_layout: TopologyLayout,
    perturbation_spec: PerturbationSpec,
    matrix: Vec<Vec<u16>>,
}

use rand::{Rng, thread_rng};

pub fn generate_block_matrix(n: usize, k: usize) -> Vec<Vec<bool>> {
    let k = k / 2;

    // Start with everything "true" (normal)
    let mut matrix = vec![vec![true; n]; n];

    // For symmetry, we assign blocks in round-robin pairs:
    // node i blocks nodes (i+1)%n, (i+2)%n, ..., (i+k)%n
    #[expect(clippy::needless_range_loop)]
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
            topology_layout: TopologyLayout::Mainnet,
            perturbation_spec: PerturbationSpec::None,
            matrix: vec![vec![0u16; number_of_instances]; number_of_instances],
        }
    }
    pub fn with_topology_layout(mut self, topology_layout: TopologyLayout) -> Self {
        if let TopologyLayout::RandomClustered { number_of_clusters } = topology_layout {
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
        let mut rng = thread_rng();
        let n = self.matrix.len();

        let positions: Vec<(f64, f64)> = (0..n)
            .map(|_| (rng.gen::<f64>(), rng.gen::<f64>()))
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
        #[expect(clippy::needless_range_loop)]
        for i in 0..self.number_of_instances {
            let ci = cluster_of(i);

            for j in 0..self.number_of_instances {
                let cj = cluster_of(j);

                matrix[i][j] = self.matrix[ci][cj];
            }
        }
        matrix
    }

    /// Build a mainnet topology matrix using validator region assignments
    /// and RTT_LATENCY_TABLE for inter-region latencies.
    fn fill_mainnet(&mut self) {
        let n = self.number_of_instances;

        for i in 0..n {
            let region_i = MAINNET_NODE_REGIONS[i % MAINNET_NODE_REGIONS.len()];
            for j in 0..n {
                let region_j = MAINNET_NODE_REGIONS[j % MAINNET_NODE_REGIONS.len()];
                // Use RTT value and convert to one-way latency
                self.matrix[i][j] = RTT_LATENCY_TABLE[region_i][region_j] / 2;
            }
        }
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
        #[expect(clippy::needless_range_loop)]
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
            TopologyLayout::HardCodedClustered => {
                self.matrix = RTT_LATENCY_TABLE
                    .map(|row| row.map(|x| x / 2))
                    .map(|row| row.to_vec())
                    .to_vec();
            }
            TopologyLayout::Mainnet => {
                self.fill_mainnet();
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
            .with_topology_layout(TopologyLayout::RandomGeographical)
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(500)
            .build();
        println!("{:?}", matrix);
    }
    #[test]
    #[ignore]
    fn test_latency_clustered() {
        let matrix = LatencyMatrixBuilder::new(12)
            .with_topology_layout(TopologyLayout::RandomClustered {
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
            .with_topology_layout(TopologyLayout::RandomGeographical)
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
            .with_topology_layout(TopologyLayout::RandomClustered {
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

    #[test]
    #[ignore]
    fn test_mainnet_10_instances() {
        let matrix = LatencyMatrixBuilder::new(10)
            .with_topology_layout(TopologyLayout::Mainnet)
            .with_perturbation_spec(PerturbationSpec::None)
            .with_max_latency(300)
            .build();
        println!("{:?}", matrix);
    }

    #[test]
    fn test_mainnet_region_distribution() {
        let expected = [
            (0, 20),
            (1, 4),
            (2, 2),
            (3, 21),
            (5, 13),
            (7, 1),
            (8, 8),
            (9, 1),
        ];
        let mut counts = [0usize; 10];
        for &region in &MAINNET_NODE_REGIONS {
            counts[region] += 1;
        }
        for &(region, expected_count) in &expected {
            assert_eq!(
                counts[region], expected_count,
                "Region {}: expected {}, got {}",
                region, expected_count, counts[region]
            );
        }
    }
}
