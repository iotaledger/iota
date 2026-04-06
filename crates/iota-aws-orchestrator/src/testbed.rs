// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::HashMap, time::Duration};

use futures::future::try_join_all;
use prettytable::{Table, row};
use tokio::time::{self, Instant};

use super::client::{Instance, InstanceLifecycle, InstanceRole};
use crate::{
    client::ServerProviderClient,
    display,
    error::{TestbedError, TestbedResult},
    settings::Settings,
    ssh::SshConnection,
};

/// Represents a testbed running on a cloud provider.
pub struct Testbed<C> {
    /// The testbed's settings.
    settings: Settings,
    /// The client interfacing with the cloud provider.
    client: C,
    /// List of Node instances.
    node_instances: Vec<Instance>,
    /// List of dedicated Client instances.
    client_instances: Option<Vec<Instance>>,
    /// Dedicated Metrics Instance
    metrics_instance: Option<Instance>,
}

impl<C: ServerProviderClient> Testbed<C> {
    /// Create a new testbed instance with the specified settings and client.
    pub async fn new(settings: Settings, client: C) -> TestbedResult<Self> {
        let public_key = settings.load_ssh_public_key()?;
        client.register_ssh_public_key(public_key).await?;
        let node_instances = client.list_instances_by_role(InstanceRole::Node).await?;
        let client_instances = client.list_instances_by_role(InstanceRole::Client).await?;
        let metrics_instance = client.list_instances_by_role(InstanceRole::Metrics).await?;

        Ok(Self {
            settings,
            client,
            node_instances,
            client_instances: if client_instances.is_empty() {
                None
            } else {
                Some(client_instances)
            },
            metrics_instance: metrics_instance.into_iter().next(),
        })
    }

    /// Return the username to connect to the instances through ssh.
    pub fn username(&self) -> &'static str {
        C::USERNAME
    }

    /// Return the list of instances of the testbed.
    pub fn instances(&self) -> Vec<Instance> {
        let mut instances = self.node_instances.clone();
        if let Some(instance) = &self.metrics_instance {
            instances.push(instance.clone());
        }
        if let Some(client_instances) = &self.client_instances {
            instances.extend(client_instances.clone());
        }
        instances
    }
    /// Return the list of Node instances of the testbed.
    pub fn node_instances(&self) -> Vec<Instance> {
        self.node_instances.clone()
    }
    /// Return the list of Client instances of the testbed.
    pub fn client_instances(&self) -> Vec<Instance> {
        match &self.client_instances {
            Some(instances) => instances.clone(),
            None => self.node_instances.clone(),
        }
    }
    /// Return the Metrics Instance of the testbed.
    pub fn metrics_instance(&self) -> Option<Instance> {
        self.metrics_instance.clone()
    }

    /// Return the list of provider-specific instance setup commands.
    pub async fn setup_commands(&self) -> TestbedResult<Vec<String>> {
        self.client
            .instance_setup_commands()
            .await
            .map_err(TestbedError::from)
    }

    /// Print the current status of the testbed.
    pub fn status(&self) {
        let instances_by_region = self.instances().into_iter().fold(
            HashMap::new(),
            |mut acc: HashMap<String, Vec<Instance>>, i| {
                acc.entry(i.region.clone()).or_default().push(i);
                acc
            },
        );

        let mut table = Table::new();
        table.set_format(display::default_table_format());

        for (i, (region, instances)) in instances_by_region.iter().enumerate() {
            table.add_row(row![bH2->region.to_uppercase()]);
            let mut j = 0;
            for instance in instances {
                if j % 5 == 0 {
                    table.add_row(row![]);
                }
                let private_key_file = self.settings.ssh_private_key_file.display();
                let username = C::USERNAME;
                let ip = instance.main_ip;
                let private_ip = instance.private_ip;
                let role = instance.role.to_string();
                let lifecycle = instance.lifecycle.to_string();
                let connect = format!(
                    "[{role:<7}] [{lifecycle:<8}] [{private_ip:<15}] ssh -i {private_key_file} {username}@{ip}"
                );
                if !instance.is_terminated() {
                    if instance.is_active() {
                        table.add_row(row![bFg->format!("{j}"), connect]);
                    } else {
                        table.add_row(row![bFr->format!("{j}"), connect]);
                    }
                    j += 1;
                }
            }
            if i != instances_by_region.len() - 1 {
                table.add_row(row![]);
            }
        }

        display::newline();
        display::config("Client", &self.client);
        let repo = &self.settings.repository;
        display::config("Repo", format!("{} ({})", repo.url, repo.commit));
        display::newline();
        table.printstd();
        display::newline();
    }

    /// Populate the testbed by creating the specified amount of instances per
    /// region. The total number of instances created is thus the specified
    /// amount x the number of regions.
    pub async fn deploy(
        &mut self,
        quantity: usize,
        skip_monitoring: bool,
        dedicated_clients: usize,
        use_spot_instances: bool,
        id: String,
    ) -> TestbedResult<()> {
        display::action(format!("Deploying instances ({quantity} per region)"));

        let mut instances: Vec<Instance> = vec![];

        if !skip_monitoring {
            let metrics_region = self
                .settings
                .regions
                .first()
                .expect("At least one region must be present")
                .clone();
            let metrics_instance = self
                .client
                .create_instance(metrics_region, InstanceRole::Metrics, 1, false, id.clone())
                .await?;
            instances.extend(metrics_instance);
        }

        let node_instances = {
            // Multi-region case — call create_instance per region in parallel
            let tasks = self.settings.regions.iter().map(|region| {
                self.client.create_instance(
                    region.clone(),
                    InstanceRole::Node,
                    quantity,
                    use_spot_instances,
                    id.clone(),
                )
            });

            // Run them all concurrently, flatten Vec<Vec<Instance>> → Vec<Instance>
            try_join_all(tasks)
                .await?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        instances.extend(node_instances);

        let client_instances = match dedicated_clients {
            0 => vec![],
            instance_quantity => {
                // Multi-region case — call create_instance per region in parallel
                let tasks = self.settings.regions.iter().map(|region| {
                    self.client.create_instance(
                        region.clone(),
                        InstanceRole::Client,
                        instance_quantity,
                        false,
                        id.clone(),
                    )
                });

                // Run them all concurrently, flatten Vec<Vec<Instance>> → Vec<Instance>
                try_join_all(tasks)
                    .await?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            }
        };

        instances.extend(client_instances);

        // Wait until the instances are booted.
        if cfg!(not(test)) {
            self.wait_until_reachable(instances.iter()).await?;
        }
        let node_instances = self
            .client
            .list_instances_by_role(InstanceRole::Node)
            .await?;
        let client_instances = self
            .client
            .list_instances_by_role(InstanceRole::Client)
            .await?;
        let metrics_instance = self
            .client
            .list_instances_by_role(InstanceRole::Metrics)
            .await?;
        self.node_instances = node_instances;
        self.client_instances = if client_instances.is_empty() {
            None
        } else {
            Some(client_instances)
        };
        self.metrics_instance = metrics_instance.into_iter().next();

        display::action("Deployment completed\n\n");
        display::done();
        Ok(())
    }

    /// Destroy all instances of the testbed.
    pub async fn destroy(&mut self, keep_monitoring: bool, force: bool) -> TestbedResult<()> {
        let instances_to_destroy = self
            .instances()
            .into_iter()
            .filter(|i| !(keep_monitoring && i.role == InstanceRole::Metrics))
            .collect::<Vec<_>>();
        let mut number_of_nodes_to_destroy = 0;
        let mut number_of_clients_to_destroy = 0;
        let mut number_of_metrics_to_destroy = 0;
        for instance in instances_to_destroy.iter() {
            match instance.role {
                InstanceRole::Node => {
                    number_of_nodes_to_destroy += 1;
                }
                InstanceRole::Client => {
                    number_of_clients_to_destroy += 1;
                }
                InstanceRole::Metrics => {
                    number_of_metrics_to_destroy += 1;
                }
            }
        }
        let confirmation_message = format!(
            "Confirm you want to destroy the following instances:\n\
            \n\
            \tMonitoring Instances: {}\n\
            \tNode Instances: {}\n\
            \tClient Instances: {}\n",
            number_of_metrics_to_destroy, number_of_nodes_to_destroy, number_of_clients_to_destroy,
        );
        if cfg!(not(test)) && !force && !display::confirm(confirmation_message) {
            return Ok(());
        };
        display::action("Destroying testbed");
        self.client
            .delete_instances(instances_to_destroy.iter())
            .await?;

        display::done();
        Ok(())
    }

    /// Start the specified number of instances in each region. Returns an error
    /// if there are not enough available instances.
    pub async fn start(
        &mut self,
        quantity: usize,
        dedicated_clients: usize,
        skip_monitoring: bool,
    ) -> TestbedResult<()> {
        display::action("Booting instances");

        // Gather available instances.
        let mut available = Vec::new();
        #[cfg(not(test))]
        let stopped_node_instances_by_region = self
            .node_instances()
            .into_iter()
            .filter(|i| i.is_stopped())
            .fold(
                HashMap::new(),
                |mut acc: HashMap<String, Vec<Instance>>, i| {
                    acc.entry(i.region.clone()).or_default().push(i);
                    acc
                },
            );
        #[cfg(test)]
        let stopped_node_instances_by_region = self
            .client
            .instances()
            .into_iter()
            .filter(|i| i.role == InstanceRole::Node)
            .filter(|i| i.is_stopped())
            .fold(
                HashMap::new(),
                |mut acc: HashMap<String, Vec<Instance>>, i| {
                    acc.entry(i.region.clone()).or_default().push(i);
                    acc
                },
            );
        for (_, instances) in stopped_node_instances_by_region {
            if instances.len() < quantity {
                return Err(TestbedError::InsufficientCapacity(
                    quantity - instances.len(),
                ));
            }
            available.extend(instances.into_iter().take(quantity));
        }

        if !skip_monitoring {
            if let Some(metrics_instance) = &self.metrics_instance {
                if metrics_instance.is_stopped() {
                    available.push(metrics_instance.clone());
                } else {
                    return Err(TestbedError::MetricsServerMissing());
                }
            }
        }
        if dedicated_clients > 0 {
            if let Some(dedicated_client_nodes) = &self.client_instances {
                let stopped_client_instances_by_region = dedicated_client_nodes
                    .iter()
                    .filter(|i| i.is_stopped())
                    .fold(
                        HashMap::new(),
                        |mut acc: HashMap<String, Vec<Instance>>, i| {
                            acc.entry(i.region.clone()).or_default().push(i.clone());
                            acc
                        },
                    );
                for (_, instances) in stopped_client_instances_by_region {
                    if instances.len() < dedicated_clients {
                        return Err(TestbedError::InsufficientDedicatedClientCapacity(
                            dedicated_clients - instances.len(),
                        ));
                    }
                    available.extend(instances.into_iter().take(dedicated_clients));
                }
            }
        }

        // Start instances.
        self.client.start_instances(available.iter()).await?;

        // Wait until the instances are started.
        if cfg!(not(test)) {
            self.wait_until_reachable(available.iter()).await?;
        }
        let node_instances = self
            .client
            .list_instances_by_role(InstanceRole::Node)
            .await?;
        let client_instances = self
            .client
            .list_instances_by_role(InstanceRole::Client)
            .await?;
        let metrics_instance = self
            .client
            .list_instances_by_role(InstanceRole::Metrics)
            .await?;
        self.node_instances = node_instances;
        self.client_instances = if client_instances.is_empty() {
            None
        } else {
            Some(client_instances)
        };
        self.metrics_instance = metrics_instance.into_iter().next();

        display::done();
        Ok(())
    }

    /// Stop all instances of the testbed.
    pub async fn stop(&mut self, keep_monitoring: bool) -> TestbedResult<()> {
        display::action("Stopping instances");

        // Stop all instances.
        self.client
            .stop_instances(self.instances().iter().filter(|i| {
                i.is_active()
                    && !(i.role == InstanceRole::Metrics && keep_monitoring)
                    && i.lifecycle == InstanceLifecycle::OnDemand
            }))
            .await?;

        // Wait until the instances are stopped.
        loop {
            let mut instances = self
                .client
                .list_instances_by_role(InstanceRole::Node)
                .await?;
            let client_instances = self
                .client
                .list_instances_by_role(InstanceRole::Client)
                .await?;
            instances.extend(client_instances);
            if !keep_monitoring {
                let metrics_instance = self
                    .client
                    .list_instances_by_role(InstanceRole::Metrics)
                    .await?;
                instances.extend(metrics_instance);
            }

            if instances.iter().all(|x| x.is_inactive()) {
                break;
            }
        }

        display::done();
        Ok(())
    }

    /// Wait until all specified instances are ready to accept ssh connections.
    async fn wait_until_reachable<'a, I>(&self, instances: I) -> TestbedResult<()>
    where
        I: Iterator<Item = &'a Instance> + Clone,
    {
        let instance_region_and_ids = instances.fold(
            HashMap::new(),
            |mut acc: HashMap<String, Vec<String>>, i| {
                acc.entry(i.region.clone()).or_default().push(i.id.clone());
                acc
            },
        );
        let mut interval = time::interval(Duration::from_secs(5));
        interval.tick().await; // The first tick returns immediately.

        let start = Instant::now();
        loop {
            let now = interval.tick().await;
            let elapsed = now.duration_since(start).as_secs_f64().ceil() as u64;
            display::status(format!("{elapsed}s"));
            let instances = self
                .client
                .list_instances_by_region_and_ids(&instance_region_and_ids)
                .await?;

            let futures = instances.iter().map(|instance| {
                let private_key_file = self.settings.ssh_private_key_file.clone();
                SshConnection::new(
                    instance.ssh_address(),
                    C::USERNAME,
                    private_key_file,
                    None,
                    None,
                )
            });
            if try_join_all(futures).await.is_ok() {
                break;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::{
        client::{InstanceRole, ServerProviderClient, test_client::TestClient},
        settings::Settings,
        testbed::Testbed,
    };

    #[tokio::test]
    async fn deploy() {
        let settings = Settings::new_for_test();
        let client = TestClient::new(settings.clone());
        let mut testbed = Testbed::new(settings, client).await.unwrap();

        testbed
            .deploy(5, true, 0, false, "test".to_string())
            .await
            .unwrap();

        assert_eq!(
            testbed.node_instances.len(),
            5 * testbed.settings.number_of_regions()
        );
        for (i, instance) in testbed.node_instances.iter().enumerate() {
            assert_eq!(i.to_string(), instance.id);
        }
    }

    #[tokio::test]
    async fn destroy() {
        let settings = Settings::new_for_test();
        let client = TestClient::new(settings.clone());
        let mut testbed = Testbed::new(settings, client).await.unwrap();

        testbed.destroy(false, true).await.unwrap();

        assert_eq!(testbed.node_instances.len(), 0);
    }

    #[tokio::test]
    async fn start() {
        let settings = Settings::new_for_test();
        let client = TestClient::new(settings.clone());
        let mut testbed = Testbed::new(settings, client).await.unwrap();
        testbed
            .deploy(5, true, 0, false, "test".to_string())
            .await
            .unwrap();
        testbed.stop(false).await.unwrap();

        let result = testbed.start(2, 0, true).await;

        assert!(result.is_ok());
        for region in &testbed.settings.regions {
            let active = testbed
                .client
                .instances()
                .iter()
                .filter(|i| i.role == InstanceRole::Node)
                .filter(|x| x.is_active() && &x.region == region)
                .count();
            assert_eq!(active, 2);

            let inactive = testbed
                .client
                .instances()
                .iter()
                .filter(|i| i.role == InstanceRole::Node)
                .filter(|x| x.is_inactive() && &x.region == region)
                .count();
            assert_eq!(inactive, 3);
        }
    }

    #[tokio::test]
    async fn stop() {
        let settings = Settings::new_for_test();
        let client = TestClient::new(settings.clone());
        let mut testbed = Testbed::new(settings, client).await.unwrap();
        testbed
            .deploy(5, true, 0, false, "test".to_string())
            .await
            .unwrap();
        testbed.start(2, 0, true).await.unwrap();

        testbed.stop(false).await.unwrap();

        assert!(testbed.client.instances().iter().all(|x| x.is_inactive()))
    }
}
