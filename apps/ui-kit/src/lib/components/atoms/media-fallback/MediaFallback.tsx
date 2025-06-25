// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PlaceholderReplace } from '@iota/apps-ui-icons';

export function MediaFallback() {
    return (
        <div className="dark:bg-iota-neutral-10 flex h-full w-full items-center justify-center bg-iota-neutral-96">
            <PlaceholderReplace className="dark:text-iota-neutral-60 h-4 w-4 text-iota-neutral-40" />
        </div>
    );
}
