// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';

import { usePopups } from '@/hooks';
import { NewStakePopup } from '../Popup';

export function StartStaking() {
    const { openPopup, closePopup } = usePopups();

    function addNewStake() {
        openPopup(<NewStakePopup onClose={closePopup} />);
    }

    return (
        <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
            <div className="h-full w-full">
                <div className="flex h-full flex-col justify-between p-lg">
                    <div className="flex flex-col gap-xxs">
                        <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                            Start Staking
                        </span>
                        <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                            Earn Rewards
                        </span>
                    </div>
                    <div>
                        <Button
                            onClick={addNewStake}
                            size={ButtonSize.Small}
                            type={ButtonType.Outlined}
                            text="Stake"
                        />
                    </div>
                </div>
            </div>
        </Panel>
    );
}
