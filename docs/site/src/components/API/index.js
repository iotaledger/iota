// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect } from "react";
import BrowserOnly from "@docusaurus/BrowserOnly";
import Heading from "@theme/Heading";
import RefNav from "./api-ref/refnav";
import Methods from "./api-ref/method";
import ScrollSpy from "react-ui-scrollspy";

// TODO: Once the repo is public, we can use the old imports again and reactivate the ../../utils/getopenrpcspecs.js script
import openrpc_mainnet from "../../../../../crates/iota-open-rpc/spec/openrpc.json";
import openrpc_testnet from "../../../../../crates/iota-open-rpc/spec/openrpc.json";
import openrpc_devnet from "../../../../../crates/iota-open-rpc/spec/openrpc.json";

export function getRef(url) {
  return url.substring(url.lastIndexOf("/") + 1, url.length);
}

const Rpc = () => {
  return (
    <BrowserOnly fallback={<p>Loading...</p>}>
      {() => {
        const [openrpc, setOpenRpc] = useState(() => {
          const network = localStorage.getItem("RPC");
          switch (network) {
            case "mainnet":
              return openrpc_mainnet;
            case "testnet":
              return openrpc_testnet;
            case "devnet":
              return openrpc_devnet;
            default:
              return openrpc_mainnet;
          }
        });

        const [openDropdown, setOpenDropdown] = useState(null);

        useEffect(() => {
          const rpcswitch = () => {
            const network = localStorage.getItem("RPC");
            switch (network) {
              case "mainnet":
                setOpenRpc(openrpc_mainnet);
                break;
              case "testnet":
                setOpenRpc(openrpc_testnet);
                break;
              case "devnet":
                setOpenRpc(openrpc_devnet);
                break;
              default:
                setOpenRpc(openrpc_mainnet);
            }
          };

          window.addEventListener("storage", rpcswitch);
          return () => {
            window.removeEventListener("storage", rpcswitch);
          };
        }, []);

        const apis = [
          ...new Set(openrpc["methods"].map((api) => api.tags[0].name)),
        ].sort();
        const schemas = openrpc.components.schemas;

        if (!openrpc) {
          return <p>Open RPC file not found.</p>;
        }

        let ids = openrpc["methods"].map((method) =>
          method.name.replaceAll(/\s/g, "-").toLowerCase()
        );

        return (
          <div className="mx-4 flex flex-row">
            <div className="pt-12 w-[290px] mb-24 flex-none max-h-screen overflow-y-auto sticky top-12" style={!openDropdown ? { borderRight: '1px solid #444950' } : {}}>
              <RefNav json={openrpc} apis={apis} openDropdown={openDropdown} setOpenDropdown={setOpenDropdown} />
            </div>

            <main className="flex-grow w-3/4">
              <div className="mx-8">
                <div className="">
                  <Heading as="h1" className=" w-full py-4 top-14">
                    IOTA JSON-RPC Reference - Version: {openrpc.info.version}
                  </Heading>
                  <ScrollSpy>
                    <div className="">
                      <p>{openrpc.info.description}</p>
                      <Methods json={openrpc} apis={apis} schemas={schemas} />
                    </div>
                  </ScrollSpy>
                </div>
              </div>
            </main>
          </div>
        );
      }}
    </BrowserOnly>
  );
};

export default Rpc;
