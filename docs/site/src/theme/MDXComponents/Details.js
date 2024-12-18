// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect, useRef } from "react";
import Details from "@theme/Details";
export default function MDXDetails(props) {
  const [hover, setHover] = useState(false);
  const [isOpen, setIsOpen] = useState(false);
  const handleMouseEnter = () => {
    setHover(true);
  };
  const handleMouseLeave = () => {
    setHover(false);
  };
  const handleClick = () => {
    setIsOpen(!isOpen);
  };
  const items = React.Children.toArray(props.children);
  const mergeHandlers = (originalHandler, newHandler) => (event) => {
    if (originalHandler) {
      originalHandler(event);
    }
    if (newHandler) {
      newHandler(event);
    }
  };
  // Split summary item from the rest to pass it as a separate prop to the
  // Details theme component
  const summary = items.find(
    (item) => React.isValidElement(item) && item.type === "summary",
  );
  const children = <>{items.filter((item) => item !== summary)}</>;

  const enhancedSummary = summary
    ? React.cloneElement(summary, {
        onMouseEnter: handleMouseEnter,
        onMouseLeave: handleMouseLeave,
        onClick: mergeHandlers(summary.props.onClick, handleClick),
        className: `${summary.props.className || ""}`, // Add custom class to summary
      })
    : null;

  return (
    <div className="relative">
      <span
        className={`absolute rounded -top-3 -left-1 text-xs bg-white dark:bg-iota-gray-95 px-2 py-0.5 border border-iota-gray-65 border-solid ${hover ? "opacity-100" : "opacity-0"} duration-300 transition-opacity`}
      >
        Click to {isOpen ? "close" : "open"}
      </span>
      <Details
        {...props}
        summary={enhancedSummary}
        className={`${props.className || ""} bg-iota-gray-45 !border-iota-gray-65 dark:bg-iota-gray-90 dark:border-iota-gray-65`}
      >
        {children}
      </Details>
    </div>
  );
}
