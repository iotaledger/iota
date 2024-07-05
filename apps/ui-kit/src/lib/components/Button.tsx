// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Theme } from '../enums';

export interface ButtonProps {
    label: string;
    darkmode?: boolean;
}

const THEME_CLASSES: Record<Theme, string> = {
    [Theme.Light]: 'bg-primary-40 text-white',
    [Theme.Dark]: 'bg-primary-70 text-neutral-20',
};

export function Button({ label, darkmode }: ButtonProps): React.JSX.Element {
    const mode = darkmode ? Theme.Dark : Theme.Light;
    const themeClass = THEME_CLASSES[mode];
    return <button className={`rounded-full px-xs py-xxs ${themeClass}`}>{label}</button>;
}
