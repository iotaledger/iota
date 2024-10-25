// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeType } from '@iota/apps-ui-kit';

export function TopNav() {
    return (
        <div className="flex w-full flex-row justify-end gap-md py-xs--rs outline outline-1 outline-primary-40">
            <Badge label="Mainnet" type={BadgeType.PrimarySoft} />
        </div>
    );
}
