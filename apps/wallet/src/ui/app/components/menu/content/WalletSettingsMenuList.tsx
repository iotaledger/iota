// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useNextMenuUrl, Loading } from '_components';
import { useAppSelector } from '_hooks';
import { getCustomNetwork } from '_src/shared/api-env';
import { FAQ_LINK, ToS_LINK } from '_src/shared/constants';
import { formatAutoLock, useAutoLockMinutes } from '_src/ui/app/hooks/useAutoLockMinutes';
import FaucetRequestButton from '_src/ui/app/shared/faucet/FaucetRequestButton';
import { Link } from '_src/ui/app/shared/Link';
import { Text } from '_src/ui/app/shared/text';
import { ArrowUpRight12, Clipboard24, Domain24, LockLocked24, More24 } from '@iota/icons';
import { getNetwork, Network } from '@iota/iota.js/client';
import Browser from 'webextension-polyfill';

import { MenuLayout } from './MenuLayout';
import MenuListItem from './MenuListItem';

function MenuList() {
    const networkUrl = useNextMenuUrl(true, '/network');
    const autoLockUrl = useNextMenuUrl(true, '/auto-lock');
    const moreOptionsUrl = useNextMenuUrl(true, '/more-options');
    const network = useAppSelector((state) => state.app.network);
    const networkConfig = network === Network.Custom ? getCustomNetwork() : getNetwork(network);
    const version = Browser.runtime.getManifest().version;
    const autoLockInterval = useAutoLockMinutes();

    return (
        <MenuLayout title="Wallet Settings">
            <div className="divide-gray-45 flex flex-col divide-x-0 divide-y divide-solid">
                <MenuListItem
                    to={networkUrl}
                    icon={<Domain24 />}
                    title="Network"
                    subtitle={networkConfig.name}
                />
                <MenuListItem
                    to={autoLockUrl}
                    icon={<LockLocked24 />}
                    title="Auto-lock Accounts"
                    subtitle={
                        <Loading loading={autoLockInterval?.isPending}>
                            {autoLockInterval.data === null ? 'Not set up' : null}
                            {typeof autoLockInterval.data === 'number'
                                ? formatAutoLock(autoLockInterval.data)
                                : null}
                        </Loading>
                    }
                />
                <MenuListItem icon={<Clipboard24 />} title="FAQ" href={FAQ_LINK} />
                <MenuListItem
                    icon={<More24 className="text-steel-darker" />}
                    title="More options"
                    to={moreOptionsUrl}
                />
            </div>
            <div className="flex-1" />
            <div className="mt-2.5 flex flex-col items-stretch">
                <FaucetRequestButton variant="outline" />
            </div>
            <div className="mt-3.75 flex flex-col items-center justify-center gap-3.75 px-2.5 no-underline">
                <Link
                    href={ToS_LINK}
                    text="Terms of service"
                    after={<ArrowUpRight12 />}
                    color="steelDark"
                    weight="semibold"
                />
                <Text variant="bodySmall" weight="medium" color="steel">
                    On Iota Wallet version v{version}
                </Text>
            </div>
        </MenuLayout>
    );
}

export default MenuList;
