// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { PropsWithChildren, JSX } from 'react';
import type { ExplorerLinkConfig } from '../utils/getExplorerLink';
import type { ValidatorLogoProps } from '../interfaces';

export type RenderExplorerLinkProps = PropsWithChildren<ExplorerLinkConfig>;
export type RenderExplorerLink = (props: RenderExplorerLinkProps) => JSX.Element;
export type RenderValidatorLogo = (props: ValidatorLogoProps) => JSX.Element;
