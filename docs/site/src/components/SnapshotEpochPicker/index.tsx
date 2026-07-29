import React, { useState } from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import CodeBlock from '@theme/CodeBlock';
import {
    EpochSelection,
    Network,
    formatTimestamp,
    manifestUrl,
    useFormalSnapshotEpochs,
} from '@site/src/hooks/useFormalSnapshotEpochs';

export const selectStyle: React.CSSProperties = {
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

/** A labeled control, styled to match the picker's other fields. */
export function PickerField({
    label,
    children,
}: {
    label: string;
    children: React.ReactNode;
}) {
    return (
        <label style={fieldStyle}>
            <span>{label}</span>
            {children}
        </label>
    );
}

export interface SnapshotEpochPickerProps {
    /** Networks offered in the dropdown; the first is the default. */
    networks: readonly Network[];
    /** Builds the command shown for the selected network and epoch. */
    buildCommand: (network: Network, epoch: EpochSelection) => string;
    /** Extra controls rendered before the network select; own their own state. */
    extraFields?: React.ReactNode;
    /** Offer only V2 snapshots (those with an epoch-end timestamp). */
    requireV2?: boolean;
    /** Fallback text shown while the browser-only picker loads. */
    fallback?: string;
}

function Picker({
    networks,
    buildCommand,
    extraFields,
    requireV2,
}: SnapshotEpochPickerProps) {
    const [network, setNetwork] = useState<Network>(networks[0]);
    const [epoch, setEpoch] = useState<EpochSelection>('latest');
    const { epochs, loading, error } = useFormalSnapshotEpochs(network);

    const latestEpoch = epochs.length ? epochs[0][0] : null;

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
                {extraFields}

                <PickerField label="Network">
                    <select
                        style={selectStyle}
                        value={network}
                        onChange={(e) => {
                            setNetwork(e.target.value as Network);
                            setEpoch('latest');
                        }}
                    >
                        {networks.map((n) => (
                            <option key={n} value={n}>
                                {n}
                            </option>
                        ))}
                    </select>
                </PickerField>

                <PickerField label="Snapshot epoch">
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
                            // V1 snapshots have no usable timestamp; skip them
                            // when only V2 is wanted.
                            const date = formatTimestamp(ts);
                            if (requireV2 && !date) return null;
                            const label = date
                                ? `${ep} — ended at ${date}${
                                    ep === latestEpoch ? ' [latest]' : ''
                                }`
                                : String(ep);
                            return (
                                <option key={ep} value={ep}>
                                    {label}
                                </option>
                            );
                        })}
                    </select>
                </PickerField>
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
                    ({error}). You can still generate a command for the latest
                    snapshot below.
                </div>
            )}

            <CodeBlock language="bash">{buildCommand(network, epoch)}</CodeBlock>
        </div>
    );
}

export default function SnapshotEpochPicker(props: SnapshotEpochPickerProps) {
    return (
        <BrowserOnly fallback={<div>{props.fallback ?? 'Loading picker…'}</div>}>
            {() => <Picker {...props} />}
        </BrowserOnly>
    );
}
