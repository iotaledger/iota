// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType, Panel } from '@iota/apps-ui-kit';
import { MissingAssetsDialog } from '../../pages/home/tokens/MissingAssetsDialog';
import { useState } from 'react';

interface OffBalanceAssetsInfoProps {
    hasVesting: boolean;
    hasMigration: boolean;
    onOpenVestingInfo(): void;
    onOpenMigrationInfo(): void;
}

export function OffBalanceAssetsInfo({
    hasVesting,
    hasMigration,
    onOpenVestingInfo,
    onOpenMigrationInfo,
}: OffBalanceAssetsInfoProps): JSX.Element | null {
    const [dialogMissingAssetsOpen, setDialogMissingAssetsOpen] = useState(false);
    return (
        <>
            <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
                <div className="flex flex-col gap-xs p-md">
                    <span className="text-title-sm text-neutral-10 dark:text-neutral-92">
                        Off-Balance Assets
                    </span>

                    <p className="text-body-sm text-neutral-40 dark:text-neutral-60">
                        Tagged addresses may require manual input to be accurately added and
                        reflected in your balance.
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
                        <button onClick={() => setDialogMissingAssetsOpen(true)}>
                            <Badge type={BadgeType.Neutral} label="More Info" />
                        </button>
                    </div>
                </div>
            </Panel>
            <MissingAssetsDialog
                open={dialogMissingAssetsOpen}
                setOpen={(isOpen) => setDialogMissingAssetsOpen(isOpen)}
            />
        </>
    );
}
