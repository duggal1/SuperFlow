import React from "react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  AiBeautifyIcon,
  BadgeInfoIcon,
  HistoryIcon,
  Home01Icon,
  Settings01Icon,
  ZapIcon,
} from "@hugeicons/core-free-icons";
import SuperFlowLogo from "./icons/SuperFlowTextLogo";
import SidebarToggleIcon from "./icons/SidebarToggleIcon";
import { HomePage } from "./home";
import { AICleanupSettings } from "./ai-cleanup";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  ModelsSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface SectionConfig {
  labelKey: string;
  icon: IconSvgElement;
  component: React.ComponentType;
  enabled: (settings: ReturnType<typeof useSettings>["settings"]) => boolean;
}

export const SECTIONS_CONFIG = {
  home: {
    labelKey: "sidebar.home",
    icon: Home01Icon,
    component: HomePage,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: ZapIcon,
    component: ModelsSettings,
    enabled: () => true,
  },
  general: {
    labelKey: "sidebar.general",
    icon: Home01Icon,
    component: GeneralSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: HistoryIcon,
    component: HistorySettings,
    enabled: () => true,
  },
  aiCleanup: {
    labelKey: "sidebar.aiCleanup",
    icon: AiBeautifyIcon,
    component: AICleanupSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Settings01Icon,
    component: AdvancedSettings,
    enabled: () => true,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: Settings01Icon,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: BadgeInfoIcon,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
  open: boolean;
  onToggle: () => void;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
  open,
  onToggle,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <motion.nav
      initial={false}
      animate={{ width: open ? 176 : 0 }}
      transition={{ duration: 0.25, ease: [0.32, 0.72, 0, 1] }}
      className="h-full shrink-0 overflow-hidden bg-sidebar"
    >
      <div className="flex h-full w-44 flex-col px-2 pb-2 pt-3">
        {/* Brand + collapse control */}
        <div className="mb-3 flex items-center justify-between pl-1.5 pr-0.5">
          <SuperFlowLogo className="text-stone-100" />
          <button
            type="button"
            onClick={onToggle}
            aria-label="Toggle sidebar"
            title="Toggle sidebar (Ctrl+B)"
            className="flex size-7 shrink-0 items-center justify-center rounded-lg text-stone-500 transition-colors duration-150 hover:bg-stone-900 hover:text-stone-300"
          >
            <SidebarToggleIcon expanded={open} />
          </button>
        </div>

        {/* Sections */}
        <nav className="flex w-full flex-col gap-0.5 border-t border-divider pt-2">
          {availableSections.map((section) => {
            const isActive = activeSection === section.id;

            return (
              <button
                key={section.id}
                type="button"
                onClick={() => onSectionChange(section.id)}
                title={t(section.labelKey)}
                className={`flex w-full items-center gap-2.5 rounded-lg p-2 text-start text-sm font-normal tracking-tight transition-colors duration-150 ${
                  isActive
                    ? "bg-surface-hover text-stone-50"
                    : "text-stone-400 hover:bg-stone-900/60 hover:text-stone-200"
                }`}
              >
                <HugeiconsIcon
                  icon={section.icon}
                  size={17}
                  className="shrink-0 opacity-80"
                />
                <span className="truncate">{t(section.labelKey)}</span>
              </button>
            );
          })}
        </nav>
      </div>
    </motion.nav>
  );
};
