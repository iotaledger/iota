// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect } from "react";
import _ from "lodash";
import { getRef } from "../index";
import { Light as SyntaxHighlighter } from "react-syntax-highlighter";
import { a11yLight, vs2015 } from "react-syntax-highlighter/dist/esm/styles/hljs";
import BrowserOnly from '@docusaurus/BrowserOnly';

const TypeDef = (props) => {
  return (
    <BrowserOnly fallback={<div>Loading...</div>}>
      {() => {
        const theme = localStorage.getItem("theme");
        const [light, setLight] = useState(theme === "light");

        useEffect(() => {
          const checkTheme = () => {
            const theme = localStorage.getItem("theme");
            setLight(theme === "light");
          };

          window.addEventListener("storage", checkTheme);
          return () => {
            window.removeEventListener("storage", checkTheme);
          };
        }, [light]);

        const { schema, schemas } = props;
        const schemaObj = schemas[schema];
        let refs = [{ title: schema, ...schemaObj }];

        const collectRefs = (obj) => {
          for (const [key, value] of Object.entries(obj)) {
            if (value && Array.isArray(value)) {
              for (let x = 0; x < value.length; x++) {
                collectRefs(value[x]);
              }
            } else if (value && typeof value === "object") {
              collectRefs(value);
            }
            if (key === "$ref") {
              refs.push({ title: getRef(value), ...schemas[getRef(value)] });
            }
          }
        };

        collectRefs(schemaObj);

        refs.map((ref) => collectRefs(schemas[ref.title]));
        refs.map((ref) => collectRefs(schemas[ref.title]));

        return (
          <div>
            {_.uniqWith(refs, (a, b) => a.title === b.title).map((curObj, idx) => (
              <div key={idx}>
                <p className="text-lg font-bold mb-0 mt-8">{curObj.title}</p>
                <hr className="mt-0" />
                <SyntaxHighlighter 
                  language="json" 
                  style={light ? a11yLight : vs2015} 
                  customStyle={{
                    ...(light ? { boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.1)' } : {}),
                    padding: '15px'
                  }}
                >
                  {JSON.stringify(_.omit(curObj, "title"), null, 4)}
                </SyntaxHighlighter>
              </div>
            ))}
          </div>
        );
      }}
    </BrowserOnly>
  );
};

export default TypeDef;
