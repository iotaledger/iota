// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { useEffect, useState } from 'react';
import { fetchCommittee } from '../api/client';
import type { CommitteeMessage } from '../api/types';

export interface UseCommitteeResult {
  committee: CommitteeMessage | null;
  loading: boolean;
  error: string | null;
}

export function useCommittee(): UseCommitteeResult {
  const [committee, setCommittee] = useState<CommitteeMessage | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;

    fetchCommittee()
      .then((data) => {
        if (!cancelled) {
          setCommittee(data);
          setLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : 'Failed to fetch committee');
          setLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, []);

  return { committee, loading, error };
}
