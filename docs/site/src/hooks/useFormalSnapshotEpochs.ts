import { useEffect, useState } from 'react';

// Supports both the legacy schema (`available_epochs: number[]`) and the
// current one (`available_epochs: [epoch, endTimestampMs | null][]`).
interface Manifest {
    available_epochs: Array<number | [number, number | null]>;
}

/** Networks that publish a formal snapshot. */
export type Network = 'mainnet' | 'testnet' | 'devnet';

/** A selected snapshot epoch, or the latest available one. */
export type EpochSelection = number | 'latest';

/** An available snapshot epoch and the epoch-end timestamp (ms), if known. */
export type EpochEntry = [number, number | null];

/**
 * Formats an epoch-end timestamp (ms) as `YYYY-MM-DD HH:MM UTC`, or `''` when
 * there is no usable timestamp. A missing (`null`) or `0` timestamp marks a V1
 * snapshot; a non-empty result marks V2.
 */
export function formatTimestamp(ms: number | null): string {
    if (!ms) return '';
    const d = new Date(ms);
    if (Number.isNaN(d.getTime())) return '';
    return `${d.toISOString().slice(0, 16).replace('T', ' ')} UTC`;
}

/** URL of the formal snapshot MANIFEST for a network. */
export const manifestUrl = (network: Network) =>
    `https://formal-snapshot.${network}.iota.cafe/MANIFEST`;

function normalizeEpochs(
    raw: Manifest['available_epochs'] | undefined,
): EpochEntry[] {
    return (raw ?? []).map((entry) =>
        Array.isArray(entry) ? entry : [entry, null],
    );
}

export interface FormalSnapshotEpochs {
    /** Available epochs, sorted newest first. */
    epochs: EpochEntry[];
    loading: boolean;
    error: string | null;
}

/**
 * Fetches the formal snapshot MANIFEST for `network` and returns the available
 * epochs sorted newest first, along with loading and error state. Re-fetches
 * whenever `network` changes.
 */
export function useFormalSnapshotEpochs(network: Network): FormalSnapshotEpochs {
    const [epochs, setEpochs] = useState<EpochEntry[]>([]);
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

    return { epochs, loading, error };
}
