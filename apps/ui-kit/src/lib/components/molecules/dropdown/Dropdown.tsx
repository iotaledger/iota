// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';

export interface DropdownProps extends React.PropsWithChildren {
    className?: string;
}

export function Dropdown({ children, className }: DropdownProps): React.JSX.Element {
    return (
        <ul
            className={cx(
                'dropdown-bg dropdown-border-color list-none rounded-lg border py-xs',
                className,
            )}
        >
            {children}
        </ul>
    );
}
