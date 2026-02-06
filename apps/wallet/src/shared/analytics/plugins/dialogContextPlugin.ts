// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BrowserClient, EnrichmentPlugin, Event } from '@amplitude/analytics-types';
import type { ContextSnapshot } from './contextSnapshotCache';
import { contextSnapshotCache } from './contextSnapshotCache';

/** Extract the title of the currently open dialog from the DOM. */
/** Extract the title of the currently open dialog from the DOM. */
function extractDialogTitle(): string | null {
    const dialog = document.querySelector('div[role="dialog"]');
    if (!dialog) {
        return null;
    }

    // Try multiple strategies to find dialog title
    const title =
        // headers within the dialog
        dialog.querySelector('.header-bg-color span.text-title-lg')?.textContent?.trim() ||
        dialog
            .querySelector('.dialog-content-bg .header-bg-color span.text-title-lg')
            ?.textContent?.trim() ||
        // dialogs without header might have a title element directly inside
        dialog.querySelector('.text-headline-sm')?.textContent?.trim() ||
        // elements with title-like class names
        dialog.querySelector('[class*="title"], [class*="heading"]')?.textContent?.trim() ||
        // ARIA attributes for accessibility
        dialog.getAttribute('aria-label') ||
        dialog.getAttribute('aria-labelledby') ||
        // fallback to dialog ID or data attribute if no title found
        dialog.id ||
        dialog.getAttribute('data-dialog-name');

    return title || null;
}

/** Capture a snapshot of the current UI context. */
function captureContextSnapshot(): ContextSnapshot {
    return {
        dialogTitle: extractDialogTitle(),
        timestamp: Date.now(),
    };
}

/** Handle click events during capture phase to snapshot UI context. */
function handleCapturePhaseClick(event: MouseEvent): void {
    if (event.type !== 'click') return;
    const snapshot = captureContextSnapshot();
    contextSnapshotCache.set(snapshot);
}

/** Track dialog opened event when a dialog appears in the DOM. */
function setupDialogTracking(client: BrowserClient): () => void {
    if (typeof window === 'undefined' || typeof document === 'undefined') {
        return () => {};
    }

    const observer = new MutationObserver((mutations) => {
        for (const mutation of mutations) {
            mutation.addedNodes.forEach((node) => {
                if (node instanceof HTMLElement) {
                    // Check if the added node is a dialog or contains a dialog
                    const dialog = node.matches?.('div[role="dialog"]')
                        ? node
                        : node.querySelector?.('div[role="dialog"]');

                    if (dialog) {
                        // Wait a tick for dialog content to render
                        setTimeout(() => {
                            const dialogTitle = extractDialogTitle();
                            const eventName = dialogTitle
                                ? `${dialogTitle} Dialog Opened`
                                : 'Dialog Opened';

                            client.track(eventName, {
                                dialog_title: dialogTitle || 'Untitled',
                            });
                        }, 50);
                    }
                }
            });
        }
    });

    observer.observe(document.body, {
        childList: true,
        subtree: true,
    });

    return () => {
        observer.disconnect();
    };
}

/** Set up event listeners to capture UI context before user interactions. */
function setupContextCapture(): () => void {
    if (typeof window === 'undefined' || typeof document === 'undefined') {
        return () => {};
    }

    document.addEventListener('click', handleCapturePhaseClick, true);

    return () => {
        document.removeEventListener('click', handleCapturePhaseClick, true);
    };
}

/**
 * Amplitude enrichment plugin that adds dialog context to all events
 * and tracks dialog open events.
 */
export function dialogContextPlugin(client: BrowserClient): EnrichmentPlugin {
    let cleanupContextCapture: (() => void) | null = null;
    let cleanupDialogTracking: (() => void) | null = null;

    return {
        name: 'dialog-context',
        type: 'enrichment',

        setup: async () => {
            cleanupContextCapture = setupContextCapture();
            cleanupDialogTracking = setupDialogTracking(client);
            return Promise.resolve();
        },

        execute: async (event: Event): Promise<Event> => {
            if (typeof window === 'undefined' || typeof document === 'undefined') {
                return event;
            }

            const cachedContext = contextSnapshotCache.get();
            const dialogTitle = cachedContext?.dialogTitle ?? extractDialogTitle();

            if (dialogTitle) {
                return {
                    ...event,
                    event_properties: {
                        ...event.event_properties,
                        dialog_title: dialogTitle,
                    },
                };
            }

            return event;
        },

        teardown: async () => {
            if (cleanupContextCapture) {
                cleanupContextCapture();
            }
            if (cleanupDialogTracking) {
                cleanupDialogTracking();
            }
            contextSnapshotCache.clear();
            return Promise.resolve();
        },
    };
}
