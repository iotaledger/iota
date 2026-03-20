// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ReactNode } from 'react';
import { useCurrentAccount } from '@iota/dapp-kit';
import { useGetSupplyIncreaseVestingObjects } from '@/hooks';
import { InfoBox, InfoBoxStyle, InfoBoxType, Panel } from '@iota/apps-ui-kit';
import { Vesting } from '@iota/apps-ui-icons';

interface SupplyIncreaseVestingOverviewProps {
    customButton?: ReactNode;
}

export function SupplyIncreaseVestingOverview({
    customButton,
}: SupplyIncreaseVestingOverviewProps = {}) {
    const account = useCurrentAccount();
    const address = account?.address || '';
    const { isSupplyIncreaseVestingScheduleEmpty, supplyIncreaseVestingStakedMapped } =
        useGetSupplyIncreaseVestingObjects(address);

    return !isSupplyIncreaseVestingScheduleEmpty || supplyIncreaseVestingStakedMapped.length > 0 ? (
        <div style={{ gridArea: 'vesting' }} className="with-vesting flex grow overflow-hidden">
            <Panel>
                <div className="flex flex-col gap-md p-md sm:flex-row">
                    <InfoBox
                        title="Your vesting period has ended"
                        supportingText="Claim your rewards and migrate your stake now to make your tokens fully compatible with your favorite wallets and ready for use."
                        type={InfoBoxType.Warning}
                        style={InfoBoxStyle.Default}
                        icon={<Vesting />}
                    />
                    {customButton && (
                        <div className="flex shrink-0 flex-col items-center justify-center">
                            {customButton}
                        </div>
                    )}
                </div>
            </Panel>
        </div>
    ) : null;
}
