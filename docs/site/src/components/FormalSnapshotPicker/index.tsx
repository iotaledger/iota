import React, { useState } from 'react';
import SnapshotEpochPicker, {
    PickerField,
    selectStyle,
} from '@site/src/components/SnapshotEpochPicker';
import { EpochSelection, Network } from '@site/src/hooks/useFormalSnapshotEpochs';

const NETWORKS = ['mainnet', 'testnet', 'devnet'] as const;

const SETUPS = ['binary', 'docker'] as const;
type Setup = (typeof SETUPS)[number];

function buildCommand(
    setup: Setup,
    network: Network,
    epoch: EpochSelection,
): string {
    const epochArg = epoch === 'latest' ? '--latest' : `--epoch ${epoch}`;

    if (setup === 'binary') {
        return [
            'iota-tool download-formal-snapshot \\',
            `  ${epochArg} \\`,
            '  --genesis "<PATH-TO-GENESIS-BLOB>" \\',
            `  --network ${network} \\`,
            '  --path "<PATH-TO-NODE-DB>" \\',
            '  --verify normal \\',
            '  --no-sign-request \\',
            '  --verbose',
        ].join('\n');
    }

    return [
        'docker run --rm \\',
        '  -v "$PWD/data/db":/opt/iota/db \\',
        '  -v "$PWD/data/config/genesis.blob":/opt/iota/config/genesis.blob \\',
        `  iotaledger/iota-tools:${network} \\`,
        '  /bin/sh -c "/usr/local/bin/iota-tool download-formal-snapshot \\',
        `    ${epochArg} \\`,
        '    --genesis /opt/iota/config/genesis.blob \\',
        '    --path /opt/iota/db/authorities_db \\',
        '    --verify normal \\',
        '    --no-sign-request \\',
        `    --network ${network} \\`,
        '    --verbose"',
    ].join('\n');
}

export default function FormalSnapshotPicker() {
    const [setup, setSetup] = useState<Setup>('binary');
    return (
        <SnapshotEpochPicker
            networks={NETWORKS}
            fallback="Loading snapshot picker…"
            extraFields={
                <PickerField label="Setup type">
                    <select
                        style={selectStyle}
                        value={setup}
                        onChange={(e) => setSetup(e.target.value as Setup)}
                    >
                        <option value="binary">Binary</option>
                        <option value="docker">Docker</option>
                    </select>
                </PickerField>
            }
            buildCommand={(network, epoch) => buildCommand(setup, network, epoch)}
        />
    );
}
