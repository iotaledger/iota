// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Config, CoreClient, EnrichmentPlugin, Event } from '@amplitude/analytics-types';
import { AMP_COOKIES_KEY, EVENTS_STORAGE_KEY, MAX_QUEUED_EVENTS } from './constants';

interface QueuedEvent {
    event: Event;
    timestamp: number;
}

/**
 * Amplitude plugin that buffers events before user consent is given.
 * This plugin:
 * - Stores events in localStorage to persist across page reloads
 * - Queues events when user hasn't consented yet
 * - Sends all queued events when user accepts cookies
 * - Clears all queued events when user declines cookies
 */
class ConsentBufferPlugin implements EnrichmentPlugin {
    name = 'consent-buffer';
    type = 'enrichment' as const;

    private eventQueue: QueuedEvent[] = [];
    private client?: CoreClient;

    constructor() {
        this.loadQueueFromStorage();
    }

    async setup(_config: Config, client?: CoreClient): Promise<void> {
        this.client = client;

        // If user already has consent, flush any queued events from previous sessions
        if (this.consentCookieStatus === 'accepted' && this.eventQueue.length > 0) {
            this.flushQueue();
        }
    }

    async execute(event: Event): Promise<Event | null> {
        if (this.consentCookieStatus === 'pending') {
            // Queue the event both in memory and localStorage
            const queuedEvent: QueuedEvent = {
                event: { ...event },
                timestamp: Date.now(),
            };
            this.eventQueue.push(queuedEvent);

            // Apply size limit: remove oldest events if we exceed the maximum
            if (this.eventQueue.length > MAX_QUEUED_EVENTS) {
                const removed = this.eventQueue.length - MAX_QUEUED_EVENTS;
                this.eventQueue = this.eventQueue.slice(-MAX_QUEUED_EVENTS);
                console.warn(
                    `[ConsentBuffer] Queue size limit exceeded. Removed ${removed} oldest event(s)`,
                );
            }

            this.saveQueueToStorage();
            return null;
        }

        return event;
    }

    /**
     * Called when user accepts cookies.
     * Flushes all queued events to Amplitude and clears localStorage.
     */
    flushQueue(): void {
        if (!this.client) {
            console.error('[ConsentBuffer] Cannot flush - client not available');
            return;
        }

        if (this.eventQueue.length === 0) {
            return;
        }

        const events = [...this.eventQueue];
        this.clearQueue();

        // Send all queued events to Amplitude
        events.forEach(({ event }) => {
            this.client?.track(event).promise.catch((error) => {
                console.error('[ConsentBuffer] Error flushing event:', error);
            });
        });
    }

    /**
     * Called when user declines cookies.
     * Clears all queued events from memory and localStorage.
     */
    clearQueue(): void {
        this.eventQueue = [];
        this.clearStorage();
    }

    /**
     * Get the current queue size (for debugging/testing)
     */
    getQueueSize(): number {
        return this.eventQueue.length;
    }

    /**
     * Get the approximate storage size in bytes (for debugging/monitoring)
     */
    getStorageSize(): number {
        if (typeof localStorage === 'undefined') {
            return 0;
        }

        try {
            const storedData = localStorage.getItem(EVENTS_STORAGE_KEY);
            return storedData ? new Blob([storedData]).size : 0;
        } catch {
            return 0;
        }
    }

    /**
     * Load queued events from localStorage
     */
    private loadQueueFromStorage(): void {
        if (typeof localStorage === 'undefined') {
            return;
        }

        try {
            const storedData = localStorage.getItem(EVENTS_STORAGE_KEY);
            if (storedData) {
                const parsed = JSON.parse(storedData) as QueuedEvent[];
                this.eventQueue = Array.isArray(parsed) ? parsed : [];
            }
        } catch (error) {
            console.error('[ConsentBuffer] Error loading events from localStorage:', error);
            this.eventQueue = [];
        }
    }

    /**
     * Save queued events to localStorage
     */
    private saveQueueToStorage(): void {
        if (typeof localStorage === 'undefined') {
            return;
        }

        try {
            const data = JSON.stringify(this.eventQueue);
            localStorage.setItem(EVENTS_STORAGE_KEY, data);

            // Log storage size for monitoring (only in development)
            if (process.env.NODE_ENV !== 'production') {
                const sizeInKB = (new Blob([data]).size / 1024).toFixed(2);
                console.log(`[ConsentBuffer] Storage size: ${sizeInKB} KB`);
            }
        } catch (error) {
            // Handle quota exceeded errors gracefully
            if (error instanceof Error && error.name === 'QuotaExceededError') {
                console.error(
                    '[ConsentBuffer] localStorage quota exceeded. Clearing oldest events...',
                );
                // Remove half of the events and try again
                this.eventQueue = this.eventQueue.slice(Math.floor(this.eventQueue.length / 2));
                try {
                    localStorage.setItem(EVENTS_STORAGE_KEY, JSON.stringify(this.eventQueue));
                } catch (retryError) {
                    console.error('[ConsentBuffer] Failed to save even after cleanup:', retryError);
                }
            } else {
                console.error('[ConsentBuffer] Error saving events to localStorage:', error);
            }
        }
    }

    /**
     * Clear queued events from localStorage
     */
    private clearStorage(): void {
        if (typeof localStorage === 'undefined') {
            return;
        }

        try {
            localStorage.removeItem(EVENTS_STORAGE_KEY);
            console.log('[ConsentBuffer] Cleared queued events from localStorage');
        } catch (error) {
            console.error('[ConsentBuffer] Error clearing localStorage:', error);
        }
    }

    private get consentCookieStatus() {
        if (typeof document === 'undefined') return 'pending';
        if (document.cookie.includes(`${AMP_COOKIES_KEY}=true`)) return 'accepted';
        if (document.cookie.includes(`${AMP_COOKIES_KEY}=false`)) return 'declined';
        return 'pending';
    }
}

export const consentBufferPlugin = new ConsentBufferPlugin();
