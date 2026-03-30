// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type {
    AppsBackendClientOptions,
    CoinPriceResponse,
    FeatureDefinition,
    FeaturesResponse,
    MonitorNetworkResponse,
    ProductAnalyticsConfigResponse,
} from './types';

export class AppsBackendClient {
    private url: string;
    private features: Record<string, FeatureDefinition> = {};
    private attributes: Record<string, unknown> = {};
    private listeners: Set<() => void> = new Set();
    private snapshot: Record<string, FeatureDefinition> = this.features;

    constructor(options: AppsBackendClientOptions) {
        this.url = options.url;
    }

    async init(): Promise<void> {
        await this.refreshFeatures();
    }

    async refreshFeatures(): Promise<void> {
        try {
            const res = await fetch(`${this.url}/api/features`);
            if (!res.ok) {
                throw new Error('Failed to fetch features');
            }
            const data: FeaturesResponse = await res.json();
            this.features = data.features;
            this.updateSnapshot();
        } catch (error) {
            console.error(error);
        }
    }

    getFeatureValue<T>(key: string, defaultValue: T): T {
        const feature = this.features[key];
        if (feature && feature.defaultValue !== undefined) {
            return feature.defaultValue as T;
        }
        return defaultValue;
    }

    getFeatures(): Record<string, FeatureDefinition> {
        return this.features;
    }

    setAttributes(attrs: Record<string, unknown>): void {
        this.attributes = attrs;
    }

    getAttributes(): Record<string, unknown> {
        return this.attributes;
    }

    async setPayload(payload: { features: Record<string, FeatureDefinition> }): Promise<void> {
        this.features = payload.features;
        this.updateSnapshot();
    }

    subscribe(listener: () => void): () => void {
        this.listeners.add(listener);
        return () => {
            this.listeners.delete(listener);
        };
    }

    getSnapshot(): Record<string, FeatureDefinition> {
        return this.snapshot;
    }

    async getCoinPrice(coin: string): Promise<CoinPriceResponse> {
        return this.request(`coin-price/${coin}`);
    }

    async getMonitorNetwork(project: string): Promise<MonitorNetworkResponse> {
        return this.request('monitor-network', { project });
    }

    async getProductAnalyticsConfig(): Promise<ProductAnalyticsConfigResponse> {
        return this.request('product-analytics');
    }

    async checkRestricted(): Promise<boolean> {
        const res = await fetch(`${this.url}/api/restricted/`, {
            method: 'POST',
            headers: { accept: '', 'content-type': 'application/json' },
            body: JSON.stringify({}),
        });
        return res.status === 403;
    }

    async request<T>(
        path: string,
        queryParams?: Record<string, string>,
        options?: RequestInit,
    ): Promise<T> {
        const base = `${this.url}/${path}`;
        const url =
            queryParams && Object.keys(queryParams).length > 0
                ? `${base}?${new URLSearchParams(queryParams)}`
                : base;
        const res = await fetch(url, options);
        if (!res.ok) {
            throw new Error('Unexpected response');
        }
        return res.json();
    }

    private updateSnapshot(): void {
        this.snapshot = { ...this.features };
        this.listeners.forEach((listener) => listener());
    }
}
