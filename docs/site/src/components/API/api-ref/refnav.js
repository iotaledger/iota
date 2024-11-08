// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState } from "react";
import Link from "@docusaurus/Link";
import NetworkSelect from "./networkselect";

const RefNav = (props) => {
  const { json, apis,openDropdown,setOpenDropdown } = props;
  const [page,setPage] = useState(null)
  const toggleDropdown = (index) => {
    setOpenDropdown(openDropdown === index ? null : index);
  };
  return (
    <div className="mb-24">
      <div className="sticky -top-12 -mt-8 pt-8 pb-2 bg-white dark:bg-ifm-background-color-dark flex justify-center">
        <NetworkSelect />
      </div>

      {apis.map((api, index) => {
        return (
          <div key={`${api.replaceAll(/\s/g, "-").toLowerCase()}`}>
            <div onClick={() => toggleDropdown(index)}>
              <Link
                href={`#${api.replaceAll(/\s/g, "-").toLowerCase()}`}
                data-to-scrollspy-id={`${api
                  .replaceAll(/\s/g, "-")
                  .toLowerCase()}`}
                className="hover:no-underline text-[#525860] dark:text-[#c0c0c0] hover:text-[#525860] dark:hover:text-white"
              >
                <div className="flex justify-between p-[5.625px] hover:bg-[#212121]" style={{alignItems:"center"}}>
                  <div className={`font-medium ${openDropdown===index && 'menu__link--active'}`}>{api}</div>
                  <div className="ml-4 pr-4 text-center">
                    <p className={`transition-transform duration-500 scale-y-150 ${openDropdown === index ? 'rotate-90' : ''}`} style={{marginBottom:'0px',fontSize:'20px'}}>&gt;</p>
                  </div>
                </div>
              </Link> 
            </div>
            <div className={`ml-4 transition-all duration-300 overflow-hidden ${openDropdown === index ? "max-h-screen" : "max-h-0"
              }`}>
              {json["methods"]
                .filter((method) => method.tags[0].name == api)
                .map((method) => {
                  return (
                    <Link
                      className={`menu__link font-medium block hover:no-underline text-base ${page===method.name && 'menu__link--active'}`}
                      key={`link-${method.name.toLowerCase()}`}
                      href={`#${method.name.toLowerCase()}`}
                      data-theme="dark"
                      onClick={() => setPage(method.name)}
                    >
                      {method.name}
                    </Link>
                  );
                })}
            </div>
          </div>
        );
      })}
    </div>
  );
};

export default RefNav;
