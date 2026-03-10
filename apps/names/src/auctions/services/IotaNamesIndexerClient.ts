// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface AuctionsResponse {
    names: string[];
    page: number;
    pageSize: number;
    totalItems: number;
}

export class IotaNamesIndexerClient {
    private host: string;

    constructor(host: string) {
        this.host = new URL(host).origin;
    }

    async getAllUserAuctions(address: string): Promise<AuctionsResponse> {
        let names: string[] = [];
        let currentPage = 0;
        let totalItems: number | null = null;
        const MAX_PAGE_SIZE = 100;

        do {
            const response = await this.getUserAuctions(address, currentPage, MAX_PAGE_SIZE);
            names = names.concat(response.names);
            totalItems = response.totalItems ?? 0;
            currentPage = response.page + 1;
        } while (totalItems === null || totalItems > names.length);

        return {
            names,
            page: 0,
            pageSize: 0,
            totalItems: totalItems || names.length,
        };
    }

    async getUserAuctions(
        address: string,
        page: number = 0,
        pageSize: number = 50,
    ): Promise<AuctionsResponse> {
        const queryParams = new URLSearchParams({
            page: page.toString(),
            pageSize: pageSize.toString(),
        });
        const response = await fetch(`${this.host}/auctions/${address}?${queryParams.toString()}`);

        if (!response.ok) {
            throw new Error(`Failed to fetch auctions: ${response.statusText}`);
        }

        return response.json();
    }

    async getAuctionList(
        status: 'all' | 'active' | 'finished',
        search?: string,
        sort?: 'asc' | 'desc',
        sortBy?: 'bid' | 'name' | 'ending',
        page: number = 0,
        pageSize: number = 50,
    ): Promise<AuctionsResponse> {
        const url = new URL(`${this.host}/auctions`);
        if (search) {
            url.searchParams.set('search', search);
        }
        if (status !== 'all') {
            url.searchParams.set('status', status);
        }
        if (sort) {
            url.searchParams.set('sort', sort);
        }
        if (sortBy) {
            url.searchParams.set('sortBy', sortBy);
        }
        url.searchParams.set('page', page.toString());
        url.searchParams.set('pageSize', pageSize.toString());

        const response = await fetch(url.toString());

        if (!response.ok) {
            throw new Error(`Failed to fetch auction list: ${response.statusText}`);
        }

        return response.json();
    }
}
