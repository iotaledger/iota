// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function Dropdown({ children }: React.PropsWithChildren): React.JSX.Element {
    return (
        <ul className="border-neutral-80 dark:border-neutral-20 dark:bg-neutral-6 list-none rounded-lg border bg-neutral-100 py-xs">
            {children}
        </ul>
    );
}
