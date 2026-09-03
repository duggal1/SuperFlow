
import type { SVGProps } from "react";

export type IconProps = SVGProps<SVGSVGElement>;

const iconClassName = "shrink-0 text-stone-950 dark:text-white";

const PRIMARY_OPACITY = 0.94;
const SECONDARY_OPACITY = 0.52;

function getClassName(className?: string) {
  return className ? `${iconClassName} ${className}` : iconClassName;
}

export function HomeIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        clipRule="evenodd"
        d="M10.0591 1.3631C9.4333 0.886555 8.56694 0.887431 7.94127 1.36279L2.69155 5.35258C2.2559 5.68344 2 6.19865 2 6.74598V14.25C2 15.7692 3.23079 17 4.75 17H13.25C14.7692 17 16 15.7692 16 14.25V6.74598C16 6.20006 15.7448 5.68396 15.3088 5.35286L10.0591 1.3631Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
        fillRule="evenodd"
      />
      <path
        d="M10.5 17V13C10.5 12.5 10 12 9 12C8 12 7.5 12.5 7.5 13V17H10.5Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
    </svg>
  );
}

export function SparklesIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        d="M3.025 5.623C3.093 5.827 3.285 5.965 3.5 5.965C3.715 5.965 3.906 5.827 3.975 5.623L4.396 4.36L5.659 3.939C5.863 3.871 6.001 3.68 6.001 3.465C6.001 3.25 5.863 3.059 5.659 2.991L4.396 2.57L3.975 1.307C3.838 0.899 3.163 0.899 3.026 1.307L2.605 2.57L1.342 2.991C1.138 3.059 1 3.25 1 3.465C1 3.68 1.138 3.871 1.342 3.939L2.605 4.36L3.025 5.623Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
      <path
        d="M16.525 8.803L11.99 7.01L10.197 2.475C9.97 1.903 9.029 1.903 8.802 2.475L7.009 7.01L2.474 8.803C2.188 8.916 1.999 9.193 1.999 9.5C1.999 9.807 2.187 10.084 2.474 10.197L7.009 11.99L8.802 16.525C8.915 16.811 9.192 16.999 9.499 16.999C9.806 16.999 10.083 16.811 10.196 16.525L11.989 11.99L16.524 10.197C16.81 10.084 16.999 9.807 16.999 9.5C16.999 9.193 16.811 8.916 16.525 8.803Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
    </svg>
  );
}

export function PeopleSearchIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        clipRule="evenodd"
        d="M13 10C11.3428 10 10 11.3428 10 13C10 14.6572 11.3428 16 13 16C13.5565 16 14.0775 15.8486 14.5241 15.5847L15.7197 16.7803C16.0126 17.0732 16.4875 17.0732 16.7804 16.7803C17.0732 16.4874 17.0732 16.0126 16.7804 15.7197L15.5848 14.5241C15.8486 14.0774 16 13.5564 16 13C16 11.3428 14.6572 10 13 10ZM11.5 13C11.5 12.1712 12.1712 11.5 13 11.5C13.8288 11.5 14.5 12.1712 14.5 13C14.5 13.8288 13.8288 14.5 13 14.5C12.1712 14.5 11.5 13.8288 11.5 13Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
        fillRule="evenodd"
      />
      <path
        d="M10.7632 16.9058C9.41078 16.1297 8.5 14.6714 8.5 13C8.5 11.3926 9.34236 9.98233 10.6098 9.18639C10.0932 9.06451 9.55423 9 8.99999 9C6.14167 9 3.69058 10.7157 2.60517 13.1674C2.05162 14.4186 2.74425 15.8317 4.01259 16.2313C5.29503 16.6354 6.99283 17 8.99999 17C9.6181 17 10.2069 16.9654 10.7632 16.9058Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M9 7.50049C10.7952 7.50049 12.25 6.04543 12.25 4.25049C12.25 2.45554 10.7952 1.00049 9 1.00049C7.20482 1.00049 5.75 2.45554 5.75 4.25049C5.75 6.04543 7.20482 7.50049 9 7.50049Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
    </svg>
  );
}

export function PeopleIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        clipRule="evenodd"
        d="M0.554137 13.5756C1.34525 11.4759 3.36866 9.978 5.74997 9.978C8.13128 9.978 10.1547 11.4759 10.9458 13.5756C11.3059 14.5315 10.7272 15.5154 9.84596 15.8102C8.82613 16.1509 7.42657 16.477 5.75097 16.477C4.0754 16.477 2.67527 16.151 1.65458 15.8104C0.771586 15.5163 0.194851 14.5312 0.554137 13.5756Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
        fillRule="evenodd"
      />
      <path
        d="M12.5523 13.9772C13.9847 13.9159 15.1901 13.6248 16.096 13.3222C16.9772 13.0274 17.5559 12.0435 17.1958 11.0875C16.4047 8.98793 14.3813 7.48999 12 7.48999C10.5581 7.48999 9.24737 8.03921 8.26202 8.93866C10.147 9.65809 11.6398 11.1632 12.3495 13.0467C12.4675 13.3601 12.5329 13.6723 12.5523 13.9772Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M5.75 8.50049C6.99267 8.50049 8 7.49361 8 6.25049C8 5.00736 6.99267 4.00049 5.75 4.00049C4.50733 4.00049 3.5 5.00736 3.5 6.25049C3.5 7.49361 4.50733 8.50049 5.75 8.50049Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
      <path
        d="M12 6.00049C13.2427 6.00049 14.25 4.99361 14.25 3.75049C14.25 2.50736 13.2427 1.50049 12 1.50049C10.7573 1.50049 9.75 2.50736 9.75 3.75049C9.75 4.99361 10.7573 6.00049 12 6.00049Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
    </svg>
  );
}

export function GeneralIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        d="M16.2501 6H13.2501C12.836 6 12.5001 5.6641 12.5001 5.25C12.5001 4.8359 12.836 4.5 13.2501 4.5H16.2501C16.6642 4.5 17.0001 4.8359 17.0001 5.25C17.0001 5.6641 16.6642 6 16.2501 6Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M8.75009 6H1.75009C1.33599 6 1.00009 5.6641 1.00009 5.25C1.00009 4.8359 1.33599 4.5 1.75009 4.5H8.75009C9.16419 4.5 9.50009 4.8359 9.50009 5.25C9.50009 5.6641 9.16419 6 8.75009 6Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M4.75009 13.5H1.75009C1.33599 13.5 1.00009 13.1641 1.00009 12.75C1.00009 12.3359 1.33599 12 1.75009 12H4.75009C5.16419 12 5.50009 12.3359 5.50009 12.75C5.50009 13.1641 5.16419 13.5 4.75009 13.5Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M16.2501 13.5H9.25009C8.83599 13.5 8.50009 13.1641 8.50009 12.75C8.50009 12.3359 8.83599 12 9.25009 12H16.2501C16.6642 12 17.0001 12.3359 17.0001 12.75C17.0001 13.1641 16.6642 13.5 16.2501 13.5Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M11.0001 8.25C12.6569 8.25 14.0001 6.90685 14.0001 5.25C14.0001 3.59315 12.6569 2.25 11.0001 2.25C9.34324 2.25 8.00009 3.59315 8.00009 5.25C8.00009 6.90685 9.34324 8.25 11.0001 8.25Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
      <path
        d="M7.00009 15.75C8.65695 15.75 10.0001 14.4069 10.0001 12.75C10.0001 11.0931 8.65695 9.75 7.00009 9.75C5.34324 9.75 4.00009 11.0931 4.00009 12.75C4.00009 14.4069 5.34324 15.75 7.00009 15.75Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
    </svg>
  );
}

export function HistoryIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        d="M9 1.5C5.399 1.5 2.405 4.067 1.723 7.47L1.53 6.89C1.399 6.497 0.974 6.285 0.581 6.416C0.188 6.547 -0.024 6.972 0.107 7.365L0.857 9.615C0.959 9.921 1.245 10.127 1.567 10.127H1.583C1.837 10.127 2.073 9.998 2.211 9.785L3.461 7.86C3.687 7.513 3.588 7.048 3.241 6.823C3.032 6.687 2.778 6.668 2.559 6.751C3.464 4.557 5.622 3 9 3C12.314 3 15 5.686 15 9C15 12.314 12.314 15 9 15C6.844 15 4.955 13.863 3.897 12.155C3.679 11.803 3.217 11.694 2.865 11.912C2.513 12.13 2.404 12.592 2.622 12.944C3.944 15.08 6.308 16.5 9 16.5C13.142 16.5 16.5 13.142 16.5 9C16.5 4.858 13.142 1.5 9 1.5Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M9 4.5C9.414 4.5 9.75 4.836 9.75 5.25V8.689L12.28 10.149C12.639 10.356 12.762 10.815 12.555 11.174C12.348 11.533 11.889 11.656 11.53 11.449L8.625 9.772C8.393 9.638 8.25 9.391 8.25 9.123V5.25C8.25 4.836 8.586 4.5 9 4.5Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
    </svg>
  );
}

export function AboutIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 18 18"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      {...props}
    >
      <path
        d="M9 1.25C4.72 1.25 1.25 4.72 1.25 9C1.25 13.28 4.72 16.75 9 16.75C13.28 16.75 16.75 13.28 16.75 9C16.75 4.72 13.28 1.25 9 1.25Z"
        fill="currentColor"
        fillOpacity={SECONDARY_OPACITY}
      />
      <path
        d="M9 7.75C9.414 7.75 9.75 8.086 9.75 8.5V12.25C9.75 12.664 9.414 13 9 13C8.586 13 8.25 12.664 8.25 12.25V8.5C8.25 8.086 8.586 7.75 9 7.75Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
      <path
        d="M9 4.75C9.552 4.75 10 5.198 10 5.75C10 6.302 9.552 6.75 9 6.75C8.448 6.75 8 6.302 8 5.75C8 5.198 8.448 4.75 9 4.75Z"
        fill="currentColor"
        fillOpacity={PRIMARY_OPACITY}
      />
    </svg>
  );
}

export function VADIcon({ className, ...props }: IconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      className={getClassName(className)}
      fill="none"
      stroke="currentColor"
      strokeWidth="1.65"
      strokeLinecap="round"
      strokeLinejoin="round"
      {...props}
    >
      <path
        d="M13.8461 4C14.6683 4 15.3801 4.62364 15.5584 5.50016L15.7715 6.54763C16.0037 7.68837 16.93 8.5 18 8.5H11.3827M11.3827 8.5L10.3116 13.2894C9.98567 14.5945 8.90024 15.5 7.66156 15.5H4.83224C3.62479 15.5 2.74786 14.246 3.06556 12.9738L4.44552 6.94753C4.88008 5.20729 6.32731 4 7.97888 4H13.8461C13.0551 4 12.362 4.57821 12.1539 5.41168L11.3827 8.5Z"
        strokeOpacity={PRIMARY_OPACITY}
      />
      <path
        d="M10.1539 20C9.33175 20 8.61992 19.3764 8.44158 18.4998L8.22845 17.4524C7.99635 16.3116 7.06995 15.5 6 15.5L12.6173 15.5M12.6173 15.5L13.6884 10.7106C14.0143 9.40546 15.0998 8.5 16.3384 8.5L19.1678 8.5C20.3752 8.5 21.2521 9.75395 20.9344 11.0262L19.5545 17.0525C19.1199 18.7927 17.6727 20 16.0211 20L10.1539 20C10.9449 20 11.638 19.4218 11.8461 18.5883L12.6173 15.5Z"
        strokeOpacity={SECONDARY_OPACITY}
      />
    </svg>
  );
}

export const icons = {
  home: HomeIcon,
  ai: SparklesIcon,
  sparkle: SparklesIcon,
  peopleSearch: PeopleSearchIcon,
  people: PeopleIcon,
  general: GeneralIcon,
  history: HistoryIcon,
  about: AboutIcon,
  vad: VADIcon,
} as const;

export type IconName = keyof typeof icons;

