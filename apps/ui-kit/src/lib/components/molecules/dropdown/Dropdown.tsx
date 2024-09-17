// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function Dropdown({ children }: React.PropsWithChildren): React.JSX.Element {
    return (
        <ul className="list-none rounded-lg border border-neutral-80 bg-neutral-100 py-xs dark:border-neutral-20 dark:bg-neutral-6">
            {children}
        </ul>
    );
}
