// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';
import { ArrowTopRight } from '@iota/ui-icons';

interface ViewTxnOnExplorerButtonProps {
    digest?: string;
}

export function ViewTxnOnExplorerButton({
    digest,
}: ViewTxnOnExplorerButtonProps): React.JSX.Element {
    return (
        <Button
            type={ButtonType.Outlined}
            text="View on Explorer"
            fullWidth
            icon={digest ? <ArrowTopRight /> : <LoadingIndicator data-testid="loading-indicator" />}
            iconAfterText
            disabled={!digest}
        />
    );
}
