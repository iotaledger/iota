import React from "react";
import clsx from "clsx";
import Link from "@docusaurus/Link";
export default function PaginatorNavLink(props) {
  const { permalink, title, subLabel, isNext } = props;
  return (
    <Link
      className={clsx(
        "pagination-nav__link",
        isNext ? "pagination-nav__link--next" : "pagination-nav__link--prev",
      )}
      to={permalink}
    >
      <div
        className={clsx(
          !isNext ? "page-nav-content-light" : "page-nav-content-blue",
        )}
      >
        <div
          style={
            isNext
              ? {
                  display: "flex",
                  alignItems: "baseline",
                  flexDirection: "column",
                }
              : {
                  display: "flex",
                  alignItems: "flex-end",
                  flexDirection: "column",
                }
          }
        >
          <div
            className={clsx(
              !isNext
                ? "pagination-nav__sublabel--light"
                : "pagination-nav__sublabel--blue",
            )}
          >
            {subLabel}
          </div>
          <div
            className={clsx(
              !isNext
                ? "pagination-nav__label--light"
                : "pagination-nav__label--blue",
            )}
          >
            {title}
          </div>
        </div>
        <div
          style={{ fontSize: "24px", fontWeight: "500" }}
          className={clsx(
            !isNext ? "page-nav-arrow--light" : "page-nav-arrow--blue",
          )}
        >
          {isNext ? ">" : "<"}
        </div>
      </div>
    </Link>
  );
}
