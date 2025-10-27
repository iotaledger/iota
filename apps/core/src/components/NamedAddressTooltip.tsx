// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Tooltip } from '@iota/apps-ui-kit';

interface NamedAddressTooltipProps {
    name?: string | null | undefined;
    address: string;
}

export function NamedAddressTooltip({
    name,
    address,
    children,
}: React.PropsWithChildren<NamedAddressTooltipProps>): React.ReactNode {
    if (name) {
        return <Tooltip text={address}>{children}</Tooltip>;
    }

    return children;
}
