// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Title, TitleSize } from '@iota/apps-ui-kit';

interface ContainerInfoProps {
    title: string;
    titleSize?: TitleSize;
}

export function ContainerInfo({
    title,
    titleSize,
    children,
}: React.PropsWithChildren<ContainerInfoProps>) {
    return (
        <div className="accordion-border-color border pt-sm--rs rounded-xl">
            <Title title={title} size={titleSize} />
            <div className="mt-xs">{children}</div>
        </div>
    );
}
