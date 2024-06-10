// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import React from "react";
import { MDXProvider } from "@mdx-js/react";
import MDXComponents from "@theme/MDXComponents";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import { Card, Cards } from "@site/src/components/Cards";
export default function MDXContent({ children }) {
  const iotaComponents = {
    ...MDXComponents,
    Card,
    Cards,
    Tabs,
    TabItem,
  };
  return <MDXProvider components={iotaComponents}>{children}</MDXProvider>;
}
