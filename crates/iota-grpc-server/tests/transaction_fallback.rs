// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests for serving pruned transactions from the key-value store.
mod common;

use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use common::MockGrpcStateReader;
use iota_grpc_types::{
    field::FieldMaskUtil,
    v1::{
        ledger_service::{
            GetTransactionsRequest, TransactionRequest, TransactionRequests, TransactionResult,
            ledger_service_client::LedgerServiceClient,
        },
        object::Objects,
        transaction::ExecutedTransaction,
    },
};
use iota_node_storage::{
    KVStoreCheckpointData, KVStoreTransactionData, TransactionKeyValueStoreTrait,
};
use iota_sdk_types::{
    Address, CheckpointContents, CheckpointDigest, CheckpointSummary, Event, Identifier,
    MoveStruct, ObjectId, ObjectOut, Owner, StructTag, TransactionDigest, TransactionEffects,
    TransactionEvents,
};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    base_types::VersionNumber,
    crypto::{AccountPrivateKey, AuthorityStrongQuorumSignInfo, get_key_pair},
    effects::{TestEffectsBuilder, TransactionEffectsAPI},
    error::{IotaError, IotaResult},
    gas_coin::GasCoin,
    messages_checkpoint::{
        CertifiedCheckpointSummary, CheckpointContentsExt, CheckpointSequenceNumber,
    },
    object::{MoveStructExt, OBJECT_START_VERSION, Object},
    storage::{ObjectKey, TransactionInfo},
    transaction::{TransactionEnvelope, VerifiedTransaction},
};

/// Host named in the stub store's error text, which must never reach a client.
const STORE_HOST: &str = "kv.example";
const CHECKPOINT_SEQ: u64 = 7;
const EVENT_COUNT: usize = 2;
const CHECKPOINT_TIMESTAMP_MS: u64 = 1_700_000_000_000;

// ---------------------------------------------------------------------------
// Key-value store stub
// ---------------------------------------------------------------------------

/// A [`TransactionKeyValueStoreTrait`] stub serving what a test puts in it.
#[derive(Default)]
struct MockFallbackStore {
    transactions: HashMap<TransactionDigest, TransactionEnvelope>,
    effects: HashMap<TransactionDigest, TransactionEffects>,
    events: HashMap<TransactionDigest, TransactionEvents>,
    transaction_checkpoints: HashMap<TransactionDigest, CheckpointSequenceNumber>,
    checkpoint_summaries: HashMap<CheckpointSequenceNumber, CertifiedCheckpointSummary>,
    objects: HashMap<ObjectKey, Object>,
    /// Methods whose reads fail.
    failing: HashSet<&'static str>,
    /// Requests made, in call order; `multi_get` also records which keys it
    /// got.
    requests: Arc<Mutex<Vec<&'static str>>>,
    /// Keys of each `multi_get_objects` call.
    requested_objects: Arc<Mutex<Vec<Vec<ObjectKey>>>>,
    /// Digests of each lookup, `multi_get` (transactions, then effects) and events call.
    requested_digests: Arc<Mutex<Vec<Vec<TransactionDigest>>>>,
    /// Summary keys of each `multi_get_checkpoints` call.
    requested_checkpoints: Arc<Mutex<Vec<Vec<CheckpointSequenceNumber>>>>,
    /// How long the checkpoint lookup takes.
    lookup_delay: Option<std::time::Duration>,
    evicted_objects: Arc<Mutex<Vec<ObjectKey>>>,
    evicted_events: Arc<Mutex<Vec<TransactionDigest>>>,
}

impl MockFallbackStore {
    fn serve<T>(&self, method: &'static str, value: T) -> IotaResult<T> {
        self.serve_as(method, method, value)
    }

    /// Records the call as `request` and fails it when `method` is failing.
    fn serve_as<T>(&self, method: &'static str, request: &'static str, value: T) -> IotaResult<T> {
        self.requests.lock().unwrap().push(request);
        if self.failing.contains(method) {
            return Err(IotaError::Storage(format!(
                "connection refused by http://{STORE_HOST}"
            )));
        }
        Ok(value)
    }
}

#[async_trait]
impl TransactionKeyValueStoreTrait for MockFallbackStore {
    async fn multi_get(
        &self,
        transaction_keys: &[TransactionDigest],
        effects_keys: &[TransactionDigest],
    ) -> IotaResult<KVStoreTransactionData> {
        self.requested_digests
            .lock()
            .unwrap()
            .push([transaction_keys, effects_keys].concat());
        let request = match (transaction_keys.is_empty(), effects_keys.is_empty()) {
            (false, false) => "multi_get(transaction, effects)",
            (false, true) => "multi_get(transaction)",
            (true, false) => "multi_get(effects)",
            (true, true) => "multi_get()",
        };
        self.serve_as(
            "multi_get",
            request,
            (
                transaction_keys
                    .iter()
                    .map(|digest| self.transactions.get(digest).cloned())
                    .collect(),
                effects_keys
                    .iter()
                    .map(|digest| self.effects.get(digest).cloned())
                    .collect(),
            ),
        )
    }

    async fn multi_get_checkpoints(
        &self,
        checkpoint_summaries: &[CheckpointSequenceNumber],
        _checkpoint_contents: &[CheckpointSequenceNumber],
        _checkpoint_summaries_by_digest: &[CheckpointDigest],
    ) -> IotaResult<KVStoreCheckpointData> {
        self.requested_checkpoints
            .lock()
            .unwrap()
            .push(checkpoint_summaries.to_vec());
        self.serve(
            "multi_get_checkpoints",
            (
                checkpoint_summaries
                    .iter()
                    .map(|seq| self.checkpoint_summaries.get(seq).cloned())
                    .collect(),
                Vec::new(),
                Vec::new(),
            ),
        )
    }

    async fn get_transaction_perpetual_checkpoint(
        &self,
        _digest: TransactionDigest,
    ) -> IotaResult<Option<CheckpointSequenceNumber>> {
        self.serve("get_transaction_perpetual_checkpoint", None)
    }

    async fn get_object(
        &self,
        _object_id: ObjectId,
        _version: VersionNumber,
    ) -> IotaResult<Option<Object>> {
        self.serve("get_object", None)
    }

    async fn multi_get_objects(
        &self,
        object_keys: &[ObjectKey],
    ) -> IotaResult<Vec<Option<Object>>> {
        self.requested_objects
            .lock()
            .unwrap()
            .push(object_keys.to_vec());
        self.serve(
            "multi_get_objects",
            object_keys
                .iter()
                .map(|key| self.objects.get(key).cloned())
                .collect(),
        )
    }

    async fn multi_get_transactions_perpetual_checkpoints(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<CheckpointSequenceNumber>>> {
        self.requested_digests
            .lock()
            .unwrap()
            .push(digests.to_vec());
        if let Some(delay) = self.lookup_delay {
            tokio::time::sleep(delay).await;
        }
        self.serve(
            "multi_get_transactions_perpetual_checkpoints",
            digests
                .iter()
                .map(|digest| self.transaction_checkpoints.get(digest).copied())
                .collect(),
        )
    }

    async fn multi_get_events_by_tx_digests(
        &self,
        digests: &[TransactionDigest],
    ) -> IotaResult<Vec<Option<TransactionEvents>>> {
        self.requested_digests
            .lock()
            .unwrap()
            .push(digests.to_vec());
        self.serve(
            "multi_get_events_by_tx_digests",
            digests
                .iter()
                .map(|digest| self.events.get(digest).cloned())
                .collect(),
        )
    }

    async fn evict_objects(&self, object_keys: &[ObjectKey]) {
        self.evicted_objects
            .lock()
            .unwrap()
            .extend_from_slice(object_keys);
    }

    async fn evict_events_by_tx_digests(&self, digests: &[TransactionDigest]) {
        self.evicted_events
            .lock()
            .unwrap()
            .extend_from_slice(digests);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A transaction with its effects and events, and the objects it reads and
/// writes at the digests its effects hold.
struct TestTransaction {
    transaction: TransactionEnvelope,
    effects: TransactionEffects,
    events: TransactionEvents,
    input_objects: Vec<Object>,
    output_objects: Vec<Object>,
}

impl TestTransaction {
    fn digest(&self) -> TransactionDigest {
        *self.transaction.digest()
    }
}

/// A transfer paying gas with one object and sending another, emitting
/// `EVENT_COUNT` events if `emits_events`.
fn test_transaction(emits_events: bool) -> TestTransaction {
    let (sender, key): (_, AccountPrivateKey) = get_key_pair();
    let input_objects = vec![
        test_object(ObjectId::random(), OBJECT_START_VERSION),
        test_object(ObjectId::random(), OBJECT_START_VERSION),
    ];
    let transaction =
        TestTransactionBuilder::new(sender, input_objects[0].as_inner().object_ref(), 1000)
            .transfer(input_objects[1].as_inner().object_ref(), sender)
            .build_and_sign(&key);
    let event_count = if emits_events { EVENT_COUNT } else { 0 };
    let events = TransactionEvents(
        (0..event_count)
            .map(|_| Event {
                package_id: ObjectId::ZERO,
                module: Identifier::from_static("test_module"),
                sender,
                struct_tag: StructTag::new(
                    Address::ZERO,
                    Identifier::from_static("test_module"),
                    Identifier::from_static("TestEvent"),
                    vec![],
                ),
                contents: vec![0; 8],
            })
            .collect(),
    );
    let effects = TestEffectsBuilder::new(transaction.data());
    let effects = if emits_events {
        effects.with_events_digest(events.digest())
    } else {
        effects
    };
    let mut effects = effects.build();

    // `TestEffectsBuilder` writes placeholder output digests.
    let TransactionEffects::V1(effects_v1) = &mut effects else {
        panic!("unexpected effects version");
    };
    let lamport_version = effects_v1.lamport_version;
    let mut output_objects = Vec::new();
    for changed in &mut effects_v1.changed_objects {
        if let ObjectOut::ObjectWrite { digest, .. } = &mut changed.output_state {
            let object = test_object(changed.object_id, lamport_version);
            *digest = object.as_inner().digest();
            output_objects.push(object);
        }
    }

    TestTransaction {
        transaction,
        effects,
        events,
        input_objects,
        output_objects,
    }
}

/// The summary of checkpoint `CHECKPOINT_SEQ`.
fn test_checkpoint_summary() -> CertifiedCheckpointSummary {
    let contents = CheckpointContents::new_with_digests_only_for_tests(vec![]);
    let summary = CheckpointSummary {
        epoch: 0,
        sequence_number: CHECKPOINT_SEQ,
        network_total_transactions: 0,
        contents_digest: contents.digest(),
        previous_digest: None,
        epoch_rolling_gas_cost_summary: Default::default(),
        timestamp_ms: CHECKPOINT_TIMESTAMP_MS,
        checkpoint_commitments: vec![],
        end_of_epoch_data: None,
        version_specific_data: vec![],
    };
    CertifiedCheckpointSummary::new_from_data_and_sig(
        summary,
        AuthorityStrongQuorumSignInfo {
            epoch: 0,
            signature: Default::default(),
            signers_map: Default::default(),
        },
    )
}

fn test_object(object_id: ObjectId, version: VersionNumber) -> Object {
    let move_struct = MoveStruct::new_from_execution_with_limit(
        StructTag::new_gas_coin(),
        version,
        GasCoin::new(object_id, 100).to_bcs_bytes(),
        1024,
    )
    .unwrap();
    Object::new_move(
        move_struct,
        Owner::Address(Address::random()),
        TransactionDigest::GENESIS_MARKER,
    )
}

/// Input object keys in the order of the effects.
fn input_object_keys(effects: &TransactionEffects) -> Vec<ObjectKey> {
    effects
        .old_object_metadata()
        .iter()
        .map(|modified| ObjectKey::from(modified.reference()))
        .collect()
}

/// Output object keys in the order of the effects.
fn output_object_keys(effects: &TransactionEffects) -> Vec<ObjectKey> {
    effects
        .created()
        .into_iter()
        .chain(effects.mutated())
        .chain(effects.unwrapped())
        .map(|written| ObjectKey::from(written.reference()))
        .collect()
}

fn objects_by_key<'a>(objects: impl IntoIterator<Item = &'a Object>) -> HashMap<ObjectKey, Object> {
    objects
        .into_iter()
        .map(|object| {
            (
                ObjectKey::from(object.as_inner().object_ref()),
                object.clone(),
            )
        })
        .collect()
}

/// The (object ID, version) of each served object, in order.
fn served_object_keys(objects: &Option<Objects>) -> Vec<(Vec<u8>, u64)> {
    objects
        .as_ref()
        .unwrap()
        .objects
        .iter()
        .map(|object| {
            let reference = object.reference.as_ref().unwrap();
            (
                reference.object_id.as_ref().unwrap().object_id.to_vec(),
                reference.version.unwrap(),
            )
        })
        .collect()
}

fn object_key_bytes(keys: &[ObjectKey]) -> Vec<(Vec<u8>, u64)> {
    keys.iter()
        .map(|key| (key.0.into_bytes().to_vec(), key.1.as_u64()))
        .collect()
}

fn single_executed(results: &[TransactionResult]) -> &ExecutedTransaction {
    assert_eq!(results.len(), 1);
    results[0]
        .executed_transaction()
        .unwrap()
        .unwrap_or_else(|| {
            panic!(
                "expected a transaction, got {:?}",
                results[0].error_message()
            )
        })
}

async fn start(
    state_reader: MockGrpcStateReader,
    store: Option<MockFallbackStore>,
) -> iota_grpc_server::GrpcServerHandle {
    common::start_test_server_with_transaction_fallback(
        Arc::new(state_reader),
        store.map(|store| Arc::new(store) as Arc<dyn TransactionKeyValueStoreTrait + Send + Sync>),
    )
    .await
    .0
}

/// Request `digests` and collect the per-item results.
async fn get_transactions(
    handle: &iota_grpc_server::GrpcServerHandle,
    digests: &[TransactionDigest],
    read_mask: &str,
) -> Vec<TransactionResult> {
    let channel = tonic::transport::Channel::from_shared(format!("http://{}", handle.address()))
        .unwrap()
        .connect()
        .await
        .unwrap();
    let mut client = LedgerServiceClient::new(channel);

    let request = GetTransactionsRequest::default()
        .with_requests(
            TransactionRequests::default().with_requests(
                digests
                    .iter()
                    .map(|digest| {
                        TransactionRequest::default().with_digest(
                            iota_grpc_types::v1::types::Digest::default()
                                .with_digest(digest.bytes().to_vec()),
                        )
                    })
                    .collect(),
            ),
        )
        .with_read_mask(prost_types::FieldMask::from_str(read_mask));

    let mut stream = client.get_transactions(request).await.unwrap().into_inner();
    let mut results = Vec::new();
    while let Some(message) = tokio_stream::StreamExt::next(&mut stream).await {
        results.extend(message.unwrap().transaction_results);
    }
    results
}

// ---------------------------------------------------------------------------
// Node and store states
// ---------------------------------------------------------------------------

/// What the node holds of the test transaction.
#[derive(Clone, Copy)]
struct Node {
    lowest_available: u64,
    transaction: bool,
    effects: bool,
    events: bool,
    /// The gRPC index maps the transaction to `CHECKPOINT_SEQ`.
    indexed: bool,
    summary: bool,
    objects: bool,
}

/// A node that has pruned nothing and holds nothing of the transaction.
const NEVER_PRUNED: Node = Node {
    lowest_available: 0,
    transaction: false,
    effects: false,
    events: false,
    indexed: false,
    summary: false,
    objects: false,
};

/// A node that has pruned the transaction's checkpoint.
const PRUNED: Node = Node {
    lowest_available: CHECKPOINT_SEQ + 1,
    ..NEVER_PRUNED
};

/// A node whose lowest available checkpoint is the transaction's checkpoint.
const AT_LOWEST_AVAILABLE: Node = Node {
    lowest_available: CHECKPOINT_SEQ,
    ..NEVER_PRUNED
};

/// A node that has pruned nothing and holds the transaction and its effects.
const EXECUTED: Node = Node {
    transaction: true,
    effects: true,
    ..NEVER_PRUNED
};

/// A node that has pruned the checkpoint but holds the transaction and its
/// effects.
const LOCAL: Node = Node {
    transaction: true,
    effects: true,
    ..PRUNED
};

impl Node {
    fn state(self, test: &TestTransaction) -> MockGrpcStateReader {
        let digest = test.digest();
        let mut state =
            MockGrpcStateReader::default().with_lowest_available_checkpoint(self.lowest_available);
        if self.transaction {
            state.transactions = HashMap::from([(
                digest,
                Arc::new(VerifiedTransaction::new_unchecked(test.transaction.clone())),
            )]);
        }
        if self.effects {
            state.effects = HashMap::from([(digest, test.effects.clone())]);
        }
        if self.events {
            state.events = HashMap::from([(digest, test.events.clone())]);
        }
        if self.indexed {
            state.transaction_infos = HashMap::from([(
                digest,
                TransactionInfo {
                    checkpoint: CHECKPOINT_SEQ,
                    object_types: HashMap::new(),
                },
            )]);
        }
        if self.summary {
            state.summary = Some(test_checkpoint_summary());
        }
        if self.objects {
            // Keyed by version: a mutated object is both an input and an
            // output, at different versions.
            state.object_versions =
                objects_by_key(test.input_objects.iter().chain(&test.output_objects));
        }
        state
    }
}

/// A part of the store's data.
#[derive(Clone, Copy)]
enum Part {
    TransactionAndEffects,
    Events,
    Summary,
    Objects,
}

/// What the store holds of the test transaction, which is in checkpoint
/// `CHECKPOINT_SEQ`.
#[derive(Clone, Copy)]
enum Data {
    Nothing,
    Everything,
    Without(Part),
    /// Input objects whose version differs from the effects.
    WrongVersionInputs,
    /// Output objects with other contents than the effects record.
    OtherOutputs,
    /// Events whose digest differs from the effects.
    OtherEvents,
}

#[derive(Clone, Copy)]
struct Store {
    data: Data,
    failing: &'static [&'static str],
}

const fn store(data: Data) -> Option<Store> {
    Some(Store { data, failing: &[] })
}

const EMPTY_STORE: Option<Store> = store(Data::Nothing);
const FULL_STORE: Option<Store> = store(Data::Everything);

const fn failing_store(failing: &'static [&'static str]) -> Option<Store> {
    Some(Store {
        data: Data::Everything,
        failing,
    })
}

impl Store {
    fn build(self, test: &TestTransaction) -> MockFallbackStore {
        let digest = test.digest();
        let mut store = MockFallbackStore {
            failing: self.failing.iter().copied().collect(),
            ..Default::default()
        };
        if matches!(self.data, Data::Nothing) {
            return store;
        }
        store.transactions = HashMap::from([(digest, test.transaction.clone())]);
        store.effects = HashMap::from([(digest, test.effects.clone())]);
        store.events = HashMap::from([(digest, test.events.clone())]);
        store.transaction_checkpoints = HashMap::from([(digest, CHECKPOINT_SEQ)]);
        store.checkpoint_summaries = HashMap::from([(CHECKPOINT_SEQ, test_checkpoint_summary())]);
        store.objects = objects_by_key(test.input_objects.iter().chain(&test.output_objects));
        match self.data {
            Data::Nothing | Data::Everything => {}
            Data::Without(Part::TransactionAndEffects) => {
                store.transactions.clear();
                store.effects.clear();
            }
            Data::Without(Part::Events) => store.events.clear(),
            Data::Without(Part::Summary) => store.checkpoint_summaries.clear(),
            Data::Without(Part::Objects) => store.objects.clear(),
            Data::WrongVersionInputs => {
                for key in input_object_keys(&test.effects) {
                    store
                        .objects
                        .insert(key, test_object(key.0, key.1.next().unwrap()));
                }
            }
            Data::OtherOutputs => {
                for key in output_object_keys(&test.effects) {
                    store.objects.insert(key, test_object(key.0, key.1));
                }
            }
            Data::OtherEvents => {
                store.events = HashMap::from([(digest, TransactionEvents(vec![]))]);
            }
        }
        store
    }
}

// ---------------------------------------------------------------------------
// Case table
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Expect {
    /// Served, with the timestamp of `checkpoint` when the mask asks for it.
    Served {
        checkpoint: Option<u64>,
    },
    Error(tonic::Code),
    /// `INTERNAL` with a message containing the given text.
    Internal(&'static str),
}

const MISMATCH: Expect = Expect::Internal("does not match the effects");

const NOT_FOUND: Expect = Expect::Error(tonic::Code::NotFound);
const SERVED: Expect = Expect::Served { checkpoint: None };
const SERVED_IN_CHECKPOINT: Expect = Expect::Served {
    checkpoint: Some(CHECKPOINT_SEQ),
};

/// What the store must have dropped from its cache.
#[derive(Clone, Copy)]
enum Evicted {
    Nothing,
    InputObjects,
    OutputObjects,
    Events,
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    /// Whether the effects list events.
    events: bool,
    node: Node,
    store: Option<Store>,
    read_mask: &'static str,
    expect: Expect,
    /// Store requests made, in order.
    requests: &'static [&'static str],
    evicted: Evicted,
}

const CASE: Case = Case {
    name: "",
    events: true,
    node: NEVER_PRUNED,
    store: None,
    read_mask: "transaction,effects",
    expect: NOT_FOUND,
    requests: &[],
    evicted: Evicted::Nothing,
};

const LOOKUP: &str = "multi_get_transactions_perpetual_checkpoints";
const READ_BOTH: &str = "multi_get(transaction, effects)";
const READ_EVENTS: &str = "multi_get_events_by_tx_digests";
const READ_SUMMARY: &str = "multi_get_checkpoints";
const READ_OBJECTS: &str = "multi_get_objects";

const CASES: &[Case] = &[
    // Which transactions are read from the store
    Case {
        name: "never pruned: no store request",
        store: FULL_STORE,
        expect: NOT_FOUND,
        ..CASE
    },
    Case {
        name: "pruned, unknown everywhere: only the checkpoint lookup",
        node: PRUNED,
        store: EMPTY_STORE,
        requests: &[LOOKUP],
        expect: NOT_FOUND,
        ..CASE
    },
    Case {
        name: "store checkpoint at the lowest available one: not served",
        node: AT_LOWEST_AVAILABLE,
        store: FULL_STORE,
        requests: &[LOOKUP],
        expect: NOT_FOUND,
        ..CASE
    },
    Case {
        name: "failing checkpoint lookup counts as a miss",
        node: PRUNED,
        store: failing_store(&[LOOKUP]),
        requests: &[LOOKUP],
        expect: NOT_FOUND,
        ..CASE
    },
    Case {
        name: "local effects, not in a checkpoint yet: no store request",
        node: LOCAL,
        store: EMPTY_STORE,
        read_mask: "transaction,checkpoint,timestamp",
        expect: SERVED,
        ..CASE
    },
    Case {
        name: "pruned, missing from the store after its lookup: unavailable",
        node: PRUNED,
        store: store(Data::Without(Part::TransactionAndEffects)),
        requests: &[LOOKUP, READ_BOTH],
        expect: Expect::Error(tonic::Code::Unavailable),
        ..CASE
    },
    Case {
        name: "pruned, effects missing from the store after its lookup: unavailable",
        node: PRUNED,
        store: store(Data::Without(Part::TransactionAndEffects)),
        read_mask: "effects",
        requests: &[LOOKUP, "multi_get(effects)"],
        expect: Expect::Error(tonic::Code::Unavailable),
        ..CASE
    },
    Case {
        name: "pruned, effects missing from the store: unavailable before the summary is read",
        node: PRUNED,
        store: store(Data::Without(Part::TransactionAndEffects)),
        read_mask: "effects,timestamp",
        requests: &[LOOKUP, "multi_get(effects)"],
        expect: Expect::Error(tonic::Code::Unavailable),
        ..CASE
    },
    Case {
        name: "pruned, no store",
        node: PRUNED,
        expect: NOT_FOUND,
        ..CASE
    },
    // Transaction and effects
    Case {
        name: "local transaction, pruned effects: both read from the store",
        node: Node {
            transaction: true,
            ..PRUNED
        },
        store: FULL_STORE,
        expect: SERVED,
        requests: &[LOOKUP, READ_BOTH],
        ..CASE
    },
    Case {
        name: "pruned, transaction only: only the transaction is read",
        node: PRUNED,
        store: FULL_STORE,
        read_mask: "transaction",
        expect: SERVED,
        requests: &[LOOKUP, "multi_get(transaction)"],
        ..CASE
    },
    Case {
        name: "pruned, signatures only: the transaction is read",
        node: PRUNED,
        store: FULL_STORE,
        read_mask: "signatures",
        expect: SERVED,
        requests: &[LOOKUP, "multi_get(transaction)"],
        ..CASE
    },
    Case {
        name: "local transaction, transaction only: no store request",
        node: Node {
            transaction: true,
            ..PRUNED
        },
        store: FULL_STORE,
        read_mask: "transaction",
        expect: SERVED,
        ..CASE
    },
    Case {
        name: "pruned, all fields: served from the store",
        node: PRUNED,
        store: FULL_STORE,
        read_mask: "transaction,signatures,effects,events,checkpoint,timestamp",
        expect: SERVED_IN_CHECKPOINT,
        requests: &[LOOKUP, READ_BOTH, READ_SUMMARY, READ_EVENTS],
        ..CASE
    },
    Case {
        name: "pruned, checkpoint and timestamp only: no multi_get",
        node: PRUNED,
        store: FULL_STORE,
        read_mask: "checkpoint,timestamp",
        expect: SERVED_IN_CHECKPOINT,
        requests: &[LOOKUP, READ_SUMMARY],
        ..CASE
    },
    Case {
        name: "pruned, checkpoint only: no summary read",
        node: PRUNED,
        store: store(Data::Without(Part::Summary)),
        read_mask: "checkpoint",
        expect: SERVED_IN_CHECKPOINT,
        requests: &[LOOKUP],
        ..CASE
    },
    Case {
        name: "failing transaction read",
        node: PRUNED,
        store: failing_store(&["multi_get"]),
        expect: Expect::Error(tonic::Code::Unavailable),
        requests: &[LOOKUP, READ_BOTH],
        ..CASE
    },
    // Checkpoint summary
    Case {
        name: "local summary: not read from the store",
        node: Node {
            indexed: true,
            summary: true,
            ..EXECUTED
        },
        store: FULL_STORE,
        read_mask: "transaction,checkpoint,timestamp",
        expect: SERVED_IN_CHECKPOINT,
        ..CASE
    },
    Case {
        name: "summary missing on the node, no store: internal error",
        node: Node {
            indexed: true,
            ..EXECUTED
        },
        read_mask: "checkpoint,timestamp",
        expect: Expect::Internal("Checkpoint summary"),
        ..CASE
    },
    Case {
        name: "failing summary read",
        node: PRUNED,
        store: failing_store(&[READ_SUMMARY]),
        read_mask: "timestamp",
        expect: Expect::Error(tonic::Code::Unavailable),
        requests: &[LOOKUP, READ_SUMMARY],
        ..CASE
    },
    Case {
        name: "summary missing on the node and in the store",
        node: PRUNED,
        store: store(Data::Without(Part::Summary)),
        read_mask: "checkpoint,timestamp",
        expect: Expect::Error(tonic::Code::FailedPrecondition),
        requests: &[LOOKUP, READ_SUMMARY],
        ..CASE
    },
    // Events
    Case {
        name: "pruned, local events: not read from the store",
        node: Node {
            events: true,
            ..PRUNED
        },
        store: FULL_STORE,
        read_mask: "transaction,events",
        expect: SERVED,
        requests: &[LOOKUP, READ_BOTH],
        ..CASE
    },
    Case {
        name: "pruned, events missing from the store after its lookup: unavailable",
        node: PRUNED,
        store: store(Data::Without(Part::Events)),
        read_mask: "transaction,events",
        expect: Expect::Error(tonic::Code::Unavailable),
        requests: &[LOOKUP, READ_BOTH, READ_EVENTS],
        ..CASE
    },
    Case {
        name: "not pruned, events missing on the node and in the store",
        node: LOCAL,
        store: store(Data::Without(Part::Events)),
        read_mask: "transaction,events",
        expect: Expect::Error(tonic::Code::FailedPrecondition),
        requests: &[READ_EVENTS],
        ..CASE
    },
    Case {
        name: "not pruned, events missing locally: read from the store",
        node: LOCAL,
        store: FULL_STORE,
        read_mask: "transaction,events",
        expect: SERVED,
        requests: &[READ_EVENTS],
        ..CASE
    },
    Case {
        name: "events missing, no store",
        node: LOCAL,
        read_mask: "transaction,events",
        expect: Expect::Error(tonic::Code::FailedPrecondition),
        ..CASE
    },
    Case {
        name: "effects list no events: events not read from the store",
        events: false,
        node: PRUNED,
        store: FULL_STORE,
        read_mask: "transaction,events",
        expect: SERVED,
        requests: &[LOOKUP, READ_BOTH],
        ..CASE
    },
    Case {
        name: "failing events read",
        node: PRUNED,
        store: failing_store(&[READ_EVENTS]),
        read_mask: "transaction,events",
        expect: Expect::Error(tonic::Code::Unavailable),
        requests: &[LOOKUP, READ_BOTH, READ_EVENTS],
        ..CASE
    },
    Case {
        name: "store events that do not match the effects",
        node: PRUNED,
        store: store(Data::OtherEvents),
        read_mask: "transaction,events",
        expect: MISMATCH,
        requests: &[LOOKUP, READ_BOTH, READ_EVENTS],
        evicted: Evicted::Events,
        ..CASE
    },
    // Objects
    Case {
        name: "local objects: no object request",
        node: Node {
            objects: true,
            ..LOCAL
        },
        store: FULL_STORE,
        read_mask: "input_objects,output_objects",
        expect: SERVED,
        ..CASE
    },
    Case {
        name: "never pruned, objects missing locally: read from the store",
        node: EXECUTED,
        store: FULL_STORE,
        read_mask: "input_objects,output_objects",
        expect: SERVED,
        requests: &[READ_OBJECTS],
        ..CASE
    },
    Case {
        name: "object missing, no store",
        node: LOCAL,
        read_mask: "input_objects",
        expect: Expect::Error(tonic::Code::FailedPrecondition),
        ..CASE
    },
    Case {
        name: "object missing from the store",
        node: LOCAL,
        store: store(Data::Without(Part::Objects)),
        read_mask: "input_objects",
        expect: Expect::Error(tonic::Code::FailedPrecondition),
        requests: &[READ_OBJECTS],
        ..CASE
    },
    Case {
        name: "failing object read",
        node: LOCAL,
        store: failing_store(&[READ_OBJECTS]),
        read_mask: "input_objects",
        expect: Expect::Error(tonic::Code::Unavailable),
        requests: &[READ_OBJECTS],
        ..CASE
    },
    Case {
        name: "store input objects at the wrong version: only they are evicted",
        node: LOCAL,
        store: store(Data::WrongVersionInputs),
        read_mask: "input_objects,output_objects",
        expect: MISMATCH,
        requests: &[READ_OBJECTS],
        evicted: Evicted::InputObjects,
        ..CASE
    },
    Case {
        name: "store output object with other contents",
        node: LOCAL,
        store: store(Data::OtherOutputs),
        read_mask: "output_objects",
        expect: MISMATCH,
        requests: &[READ_OBJECTS],
        evicted: Evicted::OutputObjects,
        ..CASE
    },
    Case {
        name: "object changes only: objects are read",
        node: LOCAL,
        store: FULL_STORE,
        read_mask: "object_changes",
        expect: SERVED,
        requests: &[READ_OBJECTS],
        ..CASE
    },
    Case {
        name: "balance changes only: objects are read",
        node: LOCAL,
        store: FULL_STORE,
        read_mask: "balance_changes",
        expect: SERVED,
        requests: &[READ_OBJECTS],
        ..CASE
    },
    Case {
        name: "output objects only: input objects not read",
        node: LOCAL,
        store: store(Data::WrongVersionInputs),
        read_mask: "output_objects",
        expect: SERVED,
        requests: &[READ_OBJECTS],
        ..CASE
    },
];

#[tokio::test]
async fn read_path_cases() {
    let mut failed = Vec::new();
    // Each case runs in its own task so that one failing case does not hide
    // the others.
    for case in CASES {
        if tokio::spawn(run_case(*case)).await.is_err() {
            failed.push(case.name);
        }
    }
    assert!(failed.is_empty(), "failed cases: {failed:#?}");
}

async fn run_case(case: Case) {
    let name = case.name;
    let test = test_transaction(case.events);
    let digest = test.digest();
    let store = case.store.map(|store| store.build(&test));
    let recorded = store.as_ref().map(|store| {
        (
            store.requests.clone(),
            store.evicted_objects.clone(),
            store.evicted_events.clone(),
        )
    });
    let handle = start(case.node.state(&test), store).await;

    let results = get_transactions(&handle, &[digest], case.read_mask).await;

    assert_eq!(results.len(), 1, "{name}");
    let result = &results[0];
    match case.expect {
        Expect::Served { checkpoint } => {
            let executed = result
                .executed_transaction()
                .unwrap()
                .unwrap_or_else(|| panic!("{name}: {:?}", result.error_message()));
            assert_eq!(executed.checkpoint, checkpoint, "{name}");
            let served_events = executed
                .events
                .as_ref()
                .and_then(|events| events.events.as_ref())
                .is_some_and(|events| !events.events.is_empty());
            let served = [
                ("transaction", executed.transaction.is_some()),
                (
                    "signatures",
                    executed
                        .signatures
                        .as_ref()
                        .is_some_and(|signatures| !signatures.signatures.is_empty()),
                ),
                ("effects", executed.effects.is_some()),
                ("events", served_events || !case.events),
                (
                    "balance_changes",
                    executed
                        .balance_changes
                        .as_ref()
                        .is_some_and(|changes| !changes.balance_changes.is_empty()),
                ),
            ];
            for (field, served) in served {
                if case.read_mask.split(',').any(|masked| masked == field) {
                    assert!(served, "{name}: {field} not served");
                }
            }
            let timestamp_seconds = checkpoint
                .filter(|_| case.read_mask.contains("timestamp"))
                .map(|_| (CHECKPOINT_TIMESTAMP_MS / 1000) as i64);
            assert_eq!(
                executed
                    .timestamp
                    .as_ref()
                    .map(|timestamp| timestamp.seconds),
                timestamp_seconds,
                "{name}"
            );
            // Served objects are exactly the versions the effects name.
            if case.read_mask.contains("input_objects") {
                assert_eq!(
                    served_object_keys(&executed.input_objects),
                    object_key_bytes(&input_object_keys(&test.effects)),
                    "{name}"
                );
            }
            if case.read_mask.contains("output_objects") {
                assert_eq!(
                    served_object_keys(&executed.output_objects),
                    object_key_bytes(&output_object_keys(&test.effects)),
                    "{name}"
                );
            }
        }
        Expect::Error(code) => {
            assert_eq!(result.error_code(), Some(code as i32), "{name}: {result:?}");
            let message = result.error_message().unwrap();
            if code == tonic::Code::Unavailable {
                assert!(!message.contains(STORE_HOST), "{name}: {message}");
            }
            // With a store, a missing item may be a failed store read.
            if code == tonic::Code::FailedPrecondition {
                assert_eq!(
                    message.contains("retry, and if it stays unavailable"),
                    case.store.is_some(),
                    "{name}: {message}"
                );
            }
        }
        Expect::Internal(text) => {
            assert_eq!(
                result.error_code(),
                Some(tonic::Code::Internal as i32),
                "{name}: {result:?}"
            );
            let message = result.error_message().unwrap();
            assert!(message.contains(text), "{name}: {message}");
        }
    }

    let (expected_objects, expected_events) = match case.evicted {
        Evicted::Nothing => (vec![], vec![]),
        Evicted::InputObjects => (input_object_keys(&test.effects), vec![]),
        Evicted::OutputObjects => (output_object_keys(&test.effects), vec![]),
        Evicted::Events => (vec![], vec![digest]),
    };
    match recorded {
        Some((requests, evicted_objects, evicted_events)) => {
            assert_eq!(*requests.lock().unwrap(), case.requests, "{name}");
            assert_eq!(*evicted_objects.lock().unwrap(), expected_objects, "{name}");
            assert_eq!(*evicted_events.lock().unwrap(), expected_events, "{name}");
        }
        None => assert!(
            case.requests.is_empty(),
            "{name}: no store to record requests"
        ),
    }
}

// ---------------------------------------------------------------------------
// Objects and items of one request
// ---------------------------------------------------------------------------

#[tokio::test]
async fn local_and_store_input_objects_keep_the_effects_order() {
    let test = test_transaction(true);
    let keys = input_object_keys(&test.effects);
    // The local object comes first, so the position of the pruned one among
    // the objects read from the store differs from its index in the effects.
    let (local_key, pruned_key) = (keys[0], keys[1]);
    let local_object = test
        .input_objects
        .iter()
        .find(|object| object.as_inner().id() == local_key.0)
        .unwrap()
        .clone();
    let store = FULL_STORE.unwrap().build(&test);
    let requested_objects = store.requested_objects.clone();
    let state = MockGrpcStateReader {
        objects: HashMap::from([(local_key.0, local_object)]),
        ..LOCAL.state(&test)
    };
    let handle = start(state, Some(store)).await;

    let results = get_transactions(&handle, &[test.digest()], "input_objects").await;

    let executed = single_executed(&results);
    assert_eq!(
        served_object_keys(&executed.input_objects),
        object_key_bytes(&keys)
    );
    assert_eq!(*requested_objects.lock().unwrap(), vec![vec![pruned_key]]);
}

#[tokio::test]
async fn failing_store_only_fails_its_own_item() {
    let local = test_transaction(true);
    let objects_missing = test_transaction(true);
    let local_state = Node {
        objects: true,
        ..LOCAL
    }
    .state(&local);
    let objects_missing_state = LOCAL.state(&objects_missing);
    let state = MockGrpcStateReader {
        transactions: local_state
            .transactions
            .into_iter()
            .chain(objects_missing_state.transactions)
            .collect(),
        effects: local_state
            .effects
            .into_iter()
            .chain(objects_missing_state.effects)
            .collect(),
        object_versions: local_state.object_versions,
        ..Default::default()
    };
    let store = MockFallbackStore {
        failing: HashSet::from([READ_OBJECTS]),
        ..Default::default()
    };
    let handle = start(state, Some(store)).await;

    let results = get_transactions(
        &handle,
        &[local.digest(), objects_missing.digest()],
        "transaction,input_objects",
    )
    .await;

    assert_eq!(results.len(), 2);
    assert!(results[0].executed_transaction().unwrap().is_some());
    assert_eq!(
        results[1].error_code(),
        Some(tonic::Code::Unavailable as i32)
    );
    let message = results[1].error_message().unwrap();
    assert!(message.contains("key-value store unavailable"));
}

// ---------------------------------------------------------------------------
// Batched store reads
// ---------------------------------------------------------------------------

/// The state of a node that holds `node` of each of `tests`, and a store that holds all of them.
fn node_and_store(
    node: Node,
    tests: &[TestTransaction],
) -> (MockGrpcStateReader, MockFallbackStore) {
    let mut state =
        MockGrpcStateReader::default().with_lowest_available_checkpoint(node.lowest_available);
    let mut store = MockFallbackStore::default();
    for test in tests {
        let part = node.state(test);
        state.transactions.extend(part.transactions);
        state.effects.extend(part.effects);
        state.object_versions.extend(part.object_versions);
        let part = FULL_STORE.unwrap().build(test);
        store.transactions.extend(part.transactions);
        store.effects.extend(part.effects);
        store.events.extend(part.events);
        store
            .transaction_checkpoints
            .extend(part.transaction_checkpoints);
        store.checkpoint_summaries.extend(part.checkpoint_summaries);
        store.objects.extend(part.objects);
    }
    (state, store)
}

fn served_digest(result: &TransactionResult) -> Vec<u8> {
    result
        .executed_transaction()
        .unwrap()
        .unwrap_or_else(|| panic!("expected a transaction, got {result:?}"))
        .transaction
        .as_ref()
        .unwrap()
        .digest
        .as_ref()
        .unwrap()
        .digest
        .to_vec()
}

fn error_code(result: &TransactionResult) -> Option<tonic::Code> {
    result.error_code().map(tonic::Code::from)
}

#[tokio::test]
async fn pruned_transactions_of_a_request_share_one_call_for_each_kind_of_data() {
    let tests = (0..3).map(|_| test_transaction(true)).collect::<Vec<_>>();
    let digests = tests
        .iter()
        .map(TestTransaction::digest)
        .collect::<Vec<_>>();
    let (state, store) = node_and_store(PRUNED, &tests);
    let (requests, requested_objects, requested_checkpoints) = (
        store.requests.clone(),
        store.requested_objects.clone(),
        store.requested_checkpoints.clone(),
    );
    let handle = start(state, Some(store)).await;

    let results = get_transactions(
        &handle,
        &digests,
        "transaction,effects,events,timestamp,input_objects,output_objects",
    )
    .await;

    let served = results.iter().map(served_digest).collect::<Vec<_>>();
    let expected = digests
        .iter()
        .map(|digest| digest.bytes().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(served, expected);
    assert_eq!(
        *requests.lock().unwrap(),
        [LOOKUP, READ_BOTH, READ_SUMMARY, READ_EVENTS, READ_OBJECTS]
    );
    // The transactions share their checkpoint, which is read once.
    assert_eq!(*requested_checkpoints.lock().unwrap(), [[CHECKPOINT_SEQ]]);
    let object_count = tests
        .iter()
        .map(|test| {
            input_object_keys(&test.effects).len() + output_object_keys(&test.effects).len()
        })
        .sum::<usize>();
    let requested_objects = requested_objects.lock().unwrap();
    assert_eq!(requested_objects.len(), 1);
    assert_eq!(requested_objects[0].len(), object_count);
}

#[tokio::test]
async fn results_keep_the_request_order_with_local_unknown_and_pruned_transactions() {
    let (pruned, local) = (
        [test_transaction(false), test_transaction(false)],
        test_transaction(false),
    );
    let (mut state, store) = node_and_store(PRUNED, &pruned);
    let local_state = LOCAL.state(&local);
    state.transactions.extend(local_state.transactions);
    state.effects.extend(local_state.effects);
    let (requests, requested_digests) = (store.requests.clone(), store.requested_digests.clone());
    let handle = start(state, Some(store)).await;
    let unknown = TransactionDigest::random();
    let digests = [
        pruned[0].digest(),
        local.digest(),
        unknown,
        pruned[1].digest(),
    ];

    let results = get_transactions(&handle, &digests, "transaction,effects").await;

    assert_eq!(results.len(), digests.len());
    for index in [0, 1, 3] {
        assert_eq!(
            served_digest(&results[index]),
            digests[index].bytes().to_vec()
        );
    }
    assert_eq!(error_code(&results[2]), Some(tonic::Code::NotFound));
    assert_eq!(*requests.lock().unwrap(), [LOOKUP, READ_BOTH]);
    // Only the transactions the node lacks are looked up, and only the pruned ones are read.
    let pruned = [pruned[0].digest(), pruned[1].digest()];
    assert_eq!(
        *requested_digests.lock().unwrap(),
        [
            vec![pruned[0], unknown, pruned[1]],
            [pruned, pruned].concat()
        ]
    );
}

#[tokio::test]
async fn a_transaction_requested_twice_shares_its_summary_and_object_reads() {
    let test = test_transaction(false);
    let (state, store) = node_and_store(PRUNED, std::slice::from_ref(&test));
    let (requests, requested_objects, requested_checkpoints) = (
        store.requests.clone(),
        store.requested_objects.clone(),
        store.requested_checkpoints.clone(),
    );
    let handle = start(state, Some(store)).await;

    let results = get_transactions(
        &handle,
        &[test.digest(), test.digest()],
        "timestamp,input_objects",
    )
    .await;

    assert_eq!(results.len(), 2);
    for result in &results {
        assert!(
            result.executed_transaction().unwrap().is_some(),
            "{result:?}"
        );
    }
    assert_eq!(
        *requests.lock().unwrap(),
        [LOOKUP, "multi_get(effects)", READ_SUMMARY, READ_OBJECTS]
    );
    assert_eq!(*requested_checkpoints.lock().unwrap(), [[CHECKPOINT_SEQ]]);
    let mut input_keys = input_object_keys(&test.effects);
    input_keys.sort();
    assert_eq!(*requested_objects.lock().unwrap(), [input_keys]);
}

#[tokio::test]
async fn each_chunk_of_50_transactions_makes_its_own_calls() {
    let digests = (0..51)
        .map(|_| TransactionDigest::random())
        .collect::<Vec<_>>();
    let store = MockFallbackStore::default();
    let (requests, requested_digests) = (store.requests.clone(), store.requested_digests.clone());
    let handle = start(PRUNED.state(&test_transaction(false)), Some(store)).await;

    let results = get_transactions(&handle, &digests, "transaction,effects").await;

    assert_eq!(results.len(), digests.len());
    for result in &results {
        assert_eq!(error_code(result), Some(tonic::Code::NotFound));
    }
    assert_eq!(*requests.lock().unwrap(), [LOOKUP, LOOKUP]);
    assert_eq!(
        *requested_digests.lock().unwrap(),
        [digests[..50].to_vec(), digests[50..].to_vec()]
    );
}

#[tokio::test]
async fn a_failed_call_fails_every_transaction_that_needed_it() {
    let tests = (0..3).map(|_| test_transaction(false)).collect::<Vec<_>>();
    let mut state = MockGrpcStateReader::default();
    for (index, test) in tests.iter().enumerate() {
        let node = Node {
            // Only the first transaction's objects are on the node.
            objects: index == 0,
            ..LOCAL
        };
        let part = node.state(test);
        state.transactions.extend(part.transactions);
        state.effects.extend(part.effects);
        state.object_versions.extend(part.object_versions);
    }
    let store = MockFallbackStore {
        failing: HashSet::from([READ_OBJECTS]),
        ..Default::default()
    };
    let requests = store.requests.clone();
    let handle = start(state, Some(store)).await;
    let digests = tests
        .iter()
        .map(TestTransaction::digest)
        .collect::<Vec<_>>();

    let results = get_transactions(&handle, &digests, "transaction,input_objects").await;

    assert_eq!(served_digest(&results[0]), digests[0].bytes().to_vec());
    assert_eq!(error_code(&results[1]), Some(tonic::Code::Unavailable));
    assert_eq!(error_code(&results[2]), Some(tonic::Code::Unavailable));
    assert_eq!(*requests.lock().unwrap(), [READ_OBJECTS]);
}

#[tokio::test(start_paused = true)]
async fn lookups_after_the_budget_is_spent_are_unavailable() {
    // Each lookup times out after 10 s, so the third chunk's lookup spends the
    // last of the 30 s budget.
    let digests = (0..101)
        .map(|_| TransactionDigest::random())
        .collect::<Vec<_>>();
    let store = MockFallbackStore {
        lookup_delay: Some(std::time::Duration::from_secs(60)),
        ..Default::default()
    };
    let handle = start(PRUNED.state(&test_transaction(false)), Some(store)).await;

    let results = get_transactions(&handle, &digests, "transaction,effects").await;

    let codes = results.iter().map(error_code).collect::<Vec<_>>();
    let mut expected = vec![Some(tonic::Code::NotFound); 100];
    expected.push(Some(tonic::Code::Unavailable));
    assert_eq!(codes, expected);
}

#[tokio::test]
async fn a_failed_transaction_or_events_call_fails_only_the_transactions_that_needed_it() {
    // Transactions: the local one is served, the pruned one needs the failing call.
    let (local, pruned) = (test_transaction(false), test_transaction(false));
    let (mut state, mut store) = node_and_store(PRUNED, std::slice::from_ref(&pruned));
    let local_state = LOCAL.state(&local);
    state.transactions.extend(local_state.transactions);
    state.effects.extend(local_state.effects);
    store.failing = HashSet::from(["multi_get"]);
    let handle = start(state, Some(store)).await;

    let results = get_transactions(
        &handle,
        &[local.digest(), pruned.digest()],
        "transaction,effects",
    )
    .await;

    assert_eq!(served_digest(&results[0]), local.digest().bytes().to_vec());
    assert_eq!(error_code(&results[1]), Some(tonic::Code::Unavailable));

    // Events: the node holds the events of the first transaction only.
    let tests = [test_transaction(true), test_transaction(true)];
    let mut state = MockGrpcStateReader::default();
    for (index, test) in tests.iter().enumerate() {
        let part = Node {
            events: index == 0,
            ..LOCAL
        }
        .state(test);
        state.transactions.extend(part.transactions);
        state.effects.extend(part.effects);
        state.events.extend(part.events);
    }
    let store = MockFallbackStore {
        failing: HashSet::from([READ_EVENTS]),
        ..Default::default()
    };
    let handle = start(state, Some(store)).await;

    let results = get_transactions(
        &handle,
        &[tests[0].digest(), tests[1].digest()],
        "transaction,events",
    )
    .await;

    assert_eq!(
        served_digest(&results[0]),
        tests[0].digest().bytes().to_vec()
    );
    assert_eq!(error_code(&results[1]), Some(tonic::Code::Unavailable));
}

#[tokio::test]
async fn only_the_events_that_do_not_match_are_evicted() {
    let tests = [test_transaction(true), test_transaction(true)];
    let (state, mut store) = node_and_store(PRUNED, &tests);
    store
        .events
        .insert(tests[1].digest(), TransactionEvents(vec![]));
    let evicted_events = store.evicted_events.clone();
    let handle = start(state, Some(store)).await;
    let digests = [tests[0].digest(), tests[1].digest()];

    let results = get_transactions(&handle, &digests, "transaction,events").await;

    assert_eq!(served_digest(&results[0]), digests[0].bytes().to_vec());
    assert_eq!(error_code(&results[1]), Some(tonic::Code::Internal));
    assert_eq!(*evicted_events.lock().unwrap(), [digests[1]]);
}

#[tokio::test]
async fn a_wrong_shared_object_fails_every_transaction_that_reads_it() {
    let test = test_transaction(false);
    let store = store(Data::WrongVersionInputs).unwrap().build(&test);
    let evicted_objects = store.evicted_objects.clone();
    let handle = start(LOCAL.state(&test), Some(store)).await;

    let results = get_transactions(&handle, &[test.digest(), test.digest()], "input_objects").await;

    for result in &results {
        assert_eq!(error_code(result), Some(tonic::Code::Internal));
    }
    // Each rejected object is evicted once.
    let mut input_keys = input_object_keys(&test.effects);
    input_keys.sort();
    assert_eq!(*evicted_objects.lock().unwrap(), input_keys);
}
