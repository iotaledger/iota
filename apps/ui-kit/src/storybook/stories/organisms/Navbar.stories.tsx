// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
// import { ButtonSize } from '@/lib/components';

import type { Meta, StoryObj } from '@storybook/react';
import { Home, Assets, Apps, Activity } from '@iota/ui-icons';
import { Navbar, NavbarProps, NavbarItemWithID } from '@/components';
import { useState } from 'react';

const meta: Meta<NavbarProps> = {
    component: Navbar,
    tags: ['autodocs'],
    render: () => {
        const NAVBAR_ITEMS: NavbarItemWithID[] = [
            { id: 'home', icon: <Home /> },
            { id: 'assets', icon: <Assets /> },
            { id: 'apps', icon: <Apps /> },
            { id: 'activity', icon: <Activity /> },
        ];

        const [activeId, setActiveId] = useState<string>(NAVBAR_ITEMS[0].id);

        return (
            <div className="flex w-1/3 border">
                <Navbar
                    items={NAVBAR_ITEMS}
                    activeId={activeId}
                    onClick={(id) => setActiveId(id)}
                />
            </div>
        );
    },
} satisfies Meta<typeof Navbar>;

export default meta;

type Story = StoryObj<typeof meta>;

export const Default: Story = {};
