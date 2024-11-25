// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Theme, useTheme } from '@iota/core';
import NoDataImage from '_assets/images/no_data.svg';
import NoDataDarkImage from '_assets/images/no_data_darkmode.svg';
interface NoDataProps {
    message: string;
}

export function NoData({ message }: NoDataProps) {
    const { theme } = useTheme();

    return (
        <div className="flex h-full flex-col items-center justify-center gap-md text-center">
            {theme === Theme.Dark ? <NoDataDarkImage /> : <NoDataImage />}
            <span className="text-label-lg text-neutral-60">{message}</span>
        </div>
    );
}
