// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const EIP6963_ANNOUNCE_PROVIDER_EVENT = 'eip6963:announceProvider';

// EIP6963 Types
// @see https://eips.ethereum.org/EIPS/eip-6963

/**
 * EIP-6963: Multi Injected Provider Discovery
 */
interface EIP6963ProviderInfo {
    rdns: string;
    uuid: string;
    name: string;
    icon: string;
}

/**
 * EIP-6963: Multi Injected Provider Discovery
 */
interface EIP6963ProviderDetail {
    info: EIP6963ProviderInfo;
    provider: unknown;
}

/**
 * EIP-6963: Multi Injected Provider Discovery
 */
interface EIP6963AnnounceProviderEvent extends CustomEvent {
    type: typeof EIP6963_ANNOUNCE_PROVIDER_EVENT;
    detail: EIP6963ProviderDetail;
}

// Only allow these wallets to be discovered via EIP-6963
const ALLOWED_WALLET_RDNS = ['io.metamask'];

export function interceptProviderAnnouncements() {
    if (typeof window !== 'undefined') {
        const originalAddEventListener = window.addEventListener.bind(window);

        window.addEventListener = function <K extends keyof WindowEventMap>(
            type: K | string,
            listener: EventListenerOrEventListenerObject | ((event: Event) => void),
            options?: boolean | AddEventListenerOptions,
        ): void {
            if (type === EIP6963_ANNOUNCE_PROVIDER_EVENT) {
                // Wrap the listener to filter out wallets not in the allowed list
                const wrappedListener = function (event: EIP6963AnnounceProviderEvent): void {
                    const detail = event.detail;
                    if (detail?.info?.rdns && !ALLOWED_WALLET_RDNS.includes(detail.info.rdns)) {
                        return;
                    }

                    if (typeof listener === 'function') {
                        listener.call(this, event);
                    } else if (listener && typeof listener.handleEvent === 'function') {
                        listener.handleEvent(event);
                    }
                };

                return originalAddEventListener(type, wrappedListener as EventListener, options);
            }

            return originalAddEventListener(type, listener as EventListener, options);
        };
    }
}
