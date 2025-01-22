// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonProps } from '../Button';

type ButtonPickedProps = Pick<ButtonProps, 'htmlType' | 'testId'>;
type PropsFromButtonElement = Omit<React.HTMLProps<HTMLButtonElement>, 'type'>;

export interface ButtonVariantProps extends ButtonPickedProps, PropsFromButtonElement {
    children: React.ReactNode;
}
