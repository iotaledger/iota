// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { isZkLoginAccountSerializedUI } from '_src/background/accounts/zklogin/ZkLoginAccount';
import {
    LedgerLogo17,
    LogoGoogle,
    LogoQredo,
    LogoTwitch,
    SocialFacebook24,
    SocialKakao24,
    Iota,
} from '@iota/icons';

function IotaIcon() {
    return (
        <div className="flex h-4 w-4 items-center justify-center rounded-full bg-steel p-1 text-white">
            <Iota />
        </div>
    );
}

function ProviderIcon({ provider }: { provider: string }) {
    switch (provider) {
        case 'google':
            return <LogoGoogle className="h-4 w-4" />;
        case 'twitch':
            return <LogoTwitch className="h-4 w-4 text-twitch" />;
        case 'facebook':
            return <SocialFacebook24 className="h-4 w-4 text-facebook" />;
        case 'kakao':
            return <SocialKakao24 className="h-4 w-4" />;
        default:
            return <IotaIcon />;
    }
}

export function AccountIcon({ account }: { account: SerializedUIAccount }) {
    if (isZkLoginAccountSerializedUI(account)) {
        return <ProviderIcon provider={account.provider} />;
    }
    if (account.type === 'qredo') {
        return <LogoQredo className="h-4 w-4" />;
    }
    if (account.type === 'ledger') {
        return <LedgerLogo17 className="h-4 w-4" />;
    }
    return <IotaIcon />;
}
