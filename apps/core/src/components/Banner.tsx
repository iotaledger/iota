// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Panel } from '@iota/apps-ui-kit';
import { clsx } from 'clsx';

export enum BannerSize {
    Small = 'small',
    Default = 'default',
}

interface BannerProps {
    videoSrc: string;
    title: string;
    subtitle?: string;
    size?: BannerSize;
}
export function Banner({
    videoSrc,
    title,
    subtitle,
    children,
    size = BannerSize.Default,
}: React.PropsWithChildren<BannerProps>): React.ReactElement {
    return (
        <Panel bgColor="bg-iota-secondary-90 dark:bg-iota-secondary-10">
            <div className="flex h-full w-full justify-between ">
                <div
                    className={clsx(
                        'flex h-full flex-col justify-between gap-y-sm',
                        size === BannerSize.Small
                            ? 'min-h-[120px] p-md w-2/3'
                            : 'min-h-[200px] p-lg w-full',
                    )}
                >
                    <div className="flex flex-col gap-xxs items-start text-start">
                        <span
                            className={clsx(
                                'text-iota-neutral-10 dark:text-iota-neutral-92',
                                size === BannerSize.Small ? 'text-title-sm' : 'text-headline-sm',
                            )}
                        >
                            {title}
                        </span>
                        <span
                            className={clsx(
                                'text-iota-neutral-40 dark:text-iota-neutral-60',
                                size === BannerSize.Small ? 'text-body-sm' : 'text-body-md',
                            )}
                        >
                            {subtitle}
                        </span>
                    </div>
                    <div>{children}</div>
                </div>
                <div
                    className={clsx(
                        'relative overflow-hidden',
                        size === BannerSize.Small ? 'w-1/3' : 'w-full ',
                    )}
                >
                    <video
                        src={videoSrc}
                        autoPlay
                        loop
                        muted
                        className={clsx(
                            'absolute h-80 w-full',
                            size === BannerSize.Small ? '-top-24' : '-top-16',
                        )}
                    ></video>
                </div>
            </div>
        </Panel>
    );
}
