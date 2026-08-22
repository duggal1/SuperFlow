import React from "react";

type ModelStatus =
  | "ready"
  | "loading"
  | "downloading"
  | "verifying"
  | "extracting"
  | "error"
  | "unloaded"
  | "none";

interface ModelStatusButtonProps {
  status: ModelStatus;
  displayText: string;
  isDropdownOpen: boolean;
  onClick: () => void;
  className?: string;
}

const ModelStatusButton: React.FC<ModelStatusButtonProps> = ({
  status,
  displayText,
  isDropdownOpen,
  onClick,
  className = "",
}) => {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 hover:text-text/80 transition-colors ${className}`}
      title={`Model status: ${displayText}`}
    >
      <svg
        width={14}
        height={14}
        viewBox="0 0 108.92 110"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        aria-hidden="true"
        className={`shrink-0 text-blue-600 ${
          status === "loading" || status === "downloading"
            ? "animate-pulse"
            : ""
        }`}
      >
        <path
          fill="currentColor"
          d="M107.25,60.99l-18.19-8.15c-11.99-5.37-16.72-19.92-10.18-31.32l9.92-17.29c1.67-2.91-2.26-5.77-4.51-3.28l-13.38,14.78c-8.81,9.74-24.11,9.74-32.93,0L24.62.95c-2.25-2.49-6.18.37-4.51,3.28l9.92,17.29c6.54,11.39,1.81,25.94-10.18,31.32L1.67,60.99c-3.06,1.37-1.56,5.99,1.72,5.3l19.51-4.1c11.65-2.45,22.91,4.71,25.96,15.79.48,1.74.68,3.55.73,5.35l2.08,19c.36,3.34,5.22,3.34,5.58,0l2.08-19c.05-1.8.25-3.61.73-5.35,3.05-11.08,14.31-18.24,25.96-15.79l19.51,4.1c3.29.69,4.79-3.93,1.72-5.3ZM54.46,61.04c-7.88,0-14.26-6.39-14.26-14.26s6.39-14.26,14.26-14.26,14.26,6.39,14.26,14.26-6.39,14.26-14.26,14.26Z"
        />
      </svg>
      <span className="max-w-28 truncate">{displayText}</span>
      <svg
        className={`w-3 h-3 transition-transform ${isDropdownOpen ? "rotate-180" : ""}`}
        fill="none"
        stroke="currentColor"
        viewBox="0 0 24 24"
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          strokeWidth={2}
          d="M19 9l-7 7-7-7"
        />
      </svg>
    </button>
  );
};

export default ModelStatusButton;
