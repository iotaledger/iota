// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useInitializedGuard } from '../../hooks';
import { Text } from '../../shared/text';
import SadCapy from './SadCapy.svg';

export function RestrictedPage() {
    useInitializedGuard(true);

    return (
        <div className="flex w-full max-w-[400px] flex-col items-center gap-10 rounded-20 bg-sui/10 px-10 py-15 text-center">
            <SadCapy role="presentation" />
            <Text variant="pBody" color="steel-darker" weight="medium">
                Regrettably this service is not available to you. Applicable laws prohibit us from
                providing our services to your location at this time.
            </Text>
        </div>
    );
}
