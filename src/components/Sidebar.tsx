import React from "react";
import { useTranslation } from "react-i18next";
import { motion, useReducedMotion } from "motion/react";
import { HugeiconsIcon, type IconSvgElement } from "@hugeicons/react";
import {
  AiBeautifyIcon,
  BadgeInfoIcon,
  ComputerIcon,
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

const SIDEBAR_WIDTH = 184;

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
  general: {
    labelKey: "sidebar.general",
    icon: ComputerIcon,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: ZapIcon,
    component: ModelsSettings,
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
  const reduceMotion = useReducedMotion();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <motion.div
      initial={false}
      animate={{ width: open ? SIDEBAR_WIDTH : 0 }}
      transition={
        reduceMotion
          ? { duration: 0 }
          : { duration: 0.22, ease: [0.32, 0.72, 0, 1] }
      }
      className="relative h-full shrink-0 overflow-hidden"
    >
      <nav className="sidebar-material flex h-full w-[184px] flex-col border-r border-white/[0.045] px-2 pb-2 pt-3">
        <div
          data-tauri-drag-region
          className="flex h-8 shrink-0 items-center justify-end pr-0.5"
        >
          <button
            type="button"
            onClick={onToggle}
            aria-label="Close sidebar"
            title="Close sidebar (Ctrl+B)"
            className="flex size-7 shrink-0 items-center justify-center rounded-md text-stone-400 transition-colors duration-150 hover:bg-white/[0.06] hover:text-stone-100"
          >
            <SidebarToggleIcon expanded={open} />
          </button>
        </div>

        <div className="flex min-h-0 flex-1 flex-col">
          {/* Sidebar content starts below the native titlebar controls. */}
          <div className="mb-3 flex h-8 shrink-0 items-center pl-1.5 pr-0.5">
            <SuperFlowLogo className="text-stone-100" />
          </div>

          <div className="flex w-full flex-col gap-0.5">
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
                      ? "bg-white/[0.075] text-stone-50"
                      : "text-stone-300/75 hover:bg-white/[0.045] hover:text-stone-100"
                  }`}
                >
                  <HugeiconsIcon
                    icon={section.icon}
                    size={16}
                    className="shrink-0 opacity-75"
                  />
                  <span className="truncate">{t(section.labelKey)}</span>
                </button>
              );
            })}
          </div>
        </div>
      </nav>
    </motion.div>
  );
};
