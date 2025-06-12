import { AssetsResponse } from '../types';

export class EvmRpcClient {
    public baseUrl: string;

    constructor(baseUrl?: string) {
        if (!baseUrl) {
            throw new Error('Base URL for EVM rpc is required.');
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

        return response.json() as T;
    }

    public getBalanceBaseToken = async (address: string): Promise<AssetsResponse> => {
        return this.request(`/v1/chain/core/accounts/account/${address}/balance`);
    };
}
