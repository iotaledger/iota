export function DirectionalArrowsSvg(props: React.SVGProps<SVGSVGElement>) {
    return (
        <svg xmlns="http://www.w3.org/2000/svg" width={64} height={64} fill="none" {...props}>
            <rect
                width={64}
                height={64}
                fill="currentColor"
                rx={8}
                className="text-primary-90 dark:text-primary-10"
            />
            <path
                fill="#3131FF"
                d="M37.978 25.119h3.724v1.619l-3.711 3.643h3.382V42.48L49 34.317v5.297l-6.918 6.97.007.007L39.698 49l-.007-.007-.007.007-2.392-2.41.007-.007-6.918-6.97v-4.847l7.61 7.696V30.38h-.013v-5.262Z"
            />
            <path
                fill="currentColor"
                className="dark:text-neutral-92 text-neutral-10"
                d="M23.002 38.881h3.724v-1.62l-3.711-3.642h3.382V21.52l7.627 8.163v-5.297l-6.918-6.97.007-.007L24.722 15l-.007.007-.008-.007-2.39 2.41.006.006-6.918 6.97v4.848l7.61-7.696V33.62h-.013v5.262Z"
            />
        </svg>
    );
}
