// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';
import { usePopups } from '@/hooks';
import { NewStakePopup } from '../Popup';
import { useEffect, useState } from 'react';

export function StartStaking() {
    const { openPopup, closePopup } = usePopups();
    const [videoSrc, setVideoSrc] = useState('');

    function addNewStake() {
        openPopup(<NewStakePopup onClose={closePopup} />);
    }

    useEffect(() => {
        const updateVideoSrc = () => {
            const isDarkMode = document.documentElement.classList.contains('dark');
            setVideoSrc(
                isDarkMode
                    ? 'https://files.iota.org/media/tooling/wallet-dashboard-staking-dark.mp4'
                    : 'https://files.iota.org/media/tooling/wallet-dashboard-staking-light.mp4',
            );
        };

        // Observer to detect changes in the class attribute of `documentElement`
        const observer = new MutationObserver(updateVideoSrc);

        // Configure the observer to listen only to changes in the `class` attribute
        observer.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['class'],
        });

        updateVideoSrc();

        // Disconnect the observer when the component is unmounted
        return () => observer.disconnect();
    }, []);

    return (
        <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
            <div className="flex h-full w-full justify-between">
                <div className="flex h-full w-full flex-col justify-between p-lg">
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
                <div className="relative w-full overflow-hidden">
                    <video
                        src={videoSrc}
                        autoPlay
                        loop
                        muted
                        className="absolute -top-16 h-80 w-full"
                    ></video>
                </div>
            </div>
        </Panel>
    );
}
