// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useRef } from "react";
import { useHistory } from "@docusaurus/router";
import Heading from "@theme/Heading";
import Parameters from "./parameters";
import Result from "./result";
import Examples from "./examples";
import Markdown from "markdown-to-jsx";
import ScrollSpy from "react-ui-scrollspy";

const Method = (props) => {
  const { json, apis, schemas } = props;
  const history = useHistory();

  const parentScrollContainerRef = () => {
    (useRef < React.HTMLDivElement) | (null > null);
  };

  const handleClick = (e) => {
    let href = "#";
    if (!e.target.nodeName.match(/^H/)) return;
    if (e.target.id) {
      href += e.target.id;
    } else {
      href += e.target.parentNode.id;
    }

    history.push(href);
  };

  return (
    <>
      {apis.map((api) => {
        return (
          <div
            key={`div-${api.replaceAll(/\s/g, "-").toLowerCase()}`}
            ref={parentScrollContainerRef()}
          >
            <Heading
              as="h2"
              id={`${api.replaceAll(/\s/g, "-").toLowerCase()}`}
              className="border-0 border-b border-solid border-iota-blue-dark dark:border-iota-blue scroll-mt-32 text-3xl text-iota-blue-dark dark:text-iota-blue font-bold mt-12 after:content-['_#'] after:hidden after:hover:inline after:opacity-20 cursor-pointer"
              onClick={handleClick}
              key={api.replaceAll(/\s/g, "-").toLowerCase()}
            >
              {api}
            </Heading>
            <ScrollSpy parentScrollContainerRef={parentScrollContainerRef()}>
              {json["methods"]
                .filter((method) => method.tags[0].name === api)
                .map((method) => {
                  const desc = method.description
                    ? method.description
                        .replaceAll(/\</g, "&lt;")
                        .replaceAll(/\{/g, "&#123;")
                    : "";
                  return (
                    <div
                      className={`snap-start scroll-mt-32 ${
                        method.deprecated
                          ? "bg-iota-warning-light p-8 pt-4 rounded-lg mt-8 dark:bg-iota-warning-dark"
                          : "pt-8"
                      }`}
                      key={`div-${api
                        .replaceAll(/\s/g, "-")
                        .toLowerCase()}-${method.name.toLowerCase()}`}
                      id={`${method.name.toLowerCase()}`}
                      onClick={handleClick}
                    >
                      <Heading
                        as="h3"
                        className="text-2xl font-bold after:content-['_#'] after:hidden after:hover:inline after:opacity-20 cursor-pointer"
                        key={`link-${method.name.toLowerCase()}`}
                        id={`${method.name.toLowerCase()}`}
                        onClick={null}
                      >
                        {method.name}
                      </Heading>

                      {method.deprecated && (
                        <div className="p-4 bg-iota-issue rounded-lg font-bold mt-4">
                          Deprecated
                        </div>
                      )}
                      <div className="">
                        <p className="mb-8">
                          <Markdown>{desc}</Markdown>
                        </p>
                        <p className="font-bold mt-4 mb-2 text-xl text-iota-grey-80 dark:text-iota-gray-70">
                          Parameters
                        </p>
                        <Parameters
                          method={method.name.toLowerCase()}
                          params={method.params}
                          schemas={schemas}
                        />
                        <p className="font-bold mb-2 text-xl text-iota-gray-80 dark:text-iota-gray-70">
                          Result
                        </p>
                        <Result result={method.result} json={json} />
                        {method.examples && (
                          <>
                            <p className="mt-4 font-bold text-xl text-iota-gray-80 dark:text-iota-gray-70">
                              Example
                            </p>
                            <Examples method={method.name} examples={method.examples} />
                          </>
                        )}
                      </div>
                    </div>
                  );
                })}
            </ScrollSpy>
          </div>
        );
      })}
    </>
  );
};

export default Method;
