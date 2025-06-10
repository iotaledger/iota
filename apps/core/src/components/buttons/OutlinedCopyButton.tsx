// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonType } from '@iota/apps-ui-kit';
import { Copy } from '@iota/apps-ui-icons';
import { useCopyToClipboard } from '../../hooks';

interface OutlinedCopyButtonProps {
    onCopySuccess?: () => void;
    textToCopy: string;
}

export function OutlinedCopyButton({
    onCopySuccess,
    textToCopy,
}: OutlinedCopyButtonProps): React.JSX.Element {
    const copyToClipboard = useCopyToClipboard(onCopySuccess);

    const handleCopy = async () => {
        await copyToClipboard(textToCopy);
    };

    return (
        <Button
            type={ButtonType.Outlined}
            icon={<Copy width={16} height={16} />}
            onClick={handleCopy}
            fullWidth
        />
    );
}
