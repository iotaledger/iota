// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { PlaceholderReplace } from '@iota/apps-ui-icons';

export function MediaFallback() {
    return (
        <div className="bg-neutral-96 dark:bg-neutral-10 flex h-full w-full items-center justify-center">
            <PlaceholderReplace className="text-neutral-40 dark:text-neutral-60 h-4 w-4" />
        </div>
    );
}
