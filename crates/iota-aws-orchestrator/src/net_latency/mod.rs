// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{client::Instance, net_latency::latency_matrix_builder::LatencyMatrixBuilder};

pub mod latency_matrix_builder;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum TopologyLayout {
    /// All nodes are randomly distributed with their own latencies, no clusters
    RandomGeographical,
    /// Nodes are randomly distributed in number_of_clusters clusters
    RandomClustered { number_of_clusters: usize },
    /// Uses a hardcoded 10x10 matrix with 10 equal-sized regions
    HardCodedClustered,
    /// Use mainnet validator region distribution with shuffled order
    Mainnet,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum PerturbationSpec {
    /// No Perturbation introduced
    None,
    /// Broken Triangle introduced for number_of_triangles Triangles of nodes
    /// latency(A,B) + latency(B,C) + added_latency =  Latency(A.C)
    BrokenTriangle {
        number_of_triangles: u16,
        added_latency: u16,
    },
    /// Blocking connections
    Blocking {
        number_of_blocked_connections: usize,
    },
}

pub struct NetworkLatencyCommandBuilder<'a> {
    instances: &'a [Instance],
    topology_layout: TopologyLayout,
    perturbation_spec: PerturbationSpec,
    max_latency: u16,
}

pub fn latency_command(latency_vector: &Vec<(&Instance, u16)>) -> String {
    // Clean existing rules
    let mut cmd = "export IFACE=$(ip -j route get 172 | jq -r '.[0].dev') && ".to_string();
    cmd.push_str("sudo tc qdisc del dev $IFACE root 2>/dev/null || true && ");
    cmd.push_str("sudo tc qdisc add dev $IFACE root handle 1: htb default 1 && ");
    // Root prio qdisc
    cmd.push_str("sudo tc class add dev $IFACE parent 1: classid 1:1 htb rate 1gbit && ");

    // Add one netem band per IP
    for (i, (instance, latency)) in latency_vector.iter().enumerate() {
        let ip = instance.private_ip;
        let handle = i + 10; // avoid conflict with default bands
        cmd.push_str(&format!(
            "sudo tc class add dev $IFACE parent 1:1 classid 1:{handle} htb rate 1gbit && "
        ));
        cmd.push_str(&format!(
            "sudo tc qdisc add dev $IFACE parent 1:{handle} handle {handle}: netem delay {latency}ms && "
        ));
        // Add filters that map MARKs to the prio bands
        cmd.push_str(&format!(
            "sudo tc filter add dev $IFACE protocol ip parent 1: prio 1 u32 match ip dst {ip}/32 flowid 1:{handle} && "
        ));
    }

    // Remove trailing " && "
    cmd.trim_end_matches(" && ").to_string()
}
impl<'a> NetworkLatencyCommandBuilder<'a> {
    pub fn new(instances: &'a [Instance]) -> Self {
        Self {
            instances,
            topology_layout: TopologyLayout::RandomGeographical,
            perturbation_spec: PerturbationSpec::None,
            max_latency: 500,
        }
    }

    pub fn with_topology_layout(mut self, topology_layout: TopologyLayout) -> Self {
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

    pub fn build_network_latency_matrix(self) -> Vec<(Instance, String)> {
        let latency_matrix = LatencyMatrixBuilder::new(self.instances.len())
            .with_max_latency(self.max_latency)
            .with_topology_layout(self.topology_layout)
            .with_perturbation_spec(self.perturbation_spec)
            .build();
        // print out the latency matrix
        println!("\n\n{:?}\n\n", latency_matrix);
        let mut instance2instance_latency_map: HashMap<&Instance, Vec<(&Instance, u16)>> =
            HashMap::new();
        for (i, instance) in self.instances.iter().enumerate() {
            let entry = instance2instance_latency_map.entry(instance).or_default();
            #[expect(clippy::needless_range_loop)]
            for j in 0..self.instances.len() {
                if latency_matrix[i][j] == 0 {
                    // no need to generate latency commands where latency is 0
                    // same cluster or same node
                    continue;
                }
                entry.push((&self.instances[j], latency_matrix[i][j]));
            }
        }
        instance2instance_latency_map
            .iter()
            .map(|(instance, vector)| ((*instance).clone(), latency_command(vector)))
            .collect::<Vec<_>>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore]
    fn test_build_network_latency_matrix() {
        let mut instances = Vec::new();
        let n = 12;

        for i in 0..n {
            instances.push(Instance::new_for_test(i.to_string()));
        }

        let latency_network_command = NetworkLatencyCommandBuilder::new(&instances)
            .with_perturbation_spec(PerturbationSpec::BrokenTriangle {
                added_latency: 50,
                number_of_triangles: 2,
            })
            .with_topology_layout(TopologyLayout::RandomGeographical)
            .build_network_latency_matrix();
        println!(
            "{:?}",
            latency_network_command
                .iter()
                .map(|(x, t)| (x.id.clone(), t.clone()))
                .collect::<Vec<_>>()
        );
    }
}
