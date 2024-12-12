IOTA Analytical Indexer
=======================

The IOTA Analytical Indexer is a service that exports data from the main IOTA network to a remote big object store (S3/GCS/Azure) for further analytical processing. It does not perform any analysis on its own.

**Key Features**
----------------

* Exports data from the IOTA network to a remote big object store
* Provides BigQuery and Snowflake schemas for the exported data

**Relation to iota-indexer**
----------------------------

The data exported by the iota-analytics-indexer can be later analysed by some other solutions outside of the crate.
Functionality from iota-indexer that is not required to serve user JSON RPC/GraphQL requests could potentially be moved away from iota-indexer and served based on data exported by the iota-analytics-indexer.

**Architecture**
----------------

When running the indexer, one needs to specify object type that would be extracted from checkpoints and uploaded to the cloud.

The following object types are supported:
- Checkpoint
- Object
- Transaction
- TransactionObjects
- Event
- MoveCall
- MovePackage
- DynamicField
- WrappedObject

Only one object type can be passed in given run, to process multiple object types it is needed to run multiple analytical indexer instances.

In general, the data flow is as follows:

* Checkpoints are read via JSON RPC using reused code from `iota_data_ingestion_core`.
* Checkpoints are processed by an appropriate handler (e.g. `EventHandler`), which extracts relevant objects from each transaction of the checkpoint.
* Objects are passed to the Writer, which writes the objects to a local temporary store in CSV or Parquet format.
* The `AnalyticsProcessor` syncs the objects from the local store to the remote store (S3/GCS/Azure, or also local, for testing purposes).
* Every 5 minutes the last processed checkpoint ID is fetched from BigQuery/Snowflake and reported as a metric.

**Note:** It is assumed that data from the big object store will be readable from BigQuery/Snowflake automatically, the indexer is not putting the data in BigQuery/Snowflake tables explicitly.

Here is a graph summarizing the data flow:

```mermaid
---
config:
  look: handDrawn
  theme: neutral
---
flowchart TD
    FNODE["Fullnode/Indexer"] <-->|JSON RPC| CPREADER["`IndexerExecutor/CheckpointReader from the **iota_data_ingestion_core** package`"];
    subgraph "`**iota-analytical-indexer**`"
        CPREADER -->|"`Executor calls **AnalyticsProcessor** for each checkpoint, which in turn passes the checkpoint to appropriate Handler`"| HANDLER["CheckpointHandler/EventHandler etc., depending on indexer configuration"]
        HANDLER -->|"`**AnalyticsProcessor** reads object data extracted from the checkpoint by the Handler and passes it to the Writer`"| WRITER["CSVWriter/ParquetWriter"]
        WRITER -->|Writes objects to temporary local storage| DISK[Temporary Local Storage]
        DISK --> REMOTESYNC["`Task inside of **AnalyticsProcessor** that removes files from Local Storage and uploads them to Remote Storage(S3/GCS/Azure)`"]
        WRITER -->|"`Once every few checkpoints, **AnalyticsProcessor** calls cut() to prepare file to be sent, FileMetadata is sent to the Remote Sync Task which triggers the sync`"| REMOTESYNC
        REMOTESYNC -->|Some process outside of analytical indexer makes the newly uploaded data available via BigQuery/Snowflake tables| BQSF["BigQuery/Snowflake"]
        BQSF -->|"Every 5 minutes max processed checkpoint number is read from the tables"| METRICS[Analytics Indexer Prometheus Metrics]
    end

linkStyle 6 color:red;
```

**Metrics**
-----------

 - **total_received**: count of checkpoints processed in given run
 - **last_uploaded_checkpoint**: id of last checkpoint uploaded to the big object store
 - **max_checkpoint_on_store**: id of last checkpoint available via BigQuery/Snowflake tables
