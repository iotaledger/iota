// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, MenuIcon } from '@iota/apps-ui-icons';
import { IOTABridgeLogo, ThemeSwitcher } from '..';
import { ConnectButtonL1, ConnectButtonL2 } from './connect-buttons';
import { useState } from 'react';
import { Button, ButtonType, Divider } from '@iota/apps-ui-kit';
import clsx from 'clsx';
import { useNetwork, useFeatureEnabledByNetwork, Feature } from '@iota/core';
import type { Network } from '@iota/iota-sdk/client';
import { Link } from '../link';

export function Header(): React.JSX.Element {
    const [isMobileMenuOpen, setIsMobileMenuOpen] = useState(false);
    const networkName = useNetwork();
    const iotaNamesEnabled = useFeatureEnabledByNetwork(Feature.IotaNames, networkName as Network);

    const MobileIcon = isMobileMenuOpen ? Close : MenuIcon;
    return (
        <div className="fixed top-0 left-0 py-md--rs backdrop-blur-lg z-10 w-full">
            <div className="container flex justify-between items-center">
                <Link href="/" aria-label="IOTA EVM Bridge Home">
                    <IOTABridgeLogo className="dark:text-iota-neutral-92 text-iota-neutral-10" />
                </Link>
                <div className="flex flex-row gap-xs">
                    <ThemeSwitcher />

                    <div className="hidden md:flex flex-row gap-xs items-center">
                        <ConnectButtonL1
                            className="text-label-lg h-10"
                            connectText="Connect L1 Wallet"
                            size="md"
                            iotaNamesEnabled={iotaNamesEnabled}
                        />
                        <ConnectButtonL2 />
                    </div>

                    <div className="flex md:hidden">
                        <Button
                            type={ButtonType.Ghost}
                            icon={<MobileIcon className="h-5 w-5" />}
                            onClick={() => setIsMobileMenuOpen(!isMobileMenuOpen)}
                        />
                    </div>
                </div>
            </div>
            <div
                className={clsx(
                    isMobileMenuOpen ? 'max-h-[300px]' : 'max-h-0',
                    'transition-[max-height] overflow-y-hidden md:hidden',
                )}
            >
                <div className="flex flex-col gap-md items-end p-lg">
                    <ConnectButtonL1
                        className="text-label-lg h-10"
                        connectText="Connect L1 Wallet"
                        size="md"
                        iotaNamesEnabled={iotaNamesEnabled}
                    />
                    <Divider />
                    <ConnectButtonL2 />
                </div>
            </div>
        </div>
    );
}
