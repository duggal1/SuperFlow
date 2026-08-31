import React from "react";
import { useTranslation } from "react-i18next";
import { type } from "@tauri-apps/plugin-os";
import { MicrophoneSelector } from "../MicrophoneSelector";
import { ChannelSelector } from "../ChannelSelector";
import { ShortcutInput } from "../ShortcutInput";
import { SettingsGroup } from "../../ui/SettingsGroup";
import { OutputDeviceSelector } from "../OutputDeviceSelector";
import { PushToTalk } from "../PushToTalk";
import { AudioFeedback } from "../AudioFeedback";
import { useSettings } from "../../../hooks/useSettings";
import { VolumeSlider } from "../VolumeSlider";
import { MuteWhileRecording } from "../MuteWhileRecording";
import { ModelSettingsCard } from "./ModelSettingsCard";
import { ShortcutsCard } from "./ShortcutsCard";
import { VoiceHookCard } from "./VoiceHookCard";
import SpecificationPanel from "./specifications/shared-tab";
import { useIsLight } from "../../../lib/utils/theme";

export const GeneralSettings: React.FC = () => {
  const { t } = useTranslation();
  const { audioFeedbackEnabled, getSetting } = useSettings();
  const isLight = useIsLight();
  const pushToTalk = getSetting("push_to_talk");
  const isLinux = type() === "linux";
  return (
    <div className="max-w-3xl w-full mx-auto space-y-6">
      <SettingsGroup title={t("settings.general.title")}>
        <ShortcutInput shortcutId="transcribe" grouped={true} />
        <ShortcutInput shortcutId="hands_free_transcribe" grouped={true} />
        <PushToTalk descriptionMode="tooltip" grouped={true} />
        {/* Cancel shortcut is hidden with push-to-talk (release key cancels) and on Linux (dynamic shortcut instability) */}
        {!isLinux && !pushToTalk && (
          <ShortcutInput shortcutId="cancel" grouped={true} />
        )}
      </SettingsGroup>
      <ShortcutsCard />
      <VoiceHookCard />
      <ModelSettingsCard />
      <div>
        <div className="px-4">
          <h2 className="text-xs font-medium text-stone-500 uppercase tracking-wide">
            {t("specifications.title")}
          </h2>
        </div>
        <div className={`mt-3 rounded-[10px] bg-surface px-4 py-4 ${isLight ? "border border-stone-200/80" : ""}`}>
          <SpecificationPanel />
        </div>
      </div>
      <SettingsGroup title={t("settings.sound.title")}>
        <MicrophoneSelector descriptionMode="tooltip" grouped={true} />
        <ChannelSelector descriptionMode="tooltip" grouped={true} />
        <MuteWhileRecording descriptionMode="tooltip" grouped={true} />
        <AudioFeedback descriptionMode="tooltip" grouped={true} />
        <OutputDeviceSelector
          descriptionMode="tooltip"
          grouped={true}
          disabled={!audioFeedbackEnabled}
        />
        <VolumeSlider disabled={!audioFeedbackEnabled} />
      </SettingsGroup>
    </div>
  );
};
