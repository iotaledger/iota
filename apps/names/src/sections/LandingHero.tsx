// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Search } from '@iota/apps-ui-icons';
import { ButtonUnstyled, Video } from '@iota/apps-ui-kit';

import { SearchStylized } from '@/components/search-component/SearchStylized';
import { useAvailabilityCheckDialog } from '@/stores/useAvailabilityCheckDialog';

const TITLE = 'Your On-Chain Name';
const DESCRIPTION = 'Own a unique, human-readable name on IOTA.';

export function LandingHero() {
    const { open } = useAvailabilityCheckDialog();

    return (
        <div className="relative min-h-[700px] md:min-h-[560px] overflow-hidden">
            <Video
                src="https://files.iota.org/media/iota-names/homepage_hero.mp4"
                isAutoPlayEnabled
                disableControls
                className="absolute inset-0 w-full h-full object-cover z-[-1]"
                poster="https://files.iota.org/media/iota-names/homepage_hero_poster.jpg"
            />

            <div className="container w-full h-full pt-[200px] pb-20 flex flex-col items-center justify-center gap-y-2xl text-center">
                <div className="flex flex-col gap-md">
                    <h1 className="text-display-md -tracking-[0.4px]">{TITLE}</h1>
                    <p className="text-title-lg leading-5 -tracking-[0.4px]">{DESCRIPTION}</p>
                </div>

                <div className="w-full max-w-2xl flex flex-col">
                    <SearchStylized
                        placeholder="Search for your IOTA name"
                        onFocus={() => open({ autoFocusInput: true })}
                        trailingElement={
                            <ButtonUnstyled
                                className="p-sm rounded-full [&_svg]:h-5 [&_svg]:w-5 bg-names-neutral-100"
                                aria-label="Search"
                            >
                                <Search className="text-names-primary-0" />
                            </ButtonUnstyled>
                        }
                        lightSearch
                    />
                </div>
            </div>
        </div>
    );
}
