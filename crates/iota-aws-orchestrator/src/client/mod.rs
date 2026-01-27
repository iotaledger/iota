// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::Display,
    net::{Ipv4Addr, SocketAddr},
};

use serde::{Deserialize, Serialize};

use super::error::CloudProviderResult;

pub mod aws;

#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Hash)]
pub enum InstanceRole {
    Node,
    Client,
    Metrics,
}

impl Display for InstanceRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<&str> for InstanceRole {
    fn from(role: &str) -> Self {
        match role {
            "Node" => InstanceRole::Node,
            "Client" => InstanceRole::Client,
            "Metrics" => InstanceRole::Metrics,
            _ => unreachable!(),
        }
    }
}

#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Hash)]
pub enum InstanceLifecycle {
    Spot,
    OnDemand,
}

impl Display for InstanceLifecycle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Represents a cloud provider instance.
#[derive(Debug, Deserialize, Clone, Eq, PartialEq, Hash)]
pub struct Instance {
    /// The unique identifier of the instance.
    pub id: String,
    /// The region where the instance runs.
    pub region: String,
    /// The public ip address of the instance (accessible from anywhere).
    pub main_ip: Ipv4Addr,
    /// The public ip address of the instance (accessible from the same VPC).
    pub private_ip: Ipv4Addr,
    /// The list of tags associated with the instance.
    pub tags: Vec<String>,
    /// The specs of the instance.
    pub specs: String,
    /// The current status of the instance.
    pub status: String,
    // The role of the instance. "Node" | "Client" | "Metrics"
    pub role: InstanceRole,
    // The lifecycle of the instance. "Spot" | "OnDemand"
    pub lifecycle: InstanceLifecycle,
}

impl Display for Instance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.role, self.main_ip)
    }
}

impl Instance {
    /// Return whether the instance is active and running.
    pub fn is_active(&self) -> bool {
        self.status.to_lowercase() == "running"
    }

    /// Return whether the instance is inactive and not ready for use.
    pub fn is_inactive(&self) -> bool {
        !self.is_active()
    }

    // Return whether the instance is able to be started
    pub fn is_stopped(&self) -> bool {
        self.status.to_lowercase() == "stopped"
    }

    /// Return whether the instance is terminated and in the process of being
    /// deleted.
    pub fn is_terminated(&self) -> bool {
        self.status.to_lowercase() == "terminated"
    }

    /// Return the ssh address to connect to the instance.
    pub fn ssh_address(&self) -> SocketAddr {
        format!("{}:22", self.main_ip).parse().unwrap()
    }

    #[cfg(test)]
    pub fn new_for_test(id: String) -> Self {
        Self {
            id,
            region: Default::default(),
            main_ip: Ipv4Addr::LOCALHOST,
            private_ip: Ipv4Addr::LOCALHOST,
            tags: Default::default(),
            specs: Default::default(),
            status: Default::default(),
            role: InstanceRole::Node,
            lifecycle: InstanceLifecycle::OnDemand,
        }
    }
}

#[async_trait::async_trait]
pub trait ServerProviderClient: Display {
    /// The username used to connect to the instances.
    const USERNAME: &'static str;

    /// List all existing instances (regardless of their status) filtered by
    /// role.
    async fn list_instances_by_role(
        &self,
        role: InstanceRole,
    ) -> CloudProviderResult<Vec<Instance>>;

    async fn list_instances_by_region_and_ids(
        &self,
        ids_by_region: &HashMap<String, Vec<String>>,
    ) -> CloudProviderResult<Vec<Instance>>;

    /// Start the specified instances.
    async fn start_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send;

    /// Halt/Stop the specified instances. We may still be billed for stopped
    /// instances.
    async fn stop_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send;

    /// Create an instance in a specific region.
    async fn create_instance<S>(
        &self,
        region: S,
        role: InstanceRole,
        quantity: usize,
        use_spot_instances: bool,
        id: String,
    ) -> CloudProviderResult<Vec<Instance>>
    where
        S: Into<String> + Serialize + Send;

    /// Delete a specific instance. Calling this function ensures we are no
    /// longer billed for the specified instance.
    async fn delete_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send;

    /// Authorize the provided ssh public key to access machines.
    async fn register_ssh_public_key(&self, public_key: String) -> CloudProviderResult<()>;

    /// Return provider-specific commands to setup the instance.
    async fn instance_setup_commands(&self) -> CloudProviderResult<Vec<String>>;

    #[cfg(test)]
    fn instances(&self) -> Vec<Instance>;
}

#[cfg(test)]
pub mod test_client {
    use std::{collections::HashMap, fmt::Display, sync::Mutex};

    use serde::Serialize;

    use super::{Instance, InstanceLifecycle, InstanceRole, ServerProviderClient};
    use crate::{error::CloudProviderResult, settings::Settings};

    pub struct TestClient {
        settings: Settings,
        instances: Mutex<Vec<Instance>>,
    }

    impl TestClient {
        pub fn new(settings: Settings) -> Self {
            Self {
                settings,
                instances: Mutex::new(Vec::new()),
            }
        }
    }

    impl Display for TestClient {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "TestClient")
        }
    }

    #[async_trait::async_trait]
    impl ServerProviderClient for TestClient {
        const USERNAME: &'static str = "root";

        async fn list_instances_by_role(
            &self,
            _role: InstanceRole,
        ) -> CloudProviderResult<Vec<Instance>> {
            let guard = self.instances.lock().unwrap();
            Ok(guard.clone())
        }
        async fn list_instances_by_region_and_ids(
            &self,
            ids_by_region: &HashMap<String, Vec<String>>,
        ) -> CloudProviderResult<Vec<Instance>> {
            let guard = self.instances.lock().unwrap();
            let instances_by_ids = guard
                .iter()
                .filter(|x| {
                    if let Some(instances) = ids_by_region.get(x.region.as_str()) {
                        instances.contains(&x.id)
                    } else {
                        false
                    }
                })
                .cloned()
                .collect::<Vec<_>>();
            Ok(instances_by_ids)
        }

        async fn start_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
        where
            I: Iterator<Item = &'a Instance> + Send,
        {
            let instance_ids: Vec<_> = instances.map(|x| x.id.clone()).collect();
            let mut guard = self.instances.lock().unwrap();
            for instance in guard.iter_mut().filter(|x| instance_ids.contains(&x.id)) {
                instance.status = "running".into();
            }
            Ok(())
        }

        async fn stop_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
        where
            I: Iterator<Item = &'a Instance> + Send,
        {
            let instance_ids: Vec<_> = instances.map(|x| x.id.clone()).collect();
            let mut guard = self.instances.lock().unwrap();
            for instance in guard.iter_mut().filter(|x| instance_ids.contains(&x.id)) {
                instance.status = "stopped".into();
            }
            Ok(())
        }

        async fn create_instance<S>(
            &self,
            region: S,
            role: InstanceRole,
            quantity: usize,
            use_spot_instances: bool,
            _id: String,
        ) -> CloudProviderResult<Vec<Instance>>
        where
            S: Into<String> + Serialize + Send,
        {
            let mut guard = self.instances.lock().unwrap();
            let mut instances = Vec::new();
            let region = region.into();
            for _ in 0..quantity {
                let id = guard.len();
                let instance = Instance {
                    id: id.to_string(),
                    region: region.clone(),
                    main_ip: format!("0.0.0.{id}").parse().unwrap(),
                    private_ip: format!("0.0.0.{id}").parse().unwrap(),
                    tags: Vec::new(),
                    specs: self.settings.node_specs.clone(),
                    status: "running".into(),
                    role: role.clone(),
                    lifecycle: if use_spot_instances {
                        InstanceLifecycle::Spot
                    } else {
                        InstanceLifecycle::OnDemand
                    },
                };
                guard.push(instance.clone());
                instances.push(instance);
            }

            Ok(instances)
        }

        async fn delete_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
        where
            I: Iterator<Item = &'a Instance> + Send,
        {
            let ids_to_delete = instances.map(|x| x.id.clone()).collect::<Vec<_>>();
            let mut guard = self.instances.lock().unwrap();
            guard.retain(|x| !ids_to_delete.contains(&x.id));
            Ok(())
        }

        async fn register_ssh_public_key(&self, _public_key: String) -> CloudProviderResult<()> {
            Ok(())
        }

        async fn instance_setup_commands(&self) -> CloudProviderResult<Vec<String>> {
            Ok(Vec::new())
        }
        fn instances(&self) -> Vec<Instance> {
            self.instances.lock().unwrap().clone()
        }
    }
}
