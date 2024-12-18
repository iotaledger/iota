// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from "react";
import Link from "@docusaurus/Link";

const CompNav = (props) => {
  const { json, apis } = props;

  return (
    <div className="mb-32">
      <div>
        <h2>Component schemas</h2>
        {Object.keys(json["components"]["schemas"]).map((component) => {
          return (
          <div key={component}>
            <Link href={`#${component.toLowerCase()}`}
            data-to-scrollspy-id={`${component.toLowerCase()}`}
            className="my-1 pl-4 block text-iota-gray-95 dark:text-iota-grey-35 hover:no-underline dark:hover:text-iota-blue">
              {component}
            </Link>
            </div>
        )})}      
      </div>
    </div>
  );
};

export default CompNav;
