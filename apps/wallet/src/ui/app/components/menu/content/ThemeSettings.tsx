// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { RadioButton } from '@iota/apps-ui-kit';
import { ThemePreference, useTheme } from '@iota/core';
import { Overlay } from '_components';
import { useNavigate } from 'react-router-dom';
import { ampli } from '_src/shared/analytics/ampli';

const THEMES_TO_SHOW = [ThemePreference.Light, ThemePreference.Dark, ThemePreference.System];

const THEME_ENTRIES = (Object.entries(ThemePreference) as Array<[string, ThemePreference]>).filter(
    ([, value]) => THEMES_TO_SHOW.includes(value),
);

export function ThemeSettings() {
    const { themePreference, setThemePreference } = useTheme();

    const navigate = useNavigate();

    function updateThemePreference(value: ThemePreference) {
        setThemePreference(value);
        ampli.changedTheme({ theme: value });
    }
    return (
        <Overlay showModal title="Theme" closeOverlay={() => navigate('/tokens')} showBackButton>
            <div className="flex w-full flex-col">
                {THEME_ENTRIES.map(([label, value]) => (
                    <div className="px-md" key={value}>
                        <RadioButton
                            label={label}
                            isChecked={themePreference === value}
                            onChange={() => updateThemePreference(value)}
                        />
                    </div>
                ))}
            </div>
        </Overlay>
    );
}
