// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Helpers shared by the tests of the gRPC metrics.

use std::{collections::HashMap, sync::Arc};

use futures::StreamExt;
use iota_config::node::GrpcApiConfig;
use iota_grpc_server::{GrpcServerHandle, GrpcServerMetrics};
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        ledger_service::{
            GetObjectsRequest, GetObjectsResponse, ObjectRequest, ObjectRequests,
            ledger_service_client::LedgerServiceClient,
        },
        move_package_service::{
            ListPackageVersionsRequest, ListPackageVersionsResponse,
            move_package_service_client::MovePackageServiceClient,
        },
        state_service::{
            ListDynamicFieldsRequest, ListDynamicFieldsResponse, ListOwnedObjectsRequest,
            ListOwnedObjectsResponse, state_service_client::StateServiceClient,
        },
        types::ObjectReference,
    },
};
pub use iota_metrics::test_utils::MetricsReader;
use iota_sdk_types::{
    Address, MoveObjectType, MoveStruct, ObjectId, Owner, StructTag, TransactionDigest,
};
use iota_types::{
    gas_coin::GasCoin,
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    storage::{
        AccountOwnedObjectInfo, DynamicFieldKey, OwnedObjectCursor, PackageVersionInfo,
        PackageVersionKey,
    },
    transaction_executor::TransactionExecutor,
};
use prometheus_filtered::Registry;
use tonic::transport::Channel;

use crate::common::{
    MockGrpcStateReader, create_large_object, object_id_proto, owner_proto, start_test_server_with,
};

pub const PREFIX: &str = "node_grpc";

pub async fn start(state_reader: MockGrpcStateReader) -> (GrpcServerHandle, MetricsReader) {
    start_with(state_reader, |_| {}).await
}

pub async fn start_with(
    state_reader: MockGrpcStateReader,
    customize: impl FnOnce(&mut GrpcApiConfig),
) -> (GrpcServerHandle, MetricsReader) {
    start_with_executor(state_reader, None, customize).await
}

pub async fn start_with_executor(
    state_reader: MockGrpcStateReader,
    executor: Option<Arc<dyn TransactionExecutor>>,
    customize: impl FnOnce(&mut GrpcApiConfig),
) -> (GrpcServerHandle, MetricsReader) {
    let registry = Registry::new();
    let metrics = GrpcServerMetrics::new(&registry);
    let (handle, _reader) = start_test_server_with(
        Arc::new(state_reader),
        executor,
        None,
        None,
        Some(metrics),
        |config| {
            config.max_get_objects_batch_size = 100;
            customize(config);
        },
    )
    .await;
    (handle, MetricsReader::new(&registry).with_prefix(PREFIX))
}

pub async fn connect(handle: &GrpcServerHandle) -> Channel {
    Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap()
}

fn object_requests(ids: &[ObjectId]) -> ObjectRequests {
    ObjectRequests::default().with_requests(
        ids.iter()
            .map(|id| {
                ObjectRequest::default().with_object_ref(
                    ObjectReference::default().with_object_id(crate::common::object_id_proto(*id)),
                )
            })
            .collect(),
    )
}

/// A mock with `count` objects of about `padding` bytes each.
fn mock_with_objects(count: usize, padding: usize) -> (MockGrpcStateReader, Vec<ObjectId>) {
    let mut objects = HashMap::new();
    let mut ids = Vec::new();
    for _ in 0..count {
        let (id, object) = create_large_object(padding);
        ids.push(id);
        objects.insert(id, object);
    }
    (
        MockGrpcStateReader {
            objects,
            ..Default::default()
        },
        ids,
    )
}

/// A GetObjects call of 30 objects of about 50 kB each, with a message size
/// of 1 MiB: its response has several messages.
pub struct ObjectsCall {
    pub reader: MetricsReader,
    _handle: GrpcServerHandle,
    ledger: LedgerServiceClient<Channel>,
    ids: Vec<ObjectId>,
    pub responses: Vec<GetObjectsResponse>,
}

pub const OBJECTS: usize = 30;

pub async fn call_get_objects() -> ObjectsCall {
    call_get_objects_of(OBJECTS, 50_000, MIB).await
}

const MIB: u32 = 1024 * 1024;

/// A GetObjects call of `count` objects of about `padding` bytes each, with the
/// message size `max_message_size`.
pub async fn call_get_objects_of(
    count: usize,
    padding: usize,
    max_message_size: u32,
) -> ObjectsCall {
    let (mock, ids) = mock_with_objects(count, padding);
    let (handle, reader) = start_with(mock, |config| {
        config.max_message_size_bytes = config.max_message_size_bytes.max(max_message_size)
    })
    .await;
    let mut ledger = LedgerServiceClient::new(connect(&handle).await)
        .max_decoding_message_size(128 * 1024 * 1024);
    let request = GetObjectsRequest::default()
        .with_requests(object_requests(&ids))
        .with_read_mask(prost_types::FieldMask::from_str("reference,bcs"))
        .with_max_message_size_bytes(max_message_size);
    let responses: Vec<_> = ledger
        .get_objects(request)
        .await
        .unwrap()
        .into_inner()
        .map(|response| response.unwrap())
        .collect()
        .await;
    assert!(!responses.is_empty());
    ObjectsCall {
        reader,
        _handle: handle,
        ledger,
        ids,
        responses,
    }
}

impl ObjectsCall {
    /// A GetObjects call for one object, with no `max_message_size_bytes`.
    pub async fn call_without_max_message_size(&mut self) {
        self.ledger
            .get_objects(
                GetObjectsRequest::default()
                    .with_requests(object_requests(&self.ids[..1]))
                    .with_read_mask(prost_types::FieldMask::from_str("reference")),
            )
            .await
            .unwrap()
            .into_inner()
            .collect::<Vec<_>>()
            .await;
    }
}

/// A store with the data of the three list calls: owned objects, dynamic
/// fields and package versions.
pub struct ListFixture {
    pub mock: MockGrpcStateReader,
    pub owner: Address,
    pub parent: ObjectId,
    pub package: ObjectId,
}

/// The responses of the three list calls.
pub struct ListResponses {
    pub owned: ListOwnedObjectsResponse,
    pub fields: ListDynamicFieldsResponse,
    pub versions: ListPackageVersionsResponse,
}

/// Makes the three list calls on `channel`, for the data of a [`ListFixture`].
pub async fn call_list_methods(
    channel: Channel,
    owner: Address,
    parent: ObjectId,
    package: ObjectId,
) -> ListResponses {
    let owned = StateServiceClient::new(channel.clone())
        .list_owned_objects(ListOwnedObjectsRequest::default().with_owner(owner_proto(owner)))
        .await
        .unwrap()
        .into_inner();
    let fields = StateServiceClient::new(channel.clone())
        .list_dynamic_fields(
            ListDynamicFieldsRequest::default().with_parent(object_id_proto(parent)),
        )
        .await
        .unwrap()
        .into_inner();
    let versions = MovePackageServiceClient::new(channel)
        .list_package_versions(
            ListPackageVersionsRequest::default().with_package_id(object_id_proto(package)),
        )
        .await
        .unwrap()
        .into_inner();
    ListResponses {
        owned,
        fields,
        versions,
    }
}

pub fn list_fixture() -> ListFixture {
    list_fixture_with_coins(3)
}

/// Like [`list_fixture`], with `coins` owned objects.
pub fn list_fixture_with_coins(coins: u64) -> ListFixture {
    let owner = Address::random();
    let parent = ObjectId::random();
    let package = ObjectId::random();
    let mut mock = MockGrpcStateReader::default();
    for balance in 1..=coins {
        let id = ObjectId::random();
        let move_struct = MoveStruct::new_from_execution_with_limit(
            StructTag::new_gas_coin(),
            OBJECT_START_VERSION,
            GasCoin::new(id, balance).to_bcs_bytes(),
            256,
        )
        .unwrap();
        mock.objects.insert(
            id,
            Object::new_move(
                move_struct,
                Owner::Address(owner),
                TransactionDigest::GENESIS_MARKER,
            ),
        );
        mock.owned_objects.push((
            AccountOwnedObjectInfo {
                owner,
                object_id: id,
                version: OBJECT_START_VERSION,
                object_type: MoveObjectType::from(StructTag::new_gas_coin()),
            },
            OwnedObjectCursor {
                object_type_identifier: 1,
                object_type_params: 1,
                inverted_balance: Some(!balance),
                object_id: id,
            },
        ));
    }
    mock.owned_objects
        .sort_by_key(|(_, cursor)| (cursor.inverted_balance, cursor.object_id));
    let mut field_ids: Vec<ObjectId> = (0..4).map(|_| ObjectId::random()).collect();
    field_ids.sort();
    mock.dynamic_fields = field_ids
        .iter()
        .map(|id| DynamicFieldKey::new(parent, *id))
        .collect();
    mock.package_versions = (1..=5u64)
        .map(|version| {
            (
                PackageVersionKey {
                    original_package_id: package,
                    version,
                },
                PackageVersionInfo {
                    storage_id: ObjectId::random(),
                },
            )
        })
        .collect();
    ListFixture {
        mock,
        owner,
        parent,
        package,
    }
}
