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
    ].join('\n');

    const restore = joinArgs([
        `iota-indexer --db-url "$DATABASE_URL" restore --network ${network} run`,
        '  --staging-path /tmp/indexer-restore',
        '  --genesis-path "$GENESIS_PATH"',
        ...(epoch === 'latest' ? [] : [`  --epoch ${epoch}`]),
    ]);

    return [
        env,
        '',
        '# Bootstrap the database from the formal snapshot.',
        restore,
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
