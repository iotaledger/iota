// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { OutlinedCopyButton as CoreOutlinedCopyButton } from '@iota/core';
import { ampli } from '@/lib/utils/analytics';

interface OutlinedCopyButtonProps {
    /**
     * Callback function called when copy is successful
     */
    onCopySuccess?: () => void;
    /**
     * Text to copy to clipboard
     */
    textToCopy: string;
    /**
     * Type of element being copied for analytics tracking
     */
    type: string;
}

/**
 * Wrapper around @iota/core's OutlinedCopyButton that adds automatic analytics tracking.
 *
 * This component automatically tracks clipboard copy events using Amplitude's elementCopied event.
 *
 * @example
 * ```tsx
 * <OutlinedCopyButton
 *   textToCopy={address}
 *   type="address"
 *   onCopySuccess={() => toast.success('Address copied')}
 * />
 * ```
 */
export function OutlinedCopyButton({
    onCopySuccess,
    textToCopy,
    type,
}: OutlinedCopyButtonProps): React.JSX.Element {
    const handleCopySuccess = () => {
        // Track analytics event
        ampli.elementCopied({ type });

        // Call the original callback if provided
        if (onCopySuccess) {
            onCopySuccess();
        }
    };

    return <CoreOutlinedCopyButton textToCopy={textToCopy} onCopySuccess={handleCopySuccess} />;
}
