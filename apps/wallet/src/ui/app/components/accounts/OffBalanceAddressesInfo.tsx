// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, Panel } from '@iota/apps-ui-kit';
import { MissingFundsDialog } from '../../pages/home/tokens/MissingFundsDialog';
import { useState } from 'react';

interface OffBalanceAddressesInfoProps {
    hasVesting: boolean;
    hasMigration: boolean;
    onOpenVestingInfo(): void;
    onOpenMigrationInfo(): void;
}

export function OffBalanceAddressesInfo({
    hasVesting,
    hasMigration,
    onOpenVestingInfo,
    onOpenMigrationInfo,
}: OffBalanceAddressesInfoProps): JSX.Element | null {
    const [dialogMissingAddressesOpen, setDialogMissingAddressesOpen] = useState(false);
    return (
        <>
            <Panel bgColor="bg-iota-secondary-90 dark:bg-iota-secondary-10">
                <div className="flex flex-col gap-xs p-md">
                    <span className="text-title-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                        Off-Balance Addresses
                    </span>

                    <p className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                        Tagged addresses may show inaccurate balances due to vesting or migration
                        that require user action.
                    </p>

                    <div className="flex w-full flex-row items-center justify-between">
                        <div className="flex flex-wrap items-center gap-xxs">
                            {hasVesting && (
                                <button onClick={onOpenVestingInfo}>
                                    <Badge type={BadgeType.Warning} label="Vesting" />
                                </button>
                            )}
                            {hasMigration && (
                                <button onClick={onOpenMigrationInfo}>
                                    <Badge type={BadgeType.Warning} label="Migration" />
                                </button>
                            )}
                        </div>
                        <button onClick={() => setDialogMissingAddressesOpen(true)}>
                            <Badge type={BadgeType.Neutral} label="More Info" />
                        </button>
                    </div>
                </div>
            </Panel>
            <MissingFundsDialog
                open={dialogMissingAddressesOpen}
                setOpen={(isOpen) => setDialogMissingAddressesOpen(isOpen)}
            />
        </>
    );
}
