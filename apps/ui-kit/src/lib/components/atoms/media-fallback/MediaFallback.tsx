// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PlaceholderReplace } from '@iota/apps-ui-icons';

export function MediaFallback() {
    return (
        <div className="flex h-full w-full items-center justify-center bg-neutral-96 dark:bg-neutral-10">
            <PlaceholderReplace className="h-4 w-4 text-neutral-40 dark:text-neutral-60" />
        </div>
    );
}
