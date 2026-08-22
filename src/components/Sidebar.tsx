import React from "react";
import { useTranslation } from "react-i18next";
import { motion } from "motion/react";
import {
  ClockCounterClockwise,
  Fire,
  Flask,
  Funnel,
  Gear,
  HouseSimple,
  Info,
  Notebook,
} from "@phosphor-icons/react";
import SuperFlowLogo from "./icons/SuperFlowTextLogo";
import SidebarToggleIcon from "./icons/SidebarToggleIcon";
import { HomePage } from "./home";
import { useSettings } from "../hooks/useSettings";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  PostProcessingSettings,
  ModelsSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

interface IconProps {
  width?: number | string;
  height?: number | string;
  size?: number | string;
  className?: string;
  [key: string]: any;
}

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: any) => boolean;
}

export const SECTIONS_CONFIG = {
  home: {
    labelKey: "sidebar.home",
    icon: HouseSimple,
    component: HomePage,
    enabled: () => true,
  },
  general: {
    labelKey: "sidebar.general",
    icon: Notebook,
    component: GeneralSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: ClockCounterClockwise,
    component: HistorySettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: Fire,
    component: ModelsSettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: Gear,
    component: AdvancedSettings,
    enabled: () => true,
  },
  postprocessing: {
    labelKey: "sidebar.postProcessing",
    icon: Funnel,
    component: PostProcessingSettings,
    enabled: (settings) => settings?.post_process_enabled ?? false,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: Flask,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: Info,
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
            const Icon = section.icon;
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
                <Icon size={17} className="shrink-0 opacity-80" />
                <span className="truncate">{t(section.labelKey)}</span>
              </button>
            );
          })}
        </nav>
      </div>
    </motion.nav>
  );
};
