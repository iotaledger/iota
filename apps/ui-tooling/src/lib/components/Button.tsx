// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

export interface ButtonProps {
    label: string;
}

export function Button({ label }: ButtonProps): React.JSX.Element {
    return <button className="p-2 border rounded-full border-blue-500">{label}</button>;
}
