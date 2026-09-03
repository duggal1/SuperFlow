import React from "react";
import { useTranslation } from "react-i18next";
import { motion, useReducedMotion } from "motion/react";
import {
  AboutIcon,
  GeneralIcon,
  HistoryIcon,
  HomeIcon,
  PeopleIcon,
  SparklesIcon,
  VADIcon,
  type IconProps,
} from "./icons/sidebar";
import SuperFlowLogo from "./icons/SuperFlowTextLogo";
import SidebarToggleIcon from "./icons/SidebarToggleIcon";
import { HomePage } from "./home";
import { AICleanupSettings } from "./ai-cleanup";
import { MeetingPage } from "./meeting";
import { AgentsPage } from "./agents/AgentsPage";
import { useSettings } from "../hooks/useSettings";
import { useIsLight } from "../lib/utils/theme";
import {
  GeneralSettings,
  AdvancedSettings,
  HistorySettings,
  DebugSettings,
  AboutSettings,
  ModelsSettings,
} from "./settings";

export type SidebarSection = keyof typeof SECTIONS_CONFIG;

const SIDEBAR_WIDTH = 176;

interface SectionConfig {
  labelKey: string;
  icon: React.ComponentType<IconProps>;
  component: React.ComponentType;
  enabled: (settings: ReturnType<typeof useSettings>["settings"]) => boolean;
}

export const SECTIONS_CONFIG = {
  home: {
    labelKey: "sidebar.home",
    icon: HomeIcon,
    component: HomePage,
    enabled: () => true,
  },
  general: {
    labelKey: "sidebar.general",
    icon: GeneralIcon,
    component: GeneralSettings,
    enabled: () => true,
  },
  models: {
    labelKey: "sidebar.models",
    icon: SparklesIcon,
    component: ModelsSettings,
    enabled: () => true,
  },
  meeting: {
    labelKey: "sidebar.meeting",
    icon: PeopleIcon,
    component: MeetingPage,
    enabled: () => true,
  },
  agents: {
    labelKey: "sidebar.agents",
    icon: VADIcon,
    component: AgentsPage,
    enabled: () => true,
  },
  aiCleanup: {
    labelKey: "sidebar.aiCleanup",
    icon: SparklesIcon,
    component: AICleanupSettings,
    enabled: () => true,
  },
  history: {
    labelKey: "sidebar.history",
    icon: HistoryIcon,
    component: HistorySettings,
    enabled: () => true,
  },
  advanced: {
    labelKey: "sidebar.advanced",
    icon: GeneralIcon,
    component: AdvancedSettings,
    enabled: () => true,
  },
  debug: {
    labelKey: "sidebar.debug",
    icon: GeneralIcon,
    component: DebugSettings,
    enabled: (settings) => settings?.debug_mode ?? false,
  },
  about: {
    labelKey: "sidebar.about",
    icon: AboutIcon,
    component: AboutSettings,
    enabled: () => true,
  },
} as const satisfies Record<string, SectionConfig>;

interface SidebarProps {
  activeSection: SidebarSection;
  onSectionChange: (section: SidebarSection) => void;
  open: boolean;
  onToggle: () => void;
  opaque: boolean;
}

export const Sidebar: React.FC<SidebarProps> = ({
  activeSection,
  onSectionChange,
  open,
  onToggle,
  opaque,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const reduceMotion = useReducedMotion();
  const isLight = useIsLight();

  const availableSections = Object.entries(SECTIONS_CONFIG)
    .filter(([_, config]) => config.enabled(settings))
    .map(([id, config]) => ({ id: id as SidebarSection, ...config }));

  return (
    <motion.div
      layout
      initial={false}
      animate={{ width: open ? SIDEBAR_WIDTH : 0 }}
      transition={
        reduceMotion
          ? { duration: 0 }
          : {
              // ultra-luxury open/close: longer, no bounce, pure transform
              // Cmd+B and click share this exact frame
              duration: 0.72,
              ease: [0.16, 1, 0.3, 1],
            }
      }
      style={{ willChange: "transform" }}
      className="relative h-full shrink-0 overflow-hidden"
    >
      <nav
        className={`flex h-full w-[176px] flex-col px-2 pb-2 pt-3 ${
          opaque
            ? isLight
              ? "bg-stone-50"
              : "bg-stone-900"
            : isLight
              ? "sidebar-material border-r border-black/[0.06]"
              : "sidebar-material border-r border-white/[0.045]"
        }`}
      >
        <div
          data-tauri-drag-region
          className="flex h-8 shrink-0 items-center justify-end pr-0.5"
        >
          <button
            type="button"
            onClick={onToggle}
            aria-label="Close sidebar"
            title="Close sidebar (Ctrl+B)"
            className={
              isLight
                ? "flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-[0.5px] text-stone-900"
                : "flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-[0.5px] text-stone-100"
            }
          >
            <SidebarToggleIcon expanded={open} />
          </button>
        </div>

        <div className="flex min-h-0 flex-1 flex-col">
          {/* Sidebar content starts below the native titlebar controls. */}
          <div className="mb-3 flex h-8 shrink-0 items-center pl-1.5 pr-0.5">
            <SuperFlowLogo
              className={isLight ? "text-stone-950" : "text-stone-100"}
            />
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
                      ? isLight
                        ? "bg-stone-200 text-stone-900"
                        : "bg-white/[0.075] text-stone-50"
                      : isLight
                        ? "text-stone-600 hover:bg-stone-200 hover:text-stone-900"
                        : "text-stone-300/75 hover:bg-white/[0.045] hover:text-stone-100"
                  }`}
                >
                  <section.icon className="size-4 shrink-0 opacity-90" />
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
