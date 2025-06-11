// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button, ButtonSize, ButtonType, Panel } from '@iota/apps-ui-kit';

interface BannerProps {
    videoSrc: string;
    title: string;
    subtitle?: string;
    onButtonClick: () => void;
    buttonText: string;
    disabled?: boolean;
}
export function Banner({
    videoSrc,
    title,
    subtitle,
    onButtonClick,
    buttonText,
    disabled,
}: BannerProps) {
    return (
        <Panel bgColor="bg-iota-neutral-90 dark:bg-iota-secondary-10">
            <div className="flex h-full w-full justify-between ">
                <div className="flex h-full min-h-[200px] w-full flex-col justify-between p-lg">
                    <div className="flex flex-col gap-xxs">
                        <span className="text-headline-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                            {title}
                        </span>
                        <span className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                            {subtitle}
                        </span>
                    </div>
                    <div>
                        <Button
                            onClick={onButtonClick}
                            size={ButtonSize.Small}
                            type={ButtonType.Outlined}
                            text={buttonText}
                            disabled={disabled}
                        />
                    </div>
                </div>
                <div className="relative w-full overflow-hidden">
                    <video
                        src={videoSrc}
                        autoPlay
                        loop
                        muted
                        className="absolute -top-16 h-80 w-full"
                    ></video>
                </div>
            </div>
        </Panel>
    );
}
