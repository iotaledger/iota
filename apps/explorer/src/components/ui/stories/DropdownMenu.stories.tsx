// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// import { Filter16 } from '@iota/icons';
import { type Meta, type StoryObj } from '@storybook/react';
import { useState } from 'react';

import { DropdownMenu, DropdownMenuCheckboxItem } from '../DropdownMenu';

export default {
    component: DropdownMenu,
} as Meta;

export const Default: StoryObj<typeof DropdownMenu> = {
    render: () => {
        const [check1, setCheck1] = useState(false);
        const [check2, setCheck2] = useState(false);
        return (
            <DropdownMenu
                trigger={null} // Previously: <Filter16 />
                content={
                    <>
                        <DropdownMenuCheckboxItem
                            label="Checkbox 1"
                            checked={check1}
                            onSelect={(e) => e.preventDefault()}
                            onCheckedChange={() => setCheck1((v) => !v)}
                        />
                        <DropdownMenuCheckboxItem
                            label="Checkbox 2"
                            checked={check2}
                            onSelect={(e) => e.preventDefault()}
                            onCheckedChange={() => setCheck2((v) => !v)}
                        />
                    </>
                }
                modal={false}
            />
        );
    },
};
