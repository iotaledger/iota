// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel, Title } from '@iota/apps-ui-kit';
import type { ComponentProps } from 'react';

type TitleProps = ComponentProps<typeof Title>;
export function EpochStats({
    children,
    ...titleProps
}: React.PropsWithChildren<TitleProps>): JSX.Element {
    return (
        <Panel>
            <div className="flex flex-col">
                {titleProps && <Title {...titleProps} />}
                <div className="w-full p-md--rs">{children}</div>
            </div>
        </Panel>
    );
}

export function EpochStatsGrid({ children }: React.PropsWithChildren): React.JSX.Element {
    return <div className="grid w-full grid-cols-1 gap-md--rs md:grid-cols-2">{children}</div>;
}
