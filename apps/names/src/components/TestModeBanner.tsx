// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Close, Warning } from '@iota/apps-ui-icons';
import { Button, ButtonSize, ButtonType } from '@iota/apps-ui-kit';
import { useIotaClientContext } from '@iota/dapp-kit';

export function TestModeBanner({
    isBannerVisible,
    onDismiss,
}: {
    isBannerVisible: boolean;
    onDismiss?: () => void;
}) {
    const { network } = useIotaClientContext();

    const shouldShowBanner =
        network !== 'mainnet' && network !== 'localnet' && network !== 'custom';

    if (!shouldShowBanner || !isBannerVisible) {
        return null;
    }

    const handleDismiss = () => {
        onDismiss?.();
    };

    const handleSwitchToMainnet = () => {
        window.location.href = 'https://iotanames.com/';
    };

    return (
        <div
            className="bg-yellow-100 border-b border-yellow-200 z-[60] h-[104px] md:h-[48px]"
            data-testmode-banner={isBannerVisible}
        >
            <div className="flex flex-col md:flex-row items-start md:items-center justify-between w-full h-full px-md py-xs md:py-sm gap-xs md:gap-0">
                <div className="flex items-center gap-xs flex-1 w-full md:w-auto">
                    <Warning className="w-4 h-4 text-yellow-700 flex-shrink-0" />
                    <div className="flex flex-col">
                        <p className="text-sm font-medium text-yellow-800">Test Mode Active</p>
                        <p className="text-xs text-yellow-700">
                            You're connected to {network}. No real funds will be used.
                        </p>
                    </div>
                    <div className="md:hidden ml-auto">
                        <Button
                            onClick={handleDismiss}
                            icon={<Close className="text-yellow-900" />}
                            size={ButtonSize.Small}
                            type={ButtonType.Ghost}
                            aria-label="Dismiss test mode banner"
                        />
                    </div>
                </div>
                <div className="flex items-center justify-center md:justify-end gap-xxs w-full md:w-auto">
                    <Button
                        onClick={handleSwitchToMainnet}
                        text="Switch to Mainnet"
                        size={ButtonSize.Small}
                        type={ButtonType.Secondary}
                    />
                    <div className="hidden md:block">
                        <Button
                            onClick={handleDismiss}
                            icon={<Close className="text-yellow-900" />}
                            size={ButtonSize.Small}
                            type={ButtonType.Ghost}
                            aria-label="Dismiss test mode banner"
                        />
                    </div>
                </div>
            </div>
        </div>
    );
}
