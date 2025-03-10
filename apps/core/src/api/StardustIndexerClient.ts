// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StardustIndexerOutput } from '../utils';

export interface PageParams {
    page?: number;
    pageSize?: number;
}

export class StardustIndexerClient {
    public baseUrl: string;

    constructor(baseUrl?: string) {
        if (!baseUrl) {
            throw new Error('Base URL for IndexerAPI is required.');
        }
        // Normalize baseUrl by removing any trailing slash in the end
        this.baseUrl = baseUrl.replace(/\/$/, '');
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

    public getBasicResolvedOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/v1/basic/resolved/${address}`, undefined, {
            page: params?.page,
            page_size: params?.pageSize,
        });
    };

    public getNftResolvedOutputs = async (
        address: string,
        params?: PageParams,
    ): Promise<StardustIndexerOutput[]> => {
        return this.request(`/v1/nft/resolved/${address}`, undefined, {
            page: params?.page,
            page_size: params?.pageSize,
        });
    };
}
