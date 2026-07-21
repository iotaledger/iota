import React from 'react';
import SnapshotEpochPicker from '@site/src/components/SnapshotEpochPicker';
import { EpochSelection, Network } from '@site/src/hooks/useFormalSnapshotEpochs';

const NETWORKS = ['mainnet', 'testnet', 'devnet'] as const;

/** Joins CLI argument lines into a shell command with line continuations. */
const joinArgs = (args: string[]) => args.join(' \\\n');

function buildCommand(network: Network, epoch: EpochSelection): string {
    const env = [
        'DATABASE_URL="postgres://iota_indexer:iota_indexer@localhost:5432/iota_indexer"',
        'GENESIS_PATH="/path/to/genesis.blob"',
        'PRUNING_TOML_PATH="/path/to/pruning.toml"',
    ].join('\n');

    const restore = joinArgs([
        `iota-indexer --db-url "$DATABASE_URL" restore --network ${network} run`,
        '  --staging-path /tmp/indexer-restore',
        '  --genesis-path "$GENESIS_PATH"',
        ...(epoch === 'latest' ? [] : [`  --epoch ${epoch}`]),
    ]);

    const catchUp = joinArgs([
        'iota-indexer --db-url "$DATABASE_URL" indexer',
        `  --remote-store-url "https://checkpoints.${network}.iota.cafe/ingestion/historical"`,
        `  --live-checkpoints-store-url "https://checkpoints.${network}.iota.cafe/ingestion/live"`,
        '  --pruning-config-path "$PRUNING_TOML_PATH"',
    ]);

    return [
        env,
        '',
        '# 1. Bootstrap the database from the formal snapshot.',
        restore,
        '',
        '# 2. Catch up to the chain tip, with pruning enabled.',
        catchUp,
    ].join('\n');
}

export default function IndexerRestorePicker() {
    return (
        <SnapshotEpochPicker
            networks={NETWORKS}
            fallback="Loading restore picker…"
            buildCommand={buildCommand}
            requireV2
        />
    );
}
