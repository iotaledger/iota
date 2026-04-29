import React, { useState } from 'react';
import CodeBlock from '@theme/CodeBlock';

const NETWORKS = ['mainnet', 'testnet'] as const;
type Network = (typeof NETWORKS)[number];

const SETUPS = ['binary', 'docker'] as const;
type Setup = (typeof SETUPS)[number];

function buildCommand(setup: Setup, network: Network): string {
    if (setup === 'binary') {
        return [
            'iota-tool download-db-snapshot \\',
            '  --latest \\',
            `  --network ${network} \\`,
            '  --path "<PATH-TO-NODE-DB>" \\',
            '  --num-parallel-downloads 25 \\',
            '  --skip-indexes \\',
            '  --no-sign-request \\',
            '  --verbose',
        ].join('\n');
    }

    return [
        'docker run --rm \\',
        '  -v "<PATH-TO-NODE-DB>":/opt/iota/db \\',
        `  iotaledger/iota-tools:${network} \\`,
        '  /bin/sh -c "/usr/local/bin/iota-tool download-db-snapshot \\',
        '    --latest \\',
        `    --network ${network} \\`,
        '    --path /opt/iota/db \\',
        '    --num-parallel-downloads 25 \\',
        '    --skip-indexes \\',
        '    --no-sign-request \\',
        '    --verbose"',
    ].join('\n');
}

export default function RocksDBSnapshotPicker() {
    const [setup, setSetup] = useState<Setup>('binary');
    const [network, setNetwork] = useState<Network>('mainnet');

    const selectStyle: React.CSSProperties = {
        padding: '0.35rem 0.5rem',
        borderRadius: '4px',
        border: '1px solid var(--ifm-color-emphasis-300)',
        background: 'var(--ifm-background-color)',
        color: 'var(--ifm-font-color-base)',
    };

    const fieldStyle: React.CSSProperties = {
        display: 'flex',
        flexDirection: 'column',
        gap: '0.25rem',
        fontSize: '0.9rem',
    };

    return (
        <div
            style={{
                border: '1px solid var(--ifm-color-emphasis-200)',
                borderRadius: '6px',
                padding: '1rem',
                marginBottom: '1rem',
            }}
        >
            <div
                style={{
                    display: 'flex',
                    flexWrap: 'wrap',
                    gap: '1rem',
                    marginBottom: '1rem',
                }}
            >
                <label style={fieldStyle}>
                    <span>Setup type</span>
                    <select
                        style={selectStyle}
                        value={setup}
                        onChange={(e) => setSetup(e.target.value as Setup)}
                    >
                        <option value="binary">Binary</option>
                        <option value="docker">Docker</option>
                    </select>
                </label>

                <label style={fieldStyle}>
                    <span>Network</span>
                    <select
                        style={selectStyle}
                        value={network}
                        onChange={(e) => setNetwork(e.target.value as Network)}
                    >
                        {NETWORKS.map((n) => (
                            <option key={n} value={n}>
                                {n}
                            </option>
                        ))}
                    </select>
                </label>
            </div>

            <CodeBlock language="bash">
                {buildCommand(setup, network)}
            </CodeBlock>
        </div>
    );
}
