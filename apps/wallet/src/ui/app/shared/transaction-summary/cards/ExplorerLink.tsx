// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLinkType } from '_src/ui/app/components/explorer-link/ExplorerLinkType';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';
import { ArrowUpRight12 } from '@iota/icons';
import { useEffect, useState } from 'react';

import { Text } from '../../text';
import { Card } from '../Card';

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
    if (!shouldShowExplorerLink) return null;
    return (
        <Card as="a" href={explorerHref!} target="_blank">
            <div className="flex w-full items-center justify-center gap-1 tracking-wider">
                <Text variant="captionSmall" weight="semibold">
                    View on Explorer
                </Text>
                <ArrowUpRight12 className="text-pSubtitle text-steel" />
            </div>
        </Card>
    );
}
