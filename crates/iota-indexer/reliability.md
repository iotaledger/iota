The indexer collects internal metrics, which are exposed through Prometheus. Based on these collected metrics, the following table provides thresholds for quantitative requirements for Indexer monitoring.

### Metrics

The following indexer metrics will be used for quantitative requirements.

| **Metric**                                               | **Description**                                                                                                     |
| -------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `latest_fullnode_checkpoint_sequence_number`             | Latest checkpoint from full node ready to be synced by the indexer.                                                 |
| `max_committed_checkpoint_sequence_number`               | The highest committed checkpoint by the indexer.                                                                    |
| `checkpoint_db_commit_latency`                           | Total latency to commit a chunk of checkpoints (including all transactions, objects, events, etc.) to the database. |
| `checkpoint_db_commit_latency_transactions_chunks`       | Latency to commit a single chunk of checkpoint transactions to the database.                                        |
| `checkpoint_db_commit_latency_tx_insertion_order_chunks` | Latency to commit a single chunk of data representing the transaction insertion order to the database.              |
| `checkpoint_db_commit_latency_tx_indices_chunks`         | Latency to commit a single chunk of data representing the transaction indices to the database.                      |
| `checkpoint_db_commit_latency_events_chunks`             | Latency to commit a single chunk of data representing the events to the database.                                   |
| `checkpoint_db_commit_latency_event_indices_chunks`      | Latency to commit a single chunk of data representing the events indices to the database.                           |
| `checkpoint_db_commit_latency_packages`                  | Latency to commit a single chunk of data representing the packages to the database.                                 |
| `checkpoint_db_commit_latency_objects_chunks`            | Latency to commit a single chunk of data representing the objects to the database.                                  |
| `checkpoint_db_commit_latency_objects_history_chunks`    | Latency to commit a single chunk of data representing the object history to the database.                           |
| `checkpoint_db_commit_latency_objects_snapshot_chunks`   | Latency to commit a single chunk of data representing the object snapshot to the database.                          |
| `checkpoint_db_commit_latency_objects_snapshot`          | Total latency to commit a batch of checkpoints object snapshots to the database.                                    |
| `checkpoint_db_commit_latency_epoch`                     | Latency to commit the epoch data into database.                                                                     |
| `req_latency_by_route`                                   | Latency of indexer JSON RPC API endpoints. Measures the time from request receipt to response delivery.             |

> [!Note]
>
> - Large datasets (e.g., transactions, objects) are split into smaller **chunks** to avoid overwhelming the database with bulk writes. While most chunks contain 100 elements, object snapshot process uses a larger chunk size of 500 elements.
> - Metrics ending with `_chunks` track latency **per individual chunk**, not the entire dataset of the checkpoint being committed.
> - Example: `checkpoint_db_commit_latency_transactions_chunks` measures the time to commit **one transaction chunk**, not all transactions in a checkpoint.

### Quantitative Requirements:

| **Requirement**                                                        | **Metrics**                                                                                                                                        | **Permissible Values**                                               |
| ---------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| **1. Checkpoint Sync Lag**                                             | Time in `min` where `lag > 0`. <br><br>`lag` is defined as `latest_fullnode_checkpoint_sequence_number - max_committed_checkpoint_sequence_number` | - **Genesis**: `≤ 35 min` <br><br>- **Post Genesis**: `≤ 5 min`      |
| **2. Checkpoints batch commit latency**                                | `checkpoint_db_commit_latency`                                                                                                                     | - **Genesis**: `≤ 35 min` <br><br>- **Post Genesis**: `≤ 1 min`      |
| **3. Checkpoint chunked transactions commit latency**                  | `checkpoint_db_commit_latency_transactions_chunks`                                                                                                 | - **Genesis**: `≤ 5 min` <br><br>- **Post Genesis**: `≤ 5 sec`       |
| **4. Checkpoint chunked transactions insertion orders commit latency** | `checkpoint_db_commit_latency_tx_insertion_order_chunks`                                                                                           | `≤ 10 sec`                                                           |
| **5. Checkpoint chunked transaction indices commit latency**           | `checkpoint_db_commit_latency_tx_indices_chunks`                                                                                                   | - **Genesis**: `≤ 10 min` <br><br>- **Post Genesis**: `≤ 10 sec`     |
| **6. Checkpoint chunked events commit latency**                        | `checkpoint_db_commit_latency_events_chunks`                                                                                                       | `≤ 5 sec`                                                            |
| **7. Checkpoint chunked events indices commit latency**                | `checkpoint_db_commit_latency_event_indices_chunks`                                                                                                | `≤ 5 sec`                                                            |
| **8. Checkpoint chunked packages commit latency**                      | `checkpoint_db_commit_latency_packages`                                                                                                            | `≤ 5 sec`                                                            |
| **9. Checkpoint chunked objects commit latency**                       | `checkpoint_db_commit_latency_objects_chunks`                                                                                                      | - **Genesis**: `≤ 1 min` <br><br>- **Post Genesis**: `≤ 5 sec`       |
| **10. Checkpoint chunked objects history commit latency**              | `checkpoint_db_commit_latency_objects_history_chunks`                                                                                              | - **Genesis**: `≤ 1 min` <br><br>- **Post Genesis**: `≤ 5 sec`       |
| **11. Epoch commit latency**                                           | `checkpoint_db_commit_latency_epoch`                                                                                                               | `≤ 5 sec`                                                            |
| **12. Object snapshot chunks commit latency**                          | `checkpoint_db_commit_latency_objects_snapshot_chunks`                                                                                             | - **Genesis**: `≤ 3 min` <br><br>- **Post Genesis**: `≤ 5 sec`       |
| **13. Object snapshot commit latency**                                 | `checkpoint_db_commit_latency_objects_snapshot`                                                                                                    | - **Genesis**: `≤ 60 min` <br><br>- **Post Genesis**: `≤ 5 sec`      |
| **14. DB Connection Pool Size**                                        | Time in `sec` where `pool_size = 0`.<br><br>`pool_size` is defined as `db_conn_pool_size`                                                          | - **Genesis sync**: `≤ 10 sec` <br><br>- **Post-genesis**: `≤ 5 sec` |
| 15. **JSON RPC latency**                                               | `req_latency_by_route`                                                                                                                             | `≤ 1 second`                                                         |

### Qualitative Requirements:

1. The indexer should be able to restart in a matter of seconds.
2. The indexer must save checkpoints in order, if a checkpoints fails to sync it should not sync subsequent checkpoints.
