// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaLogoMark } from '@iota/apps-ui-icons';
import { Address } from '@iota/apps-ui-kit';

interface IdentityPackageOfficialProps {
    value: string;
    copyValue: string;
}

export function IdentityPackageOfficial({ value, copyValue }: IdentityPackageOfficialProps) {
    return (
        <div className="flex flex-row items-baseline gap-xs">
            <IotaLogoMark className="self-center" />
            <Address text={value} isCopyable copyText={copyValue} />
        </div>
    );
}
