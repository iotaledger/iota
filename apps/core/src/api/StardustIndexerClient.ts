// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StardustIndexerOutput } from '../utils';

export class StardustIndexerClient {
    private baseUrl: string;

    constructor(baseUrl?: string) {
        if (!baseUrl) {
            throw new Error('Base URL for IndexerAPI is required.');
        }
        this.baseUrl = baseUrl;
    }

    private async request<T>(endpoint: string, options?: RequestInit): Promise<T> {
        const url = `${this.baseUrl}${endpoint}`;
        const response = await fetch(url, {
            ...options,
            headers: {
                'Content-Type': 'application/json',
                ...(options?.headers || {}),
            },
        });

        if (!response.ok) {
            const errorText = await response.text();
            throw new Error(`API Error: ${response.status} ${response.statusText} - ${errorText}`);
        }

        return response.json();
    }

    public async getBasicResolvedOutputs(address: string): Promise<StardustIndexerOutput[]> {
        return this.request(`/basic/resolved/${address}`);
    }

    public async getNftResolvedOutputs(address: string): Promise<StardustIndexerOutput[]> {
        return this.request(`/nft/resolved/${address}`);
    }
}
