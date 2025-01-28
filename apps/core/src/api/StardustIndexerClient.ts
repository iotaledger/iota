// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StardustIndexerOutput } from '../utils';

interface PageParams {
    page?: number;
    page_size?: number;
}

export class StardustIndexerClient {
    private baseUrl: string;

    constructor(baseUrl?: string) {
        if (!baseUrl) {
            throw new Error('Base URL for IndexerAPI is required.');
        }
        this.baseUrl = baseUrl;
    }

    private async request<T>(
        endpoint: string,
        options?: RequestInit,
        params?: Record<string, string | number | undefined>,
    ): Promise<T> {
        const url = new URL(`${this.baseUrl}${endpoint}`);

        // Append query parameters if provided
        if (params) {
            Object.entries(params).forEach(([key, value]) => {
                if (value !== undefined) {
                    url.searchParams.append(key, value.toString());
                }
            });
        }

        const response = await fetch(url, {
            ...(options ?? {}),
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

    public getBasicOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/basic/${address}`, undefined, {
            page: params?.page,
            page_size: params?.page_size,
        });
    };

    public getBasicResolvedOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/basic/resolved/${address}`, undefined, {
            page: params?.page,
            page_size: params?.page_size,
        });
    };

    public getNftOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/nft/resolved/${address}`, undefined, {
            page: params?.page,
            page_size: params?.page_size,
        });
    };

    public getNftResolvedOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/nft/resolved/${address}`, undefined, {
            page: params?.page,
            page_size: params?.page_size,
        });
    };
}
