// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    collections::HashMap,
    fmt::{Debug, Display},
};

use aws_runtime::env_config::file::{EnvConfigFileKind, EnvConfigFiles};
use aws_sdk_ec2::{
    config::Region,
    primitives::Blob,
    types::{
        BlockDeviceMapping, EbsBlockDevice, EphemeralNvmeSupport, Filter,
        InstanceInterruptionBehavior, InstanceMarketOptionsRequest, MarketType, ResourceType,
        SpotInstanceType, SpotMarketOptions, Tag, TagSpecification, VolumeType,
        builders::FilterBuilder,
    },
};
use aws_smithy_runtime_api::client::{behavior_version::BehaviorVersion, result::SdkError};
use serde::Serialize;

use super::{Instance, InstanceLifecycle, InstanceRole, ServerProviderClient};
use crate::{
    display,
    error::{CloudProviderError, CloudProviderResult},
    settings::Settings,
};

// Make a request error from an AWS error message.
impl<T> From<SdkError<T, aws_smithy_runtime_api::client::orchestrator::HttpResponse>>
    for CloudProviderError
where
    T: Debug + std::error::Error + Send + Sync + 'static,
{
    fn from(e: SdkError<T, aws_smithy_runtime_api::client::orchestrator::HttpResponse>) -> Self {
        Self::Request(format!("{:?}", e.into_source()))
    }
}

/// A AWS client.
pub struct AwsClient {
    /// The settings of the testbed.
    settings: Settings,
    /// A list of clients, one per AWS region.
    clients: HashMap<String, aws_sdk_ec2::Client>,
}

impl Display for AwsClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "AWS EC2 client v{}", aws_sdk_ec2::meta::PKG_VERSION)
    }
}

impl AwsClient {
    const UBUNTU_NAME_PATTERN: &'static str =
        "ubuntu/images/hvm-ssd-gp3/ubuntu-noble-24.04-amd64-server-*";
    const CANONICAL_OWNER_ID: &'static str = "099720109477";

    /// Make a new AWS client.
    pub async fn new(settings: Settings) -> Self {
        let profile_files = EnvConfigFiles::builder()
            .with_file(EnvConfigFileKind::Credentials, &settings.token_file)
            .with_contents(EnvConfigFileKind::Config, "[default]\noutput=json")
            .build();

        let mut clients = HashMap::new();
        for region in settings.regions.clone() {
            let sdk_config = aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(region.clone()))
                .profile_files(profile_files.clone())
                .load()
                .await;
            let client = aws_sdk_ec2::Client::new(&sdk_config);
            clients.insert(region, client);
        }

        Self { settings, clients }
    }

    /// Parse an AWS response and ignore errors if they mean a request is a
    /// duplicate.
    fn check_but_ignore_duplicates<T, E>(
        response: Result<
            T,
            SdkError<E, aws_smithy_runtime_api::client::orchestrator::HttpResponse>,
        >,
    ) -> CloudProviderResult<()>
    where
        E: Debug + std::error::Error + Send + Sync + 'static,
    {
        if let Err(e) = response {
            let error_message = format!("{e:?}");
            if !error_message.to_lowercase().contains("duplicate") {
                return Err(e.into());
            }
        }
        Ok(())
    }
    fn get_tag_value(instance: &aws_sdk_ec2::types::Instance, key: &str) -> Option<String> {
        instance
            .tags()
            .iter()
            .find(|tag| tag.key().is_some_and(|k| k == key))
            .and_then(|tag| tag.value().map(|v| v.to_string()))
    }
    /// Convert an AWS instance into an orchestrator instance (used in the rest
    /// of the codebase).
    fn make_instance(
        &self,
        region: String,
        aws_instance: &aws_sdk_ec2::types::Instance,
    ) -> Instance {
        let role: InstanceRole = Self::get_tag_value(aws_instance, "Role")
            .expect("AWS instance should have a role")
            .as_str()
            .into();
        let lifecycle: InstanceLifecycle =
            if let Some(aws_sdk_ec2::types::InstanceLifecycleType::Spot) =
                aws_instance.instance_lifecycle
            {
                InstanceLifecycle::Spot
            } else {
                InstanceLifecycle::OnDemand
            };
        Instance {
            id: aws_instance
                .instance_id()
                .expect("AWS instance should have an id")
                .into(),
            region,
            main_ip: aws_instance
                .public_ip_address()
                .unwrap_or("0.0.0.0") // Stopped instances do not have an ip address.
                .parse()
                .expect("AWS instance should have a valid ip"),
            private_ip: aws_instance
                .private_ip_address()
                .unwrap_or("0.0.0.0") // Stopped instances do not have an ip address.
                .parse()
                .expect("AWS instance should have a valid ip"),
            tags: vec![self.settings.testbed_id.clone()],
            specs: format!(
                "{:?}",
                aws_instance
                    .instance_type()
                    .expect("AWS instance should have a type")
            ),
            status: format!(
                "{:?}",
                aws_instance
                    .state()
                    .expect("AWS instance should have a state")
                    .name()
                    .expect("AWS status should have a name")
            ),
            role,
            lifecycle,
        }
    }

    /// Query the image id determining the os of the instances.
    /// NOTE: The image id changes depending on the region.
    async fn find_image_id(&self, client: &aws_sdk_ec2::Client) -> CloudProviderResult<String> {
        // Use a more general filter that doesn't depend on specific build dates
        let filters = [
            // Filter for Ubuntu 24.04 LTS
            FilterBuilder::default()
                .name("name")
                .values(Self::UBUNTU_NAME_PATTERN)
                .build(),
            // Only look at images from Canonical
            FilterBuilder::default()
                .name("owner-id")
                .values(Self::CANONICAL_OWNER_ID)
                .build(),
            // Only want available images
            FilterBuilder::default()
                .name("state")
                .values("available")
                .build(),
        ];

        // Query images with these filters
        let request = client.describe_images().set_filters(Some(filters.to_vec()));
        let response = request.send().await?;

        // Sort images by creation date (newest first)
        let mut images = response.images().to_vec();
        images.sort_by(|a, b| {
            let a_date = a.creation_date().unwrap_or("");
            let b_date = b.creation_date().unwrap_or("");
            b_date.cmp(a_date) // Reverse order to get newest first
        });

        // Select the newest image
        let image = images
            .first()
            .ok_or_else(|| CloudProviderError::Request("Cannot find Ubuntu 24.04 image".into()))?;

        image
            .image_id
            .clone()
            .ok_or_else(|| CloudProviderError::UnexpectedResponse("Image without ID".into()))
    }

    /// Create a new security group for the instance (if it doesn't already
    /// exist).
    async fn create_security_group(&self, client: &aws_sdk_ec2::Client) -> CloudProviderResult<()> {
        // Create a security group (if it doesn't already exist).
        let request = client
            .create_security_group()
            .group_name(&self.settings.testbed_id)
            .description("Allow all traffic (used for benchmarks).");

        let response = request.send().await;
        Self::check_but_ignore_duplicates(response)?;

        // Authorize all traffic on the security group.
        for protocol in ["tcp", "udp", "icmp", "icmpv6"] {
            let mut request = client
                .authorize_security_group_ingress()
                .group_name(&self.settings.testbed_id)
                .ip_protocol(protocol)
                .cidr_ip("0.0.0.0/0"); // todo - allowing 0.0.0.0 seem a bit wild?
            if protocol == "icmp" || protocol == "icmpv6" {
                request = request.from_port(-1).to_port(-1);
            } else {
                request = request.from_port(0).to_port(65535);
            }

            let response = request.send().await;
            Self::check_but_ignore_duplicates(response)?;
        }
        Ok(())
    }

    /// Return the command to mount the first (standard) NVMe drive.
    fn nvme_mount_command(&self) -> Vec<String> {
        let directory = self.settings.working_dir.display();
        vec![
            "export NVME_DRIVE=$(nvme list | awk '/NVMe Instance Storage/ {print $1; exit}')"
                .to_string(),
            "(sudo mkfs.ext4 -E nodiscard $NVME_DRIVE || true)".to_string(),
            format!("(sudo mount $NVME_DRIVE {directory} || true)"),
            format!("sudo chmod 777 -R {directory}"),
        ]
    }

    /// Check whether the instance type specified in the settings supports NVMe
    /// drives.
    async fn check_nvme_support(&self) -> CloudProviderResult<bool> {
        // Get the client for the first region. A given instance type should either have
        // NVMe support in all regions or in none.
        let client = match self
            .settings
            .regions
            .first()
            .and_then(|x| self.clients.get(x))
        {
            Some(client) => client,
            None => return Ok(false),
        };

        // Request storage details for the instance type specified in the settings.
        let request = client
            .describe_instance_types()
            .instance_types(self.settings.node_specs.as_str().into());

        // Send the request.
        let response = request.send().await?;

        // Return true if the response contains references to NVMe drives.
        if let Some(info) = response.instance_types().first() {
            if let Some(info) = info.instance_storage_info() {
                if info.nvme_support() == Some(&EphemeralNvmeSupport::Required) {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    fn spot_options() -> InstanceMarketOptionsRequest {
        InstanceMarketOptionsRequest::builder()
            // SPOT vs CAPACITY_BLOCK
            .market_type(MarketType::Spot)
            .spot_options(
                SpotMarketOptions::builder()
                    // One-off Spot request that ends when the instance ends.
                    .spot_instance_type(SpotInstanceType::OneTime)
                    // What to do when AWS reclaims capacity.
                    // For ephemeral test runs, terminate is usually fine.
                    .instance_interruption_behavior(InstanceInterruptionBehavior::Terminate)
                    // Usually DON'T set max_price: if omitted, you just pay current Spot price,
                    // capped by On-Demand in most regions. Setting it can increase
                    // interruptions.:contentReference[oaicite:4]{index=4}
                    .build(),
            )
            .build()
    }
}

#[async_trait::async_trait]
impl ServerProviderClient for AwsClient {
    const USERNAME: &'static str = "ubuntu";

    async fn list_instances_by_role(
        &self,
        role: InstanceRole,
    ) -> CloudProviderResult<Vec<Instance>> {
        let filter_name = Filter::builder()
            .name("tag:Name")
            .values(self.settings.testbed_id.clone())
            .build();
        let filter_role = Filter::builder()
            .name("tag:Role")
            .values(role.to_string())
            .build();
        let filter_state = Filter::builder()
            .name("instance-state-name")
            .values("pending")
            .values("running")
            .values("stopping")
            .values("stopped")
            .build();

        let mut instances = Vec::new();
        for (region, client) in &self.clients {
            let request = client.describe_instances().set_filters(Some(vec![
                filter_name.clone(),
                filter_role.clone(),
                filter_state.clone(),
            ]));
            let response = request.send().await?;
            for reservation in response.reservations() {
                for instance in reservation.instances() {
                    instances.push(self.make_instance(region.clone(), instance));
                }
            }
        }
        instances.sort_by_key(|i| i.main_ip);

        Ok(instances)
    }

    async fn list_instances_by_region_and_ids(
        &self,
        ids_by_region: &HashMap<String, Vec<String>>,
    ) -> CloudProviderResult<Vec<Instance>> {
        let mut instances = Vec::new();
        for (region, client) in &self.clients {
            let request = client
                .describe_instances()
                .set_instance_ids(ids_by_region.get(region).cloned());
            let response = request.send().await?;
            for reservation in response.reservations() {
                for instance in reservation.instances() {
                    instances.push(self.make_instance(region.clone(), instance));
                }
            }
        }

        Ok(instances)
    }

    async fn start_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send,
    {
        let mut instance_ids = HashMap::new();
        for instance in instances {
            instance_ids
                .entry(&instance.region)
                .or_insert_with(Vec::new)
                .push(instance.id.clone());
        }

        for (region, client) in &self.clients {
            let ids = instance_ids.remove(&region.to_string());
            if ids.is_some() {
                client
                    .start_instances()
                    .set_instance_ids(ids)
                    .send()
                    .await?;
            }
        }
        Ok(())
    }

    async fn stop_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send,
    {
        let mut instance_ids: HashMap<String, Vec<String>> = HashMap::new();
        for i in instances {
            if i.lifecycle == InstanceLifecycle::Spot {
                return Err(CloudProviderError::FailedToStopSpotInstance(i.id.clone()));
            }
            instance_ids
                .entry(i.region.clone())
                .or_default()
                .push(i.id.clone());
        }

        for (region, ids) in instance_ids {
            let client = self.clients.get(&region).ok_or_else(|| {
                CloudProviderError::Request(format!("Undefined region {:?}", region))
            })?;
            client
                .stop_instances()
                .set_instance_ids(Some(ids))
                .send()
                .await?;
        }
        Ok(())
    }

    async fn create_instance<S>(
        &self,
        region: S,
        role: InstanceRole,
        quantity: usize,
        use_spot_instances: bool,
        id: String,
    ) -> CloudProviderResult<Vec<Instance>>
    where
        S: Into<String> + Serialize + Send,
    {
        let region = region.into();
        let testbed_id = &self.settings.testbed_id;

        let client = self
            .clients
            .get(&region)
            .ok_or_else(|| CloudProviderError::Request(format!("Undefined region {region:?}")))?;

        // Create a security group (if needed).
        self.create_security_group(client).await?;

        // Query the image id.
        let image_id = self.find_image_id(client).await?;

        // Create a new instance.
        let tags = TagSpecification::builder()
            .resource_type(ResourceType::Instance)
            .tags(Tag::builder().key("Name").value(testbed_id).build())
            .tags(Tag::builder().key("Role").value(role.to_string()).build())
            .tags(Tag::builder().key("Id").value(id).build())
            .build();

        let storage = BlockDeviceMapping::builder()
            .device_name("/dev/sda1")
            .ebs(
                EbsBlockDevice::builder()
                    .delete_on_termination(true)
                    .volume_size(500)
                    .volume_type(VolumeType::Gp2)
                    .build(),
            )
            .build();
        let instance_type = match role {
            InstanceRole::Node => &self.settings.node_specs,
            InstanceRole::Metrics => &self.settings.metrics_specs,
            InstanceRole::Client => &self.settings.client_specs,
        };

        let mut base_request = client
            .run_instances()
            .image_id(image_id)
            .instance_type(instance_type.as_str().into())
            .key_name(testbed_id)
            .security_groups(&self.settings.testbed_id)
            .tag_specifications(tags);

        // Only the monitoring device should be EBS backed.
        if role == InstanceRole::Metrics {
            base_request = base_request.block_device_mappings(storage);
        }
        let mut collected_instances = Vec::new();
        if use_spot_instances && role == InstanceRole::Node {
            let start = tokio::time::Instant::now();
            // 5min try for spot instances
            let total_runtime = tokio::time::Duration::from_secs(300);
            while start.elapsed() < total_runtime && collected_instances.len() < quantity {
                display::status(format!(
                    "{}s/{}s: {}",
                    start.elapsed().as_secs(),
                    total_runtime.as_secs(),
                    collected_instances.len()
                ));
                let needed = (quantity - collected_instances.len()) as i32;
                let request = base_request
                    .clone()
                    .min_count(1)
                    .max_count(needed)
                    .instance_market_options(Self::spot_options());
                let result = request.send().await;
                let instances = match result {
                    Ok(response) => response
                        .instances()
                        .iter()
                        .map(|i| self.make_instance(region.clone(), i))
                        .collect(),
                    Err(_) => Vec::new(),
                };
                collected_instances.extend(instances);
            }
        }
        while collected_instances.len() < quantity {
            // some instances need to be OnDemand
            let needed = (quantity - collected_instances.len()) as i32;
            let request = base_request.clone().min_count(1).max_count(needed);
            let response = request.send().await?;
            let on_demand_instances = response
                .instances()
                .iter()
                .map(|instance| self.make_instance(region.clone(), instance))
                .collect::<Vec<_>>();
            collected_instances.extend(on_demand_instances);
            display::status(format!(
                "collected instances: {}",
                collected_instances.len()
            ));
        }
        Ok(collected_instances)
    }

    async fn delete_instances<'a, I>(&self, instances: I) -> CloudProviderResult<()>
    where
        I: Iterator<Item = &'a Instance> + Send,
    {
        let map_of_ids_by_region = instances.into_iter().fold(
            HashMap::new(),
            |mut acc: HashMap<String, Vec<String>>, i| {
                acc.entry(i.region.clone()).or_default().push(i.id.clone());
                acc
            },
        );
        for (region, ids) in map_of_ids_by_region {
            let client = self.clients.get(&region).ok_or_else(|| {
                CloudProviderError::Request(format!("Undefined region {:?}", region))
            })?;
            client
                .terminate_instances()
                .set_instance_ids(Some(ids))
                .send()
                .await?;
        }
        Ok(())
    }

    async fn register_ssh_public_key(&self, public_key: String) -> CloudProviderResult<()> {
        for client in self.clients.values() {
            let request = client
                .import_key_pair()
                .key_name(&self.settings.testbed_id)
                .public_key_material(Blob::new::<String>(public_key.clone()));

            let response = request.send().await;
            Self::check_but_ignore_duplicates(response)?;
        }
        Ok(())
    }

    async fn instance_setup_commands(&self) -> CloudProviderResult<Vec<String>> {
        if self.check_nvme_support().await? {
            Ok(self.nvme_mount_command())
        } else {
            Ok(Vec::new())
        }
    }
    #[cfg(test)]
    fn instances(&self) -> Vec<Instance> {
        // Only used under testing by the TestClient, unreachable cause no test
        // should use AwsClient.
        unreachable!()
    }
}
