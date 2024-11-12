import React, { useState, useEffect } from "react";
import Markdown from "markdown-to-jsx";
import { Light as SyntaxHighlighter } from "react-syntax-highlighter";
import js from "react-syntax-highlighter/dist/esm/languages/hljs/json";
import { a11yLight, vs2015 } from "react-syntax-highlighter/dist/esm/styles/hljs";
import BrowserOnly from '@docusaurus/BrowserOnly';

SyntaxHighlighter.registerLanguage("json", js);

const Examples = (props) => {
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

  const { method, examples } = props;
  const request = {
    jsonrpc: "2.0",
    id: 1,
    method,
    params: examples[0].params.map((item) => item.value),
  };

  const stringRequest = JSON.stringify(request, null, 2).replaceAll('"  value": ', "");
  const response = {
    jsonrpc: "2.0",
    result: examples[0].result.value,
    id: 1,
  };

  return (
    <BrowserOnly fallback={<div>Loading...</div>}>
      {() => (
        <div className="mx-4">
          <p className="my-2">
            <Markdown>{examples[0].name}</Markdown>
          </p>
          {examples[0].params && (
            <div>
              <p className="font-bold mt-4 text-iota-gray-80 dark:text-iota-gray-50">
                Request
              </p>
              <SyntaxHighlighter
                language="js"
                style={light ? a11yLight : vs2015}
                customStyle={{
                  ...(light ? { boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.1)' } : {}),
                  padding: '15px'
                }}
              >
                {stringRequest}
              </SyntaxHighlighter>  
            </div>
          )}
          {examples[0].result.value && (
            <div>
              <p className="font-bold mt-6 text-iota-gray-80 dark:text-iota-gray-50">
                Response
              </p>
              <SyntaxHighlighter 
                language="json" 
                style={light ? a11yLight : vs2015} 
                customStyle={{
                  ...(light ? { boxShadow: '0 1px 2px 0 rgba(0, 0, 0, 0.1)' } : {}),
                  padding: '15px'
                }}>
                {JSON.stringify(response, null, 2)}
              </SyntaxHighlighter>
            </div>
          )}
        </div>
      )}
    </BrowserOnly>
  );
};

export default Examples;
