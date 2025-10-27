// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function Dropdown({ children }: React.PropsWithChildren): React.JSX.Element {
    return (
        <ul className="dropdown-bg dropdown-border-color list-none rounded-lg border py-xs">
            {children}
        </ul>
    );
}
