// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLinkType } from '_components';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';
import { useEffect, useState } from 'react';

import { Button, ButtonType } from '@iota/apps-ui-kit';
import { ArrowTopRight, Loader } from '@iota/ui-icons';

const TIME_TO_WAIT_FOR_EXPLORER = 60 * 1000;

function useShouldShowExplorerLink(timestamp?: string, digest?: string) {
    const [shouldShow, setShouldShow] = useState(false);
    useEffect(() => {
        if (!digest) return;
        const diff = Date.now() - new Date(Number(timestamp)).getTime();
        // if we have a timestamp, wait at least 1m from the timestamp, otherwise wait 1m from now
        const showAfter = timestamp
            ? Math.max(0, TIME_TO_WAIT_FOR_EXPLORER - diff)
            : TIME_TO_WAIT_FOR_EXPLORER;
        const timeout = setTimeout(() => setShouldShow(true), showAfter);
        return () => clearTimeout(timeout);
    }, [timestamp, digest]);

    return shouldShow;
}

interface ExplorerLinkCardProps {
    digest?: string;
    timestamp?: string;
}

export function ExplorerLinkCard({ digest, timestamp }: ExplorerLinkCardProps) {
    const shouldShowExplorerLink = useShouldShowExplorerLink(timestamp, digest);
    const explorerHref = useExplorerLink({
        type: ExplorerLinkType.Transaction,
        transactionID: digest!,
    });

    function handleOpen() {
        const newWindow = window.open(explorerHref!, '_blank', 'noopener,noreferrer');
        if (newWindow) newWindow.opener = null;
    }

    return (
        <Button
            type={ButtonType.Outlined}
            text="View on Explorer"
            onClick={handleOpen}
            fullWidth
            icon={
                shouldShowExplorerLink ? (
                    <ArrowTopRight />
                ) : (
                    <Loader className="animate-spin" data-testid="loading-indicator" />
                )
            }
            iconAfterText
            disabled={!shouldShowExplorerLink}
        />
    );
}
