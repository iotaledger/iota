// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// import { Button } from '_app/shared/ButtonUI';
import { Button, ButtonType } from '@iota/apps-ui-kit'

interface SelectAllButtonProps {
    accountIds: string[];
    selectedAccountIds: string[];
    onChange: (ids: string[]) => void;
    selectAllText?: string;
    deselectAllText?: string;
}

export function SelectAllButton({
    accountIds = [],
    selectedAccountIds = [],
    onChange,
    selectAllText = 'Select All Accounts',
    deselectAllText = 'Deselect All Accounts',
}: SelectAllButtonProps) {
    return (
        <Button
            onClick={() => {
                if (selectedAccountIds.length < accountIds.length) {
                    onChange(accountIds);
                } else {
                    onChange([]);
                }
            }}
            type={ButtonType.Ghost}
            fullWidth
            text={selectedAccountIds.length < accountIds.length ? selectAllText : deselectAllText}
        />
    );
}
