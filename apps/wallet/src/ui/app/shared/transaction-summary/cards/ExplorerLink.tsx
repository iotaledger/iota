// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLinkType } from '_components';
import { useExplorerLink } from '_src/ui/app/hooks/useExplorerLink';

import { Button, ButtonType } from '@iota/apps-ui-kit';
import { ArrowTopRight, Loader } from '@iota/ui-icons';

interface ExplorerLinkCardProps {
    digest?: string;
}

export function ExplorerLinkCard({ digest }: ExplorerLinkCardProps) {
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
                digest ? (
                    <ArrowTopRight />
                ) : (
                    <Loader className="animate-spin" data-testid="loading-indicator" />
                )
            }
            iconAfterText
            disabled={!digest}
        />
    );
}
