// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

export interface ButtonProps {
    label: string;
}

export function Button({ label }: ButtonProps): React.JSX.Element {
    return <button className="rounded-full border border-blue-500 p-2">{label}</button>;
}
