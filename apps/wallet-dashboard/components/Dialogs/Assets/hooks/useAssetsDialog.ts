// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

export enum AssetsDialogView {
    Details = 'Details',
    Send = 'Send',
}

export function useAssetsDialog() {
    const [view, setView] = React.useState<AssetsDialogView>(AssetsDialogView.Details);

    return {
        view,
        setView,
    };
}
