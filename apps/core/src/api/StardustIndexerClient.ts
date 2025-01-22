// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export class StardustIndexerClient {
    private baseUrl: string;

    constructor(baseUrl?: string) {
        if (!baseUrl) {
            throw new Error('Base URL for IndexerAPI is required.');
        }
        this.baseUrl = baseUrl;
    }

    /**
     * Utility function for making API requests
     * @param endpoint - API endpoint (relative to base URL)
     * @param options - Fetch options (e.g., method, headers, body)
     */
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

    /**
     * Fetch basic output details by output ID
     * @param outputId - The ID of the output
     */
    public async getSharedObjects(outputId: string): Promise<unknown> {
        return this.request(`/basic/${outputId}`);
    }
}
