// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { encodeB64 } from '@iota/identity-wasm/web';
import type { OnChainNotarization } from '@iota/notarization/web';
import { useQuery } from '@tanstack/react-query';
import { replaceJsonKeyValue } from '~/lib';

interface NotarizationState {
    content: string;
    lang: string;
    metadata?: string;
}

export function useNotarizationState(notarization: OnChainNotarization) {
    return useQuery<NotarizationState | null>({
        queryKey: ['notarizationState', notarization.id, notarization.state],
        queryFn: async () => {
            const state = notarization.state;
            let content: string;
            let lang = 'text';

            if (!state) {
                return null;
            }

            const dataValue = state.data.value;
            if (typeof dataValue !== 'string') {
                // If dataValue is not a string then it is certainly a byte state
                content = encodeB64(state.data.toBytes());
                lang = 'text';
            } else {
                // Try to parse the string state as JSON first, otherwise show the string
                try {
                    const json = JSON.parse(dataValue);
                    content = JSON.stringify(json, replaceJsonKeyValue, 2);
                    lang = 'json';
                } catch (e) {
                    content = dataValue;
                    lang = 'text';
                }
            }

            return {
                content,
                lang,
                metadata: state?.metadata,
            };
        },
    });
}
