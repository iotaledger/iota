// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SecondaryText } from '@iota/apps-ui-kit';
import cx from 'clsx';

export interface InputWrapperProps {
    errorMessage?: string;
    disabled?: boolean;
}

export function InputWrapper({
    disabled,
    errorMessage,
    children,
}: React.PropsWithChildren<InputWrapperProps>) {
    return (
        <div
            className={cx('group flex w-full flex-col gap-y-2', {
                'cursor-not-allowed opacity-40': disabled,
                errored: errorMessage,
                enabled: !disabled,
            })}
        >
            {children}
            {errorMessage && <SecondaryText hasErrorStyles>{errorMessage}</SecondaryText>}
        </div>
    );
}
