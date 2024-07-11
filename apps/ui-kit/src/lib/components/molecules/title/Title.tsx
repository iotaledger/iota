// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ComponentProps } from 'react';
import { Button } from '../../atoms';

interface TitleProps {
    /**
     * The title of the component.
     */
    title: string;
    /**
     * The subtitle of the component.
     */
    subtitle?: string;
    /**
     * Props for the button component.
     */
    button?: ComponentProps<typeof Button>;
    /**
     * Show the info icon with the info text.
     */
    info?: string;
}

export function Title({ title, subtitle, button }: TitleProps) {
    return (
        <div className="flex flex-row items-center justify-between gap-x-6 px-md py-sm">
            <div className="flex flex-col justify-start">
                <div className="flex flex-row items-center gap-x-0.5 text-neutral-10 dark:text-neutral-92">
                    <h4 className="text-title-lg">{title}</h4>
                    <svg
                        xmlns="http://www.w3.org/2000/svg"
                        width="16"
                        height="16"
                        viewBox="0 0 16 16"
                        fill="none"
                    >
                        <path
                            fill-rule="evenodd"
                            clip-rule="evenodd"
                            d="M8 14C11.3137 14 14 11.3137 14 8C14 4.68629 11.3137 2 8 2C4.68629 2 2 4.68629 2 8C2 11.3137 4.68629 14 8 14ZM8.0002 4.4C7.55837 4.4 7.2002 4.75817 7.2002 5.2C7.2002 5.64183 7.55837 6 8.0002 6C8.44202 6 8.8002 5.64183 8.8002 5.2C8.8002 4.75817 8.44202 4.4 8.0002 4.4ZM8.8002 10.4C8.8002 10.8418 8.44202 11.2 8.0002 11.2C7.55837 11.2 7.2002 10.8418 7.2002 10.4V8C7.2002 7.55817 7.55837 7.2 8.0002 7.2C8.44202 7.2 8.8002 7.55817 8.8002 8V10.4Z"
                            fill="currentColor"
                        />
                    </svg>
                </div>
                <p className="text-label-md text-neutral-60 dark:text-neutral-40">{subtitle}</p>
            </div>
            <Button {...button} />
        </div>
    );
}
