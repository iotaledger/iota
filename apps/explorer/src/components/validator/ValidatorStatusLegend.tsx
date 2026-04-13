// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Badge, BadgeSize, BadgeType, Panel, Title, TitleSize } from '@iota/apps-ui-kit';

const VALIDATOR_ROLES = [
    {
        type: BadgeType.Success,
        label: 'Committee',
        description: 'In the committee with voting power',
    },
    {
        type: BadgeType.PrimarySoft,
        label: 'Active',
        description: 'Eligible, not in committee',
    },
    {
        type: BadgeType.Warning,
        label: 'Pending',
        description: 'Activating in the next epoch',
    },
    {
        type: BadgeType.Neutral,
        label: 'Candidate',
        description: 'Candidate for future epochs',
    },
    {
        type: BadgeType.Error,
        label: 'At Risk',
        description: 'At risk of being slashed or penalized',
    },
];

export function ValidatorStatusLegend(): JSX.Element {
    return (
        <Panel>
            <div className="bg-shader-neutral-light-4 flex flex-col gap-y-sm border-b border-t border-shader-neutral-light-8 py-sm">
                <Title
                    size={TitleSize.Small}
                    title="Status Legend"
                    tooltipText="Each validator is assigned a role reflecting their current standing in the network."
                />
                <div className="grid grid-cols-2 gap-xl px-md md:grid-cols-3 lg:grid-cols-5">
                    {VALIDATOR_ROLES.map(({ type, label, description }) => (
                        <div key={label} className="flex flex-col items-start gap-xs">
                            <Badge type={type} label={label} size={BadgeSize.Small} />
                            <span className="text-label-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                                {description}
                            </span>
                        </div>
                    ))}
                </div>
            </div>
        </Panel>
    );
}
