import React, { useEffect, useState } from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import CodeBlock from '@theme/CodeBlock';

const NETWORKS = ['mainnet', 'testnet'] as const;
type Network = (typeof NETWORKS)[number];

const SETUPS = ['binary', 'docker'] as const;
type Setup = (typeof SETUPS)[number];

type EpochSelection = number | 'latest';

// Supports both the legacy schema (`available_epochs: number[]`) and the
// current one (`available_epochs: [epoch, startTimestampMs | null][]`).
interface Manifest {
    available_epochs: Array<number | [number, number | null]>;
}

function normalizeEpochs(
    raw: Manifest['available_epochs'] | undefined,
): Array<[number, number | null]> {
    return (raw ?? []).map((entry) =>
        Array.isArray(entry) ? entry : [entry, null],
    );
}

function formatTimestamp(ms: number): string {
    const d = new Date(ms);
    if (Number.isNaN(d.getTime())) return '';
    return `${d.toISOString().slice(0, 16).replace('T', ' ')} UTC`;
}

const manifestUrl = (network: Network) =>
    `https://formal-snapshot.${network}.iota.cafe/MANIFEST`;

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
            '  --num-parallel-downloads 50 \\',
            '  --verify normal \\',
            '  --no-sign-request \\',
            '  --verbose',
        ].join('\n');
    }

    return [
        'docker run --rm \\',
        '  -v "<PATH-TO-NODE-DB>":/opt/iota/db \\',
        '  -v "<PATH-TO-GENESIS-BLOB>":/opt/iota/config/genesis.blob \\',
        `  iotaledger/iota-tools:${network} \\`,
        '  /bin/sh -c "/usr/local/bin/iota-tool download-formal-snapshot \\',
        `    ${epochArg} \\`,
        '    --genesis /opt/iota/config/genesis.blob \\',
        '    --path /opt/iota/db/authorities_db \\',
        '    --num-parallel-downloads 50 \\',
        '    --verify normal \\',
        '    --no-sign-request \\',
        `    --network ${network} \\`,
        '    --verbose"',
    ].join('\n');
}

function Picker() {
    const [setup, setSetup] = useState<Setup>('binary');
    const [network, setNetwork] = useState<Network>('mainnet');
    const [epochs, setEpochs] = useState<Array<[number, number | null]>>([]);
    const [epoch, setEpoch] = useState<EpochSelection>('latest');
    const [loading, setLoading] = useState<boolean>(false);
    const [error, setError] = useState<string | null>(null);

    useEffect(() => {
        let cancelled = false;
        setLoading(true);
        setError(null);
        setEpochs([]);

        fetch(manifestUrl(network))
            .then((r) => {
                if (!r.ok) throw new Error(`HTTP ${r.status}`);
                return r.json();
            })
            .then((m: Manifest) => {
                if (cancelled) return;
                const sorted = normalizeEpochs(m.available_epochs).sort(
                    ([a], [b]) => b - a,
                );
                setEpochs(sorted);
            })
            .catch((e) => {
                if (!cancelled) setError(String(e?.message ?? e));
            })
            .finally(() => {
                if (!cancelled) setLoading(false);
            });

        return () => {
            cancelled = true;
        };
    }, [network]);

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
                        onChange={(e) => {
                            setNetwork(e.target.value as Network);
                            setEpoch('latest');
                        }}
                    >
                        {NETWORKS.map((n) => (
                            <option key={n} value={n}>
                                {n}
                            </option>
                        ))}
                    </select>
                </label>

                <label style={fieldStyle}>
                    <span>Snapshot epoch</span>
                    <select
                        style={selectStyle}
                        value={String(epoch)}
                        disabled={loading}
                        onChange={(e) => {
                            const v = e.target.value;
                            setEpoch(v === 'latest' ? 'latest' : Number(v));
                        }}
                    >
                        <option value="latest">
                            {loading ? 'Loading…' : 'Latest'}
                        </option>
                        {epochs.map(([ep, ts]) => {
                            const label = ts
                                ? `${ep} — started ${formatTimestamp(ts)}`
                                : String(ep);
                            return (
                                <option key={ep} value={ep}>
                                    {label}
                                </option>
                            );
                        })}
                    </select>
                </label>
            </div>

            {error && (
                <div
                    style={{
                        color: 'var(--ifm-color-danger)',
                        marginBottom: '0.75rem',
                        fontSize: '0.9rem',
                    }}
                >
                    Could not load available epochs from {manifestUrl(network)}{' '}
                    ({error}). You can still use the <code>--latest</code>{' '}
                    option below.
                </div>
            )}

            <CodeBlock language="bash">
                {buildCommand(setup, network, epoch)}
            </CodeBlock>
        </div>
    );
}

export default function FormalSnapshotPicker() {
    return (
        <BrowserOnly fallback={<div>Loading snapshot picker…</div>}>
            {() => <Picker />}
        </BrowserOnly>
    );
}
