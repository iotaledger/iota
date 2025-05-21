// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Theme } from '../enums';
import { useTheme } from '../hooks';
import NoDataImage from '../assets/images/no_data.svg';
import NoDataDarkImage from '../assets/images/no_data_darkmode.svg';

interface NoDataProps {
    message: string;
    displayImage: boolean;
}

export function NoData({ message, displayImage }: NoDataProps) {
    const { theme } = useTheme();
    return (
        <div className="flex h-full flex-col items-center justify-center gap-md text-center">
            {displayImage && (theme === Theme.Dark ? <NoDataDarkImage /> : <NoDataImage />)}
            <span className="text-label-lg text-neutral-60">{message}</span>
        </div>
    );
}
