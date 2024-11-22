// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { RadioButton } from '@iota/apps-ui-kit';
import { Theme, useTheme } from '@iota/core';
import { Overlay } from '_components';
import { useNavigate } from 'react-router-dom';

export function ThemeSettings() {
    const { theme, setTheme } = useTheme();

    const navigate = useNavigate();

    return (
        <Overlay showModal title="Theme" closeOverlay={() => navigate('/')} showBackButton>
            <div className="flex w-full flex-col">
                {Object.entries(Theme).map(([key, value]) => (
                    <div className="px-md" key={value}>
                        <RadioButton
                            label={key}
                            isChecked={theme === value}
                            onChange={() => setTheme(value)}
                        />
                    </div>
                ))}
            </div>
        </Overlay>
    );
}
