// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { OutlinedCopyButton as CoreOutlinedCopyButton } from '@iota/core';
import { ampli } from '@/lib/utils/analytics';
import React from 'react';

type CoreOutlinedCopyButtonProps = React.ComponentProps<typeof CoreOutlinedCopyButton>;

interface OutlinedCopyButtonProps extends Omit<CoreOutlinedCopyButtonProps, 'onCopySuccess'> {
    /**
     * A string that identifies the type of text being copied, used for analytics tracking.
     */
    analyticType: string;
    /**
     * Callback function called when copy is successful
     */
    onCopySuccess?: CoreOutlinedCopyButtonProps['onCopySuccess'];
    /**
     * Whether to track the copy event in analytics
     * @default true
     */
    trackEvent?: boolean;
}

/**
 * Wrapper around @iota/core's OutlinedCopyButton that adds automatic analytics tracking.
 * This component automatically tracks clipboard copy events using Amplitude's elementCopied event.
 */
export function OutlinedCopyButton({
    onCopySuccess,
    analyticType,
    trackEvent = true,
    ...rest
}: OutlinedCopyButtonProps): React.JSX.Element {
    const handleCopySuccess = React.useCallback(() => {
        // Track analytics event
        if (analyticType && trackEvent) {
            ampli.elementCopied({ type: analyticType });
        }

        // Call the original callback if provided
        if (onCopySuccess) {
            onCopySuccess();
        }
    }, [analyticType, onCopySuccess, trackEvent]);

    return <CoreOutlinedCopyButton {...rest} onCopySuccess={handleCopySuccess} />;
}
