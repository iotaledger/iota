// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled } from '@iota/apps-ui-kit';
import { Pause, Play } from '@iota/apps-ui-icons';

export interface PlayPauseProps {
    paused?: boolean;
    onChange(): void;
}

export function PlayPause({ paused, onChange }: PlayPauseProps): JSX.Element {
    const Icon = paused ? Play : Pause;

    return (
        <ButtonUnstyled
            aria-label={paused ? 'Paused' : 'Playing'}
            onClick={onChange}
            className="relative cursor-pointer border-none bg-transparent p-xxs text-iota-neutral-40 dark:text-iota-neutral-60"
        >
            <Icon />
        </ButtonUnstyled>
    );
}
