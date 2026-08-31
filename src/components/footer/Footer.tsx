import React, { useState, useEffect } from "react";
import { getVersion } from "@tauri-apps/api/app";

import UpdateChecker from "../update-checker";
import { Badge } from "../ui/Badge";
import ModelDownloadManager from "./ModelDownloadManager";
import { useIsLight } from "../../lib/utils/theme";

const Footer: React.FC = () => {
  const [version, setVersion] = useState("");

  useEffect(() => {
    const fetchVersion = async () => {
      try {
        const appVersion = await getVersion();
        setVersion(appVersion);
      } catch (error) {
        console.error("Failed to get app version:", error);
        setVersion("1.0.0");
      }
    };

    fetchVersion();
  }, []);

  const isLight = useIsLight();
  return (
    <div
      className={`w-full pt-3 ${isLight ? "border-t border-stone-200/80 bg-white" : ""}`}
    >
      <div
        className={`flex items-center justify-between px-4 pb-3 text-xs ${isLight ? "text-stone-500" : "text-text/60"}`}
      >
        <div className="flex items-center gap-4">
          <ModelDownloadManager />
        </div>

        {/* Update Status */}
        <div className="flex items-center gap-1.5">
          <UpdateChecker />
          <span>•</span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <span className={isLight ? "text-stone-600" : ""}>
            v{version === "1.0.0" ? "1.0" : version}
          </span>
          {/* eslint-disable-next-line i18next/no-literal-string */}
          <Badge variant="violet" className="px-1.5 py-0 text-[11px]">
            Beta
          </Badge>
        </div>
      </div>
    </div>
  );
};

export default Footer;
