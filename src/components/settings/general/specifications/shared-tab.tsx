import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettings } from "../../../../hooks/useSettings";
import { Input } from "../../../ui/Input";
import { ToggleSwitch } from "../../../ui/ToggleSwitch";
import { Dropdown } from "../../../ui/Dropdown";

/* ------------------------------------------------------------------ *
 * Superflow works best in Gmail, Slack, and Outlook.
 * One local, hybrid specification drives all three:
 *   typed/first-class keys  +  custom user-defined key/value pairs.
 * ------------------------------------------------------------------ */

interface IdentitySpec {
  preferred_name: string;
  full_name: string;
  job_title: string;
  company: string;
  timezone: string;
}

interface EmailSpec {
  signature_name: string;
  signoff: string;
  tone: string;
  include_job_title: boolean;
  include_company: boolean;
  phone: string;
  website: string;
  address: string;
}

interface SlackSpec {
  display_name: string;
  tone: string;
  paragraph_style: string;
  prefer_bullets: boolean;
  use_emoji: boolean;
}

interface UserSpecification {
  enabled: boolean;
  identity: IdentitySpec;
  email: EmailSpec;
  slack: SlackSpec;
  custom: Record<string, string>;
}

const emptySpecification = (): UserSpecification => ({
  enabled: false,
  identity: {
    preferred_name: "",
    full_name: "",
    job_title: "",
    company: "",
    timezone: "",
  },
  email: {
    signature_name: "",
    signoff: "Talk soon",
    tone: "formal",
    include_job_title: true,
    include_company: true,
    phone: "",
    website: "",
    address: "",
  },
  slack: {
    display_name: "",
    tone: "casual-professional",
    paragraph_style: "short",
    prefer_bullets: true,
    use_emoji: false,
  },
  custom: {},
});

type TabId = "slack" | "gmail" | "outlook";

const northAmericanTimeZones = [
  { id: "America/New_York", label: "Eastern Time (ET)" },
  { id: "America/Chicago", label: "Central Time (CT)" },
  { id: "America/Denver", label: "Mountain Time (MT)" },
  { id: "America/Los_Angeles", label: "Pacific Time (PT)" },
  { id: "America/Anchorage", label: "Alaska Time (AKT)" },
  { id: "Pacific/Honolulu", label: "Hawaii Time (HST)" },
  { id: "America/Halifax", label: "Atlantic Time (AT)" },
  { id: "America/St_Johns", label: "Newfoundland Time (NT)" },
] as const;

function zoneTime(id: string, now: Date): string {
  try {
    return new Intl.DateTimeFormat("en-US", {
      timeZone: id,
      hour: "numeric",
      minute: "2-digit",
    }).format(now);
  } catch {
    return "";
  }
}

const tabs = [
  {
    id: "slack" as const,
    label: "Slack",
    icon: "/icons/slack.svg",
    iconAlt: "Slack",
  },
  {
    id: "gmail" as const,
    label: "Gmail",
    icon: "/icons/gmail.svg",
    iconAlt: "Gmail",
  },
  {
    id: "outlook" as const,
    label: "Outlook",
    icon: "/icons/microsoft-outlook.svg",
    iconAlt: "Microsoft Outlook",
  },
];

function parseSpecification(raw: unknown): UserSpecification {
  if (typeof raw !== "string" || raw.trim() === "") return emptySpecification();
  try {
    const parsed = JSON.parse(raw) as Partial<UserSpecification>;
    const base = emptySpecification();
    return {
      enabled: parsed.enabled ?? base.enabled,
      identity: { ...base.identity, ...parsed.identity },
      email: { ...base.email, ...parsed.email },
      slack: { ...base.slack, ...parsed.slack },
      custom: parsed.custom ?? {},
    };
  } catch {
    return emptySpecification();
  }
}

export default function Page() {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const [spec, setSpec] = useState<UserSpecification>(() =>
    parseSpecification(getSetting("user_specification")),
  );
  const [activeTab, setActiveTab] = useState<TabId>("slack");
  const [customRows, setCustomRows] = useState<[string, string][]>(() =>
    Object.entries(spec.custom),
  );

  const [now, setNow] = useState<Date>(() => new Date());
  useEffect(() => {
    const interval = setInterval(() => setNow(new Date()), 1000);
    return () => clearInterval(interval);
  }, []);

  const active = tabs.find((tab) => tab.id === activeTab) ?? tabs[0];

  const persist = (next: UserSpecification) => {
    setSpec(next);
    void updateSetting("user_specification", JSON.stringify(next));
  };

  const patch = <K extends keyof UserSpecification>(
    key: K,
    value: UserSpecification[K],
  ) => persist({ ...spec, [key]: value });

  const patchIdentity = (value: Partial<IdentitySpec>) =>
    persist({ ...spec, identity: { ...spec.identity, ...value } });

  const patchEmail = (value: Partial<EmailSpec>) =>
    persist({ ...spec, email: { ...spec.email, ...value } });

  const patchSlack = (value: Partial<SlackSpec>) =>
    persist({ ...spec, slack: { ...spec.slack, ...value } });

  const isEmailSurface = activeTab === "gmail" || activeTab === "outlook";

  const updateCustomRows = (next: [string, string][]) => {
    setCustomRows(next);
    persist({
      ...spec,
      custom: Object.fromEntries(next.filter(([k]) => k.trim() !== "")),
    });
  };

  return (
    <div className="space-y-5">
      {/* Tabs — keep the icon rail exactly as designed */}
      <div className="flex items-center gap-1 rounded-2xl bg-white/[0.06] p-1.5 ring-1 ring-white/[0.05]">
        {tabs.map((tab) => {
          const isActive = tab.id === activeTab;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setActiveTab(tab.id)}
              aria-pressed={isActive}
              aria-label={tab.label}
              className={[
                "group relative flex h-14 w-14 items-center cursor-pointer justify-center",
                "rounded-xl outline-none",
                "transition-[background-color,opacity,transform,filter,box-shadow] duration-200",
                "focus-visible:ring-2 focus-visible:ring-white/20",
                isActive
                  ? [
                      "bg-stone-800",
                      "opacity-100",
                      "shadow-[0_1px_3px_rgba(0,0,0,0.32),0_6px_18px_rgba(0,0,0,0.28)]",
                      "ring-1 ring-white/[0.06]",
                    ].join(" ")
                  : [
                      "opacity-45",
                      "grayscale",
                      "hover:bg-white/[0.05]",
                      "hover:opacity-75",
                      "hover:grayscale-0",
                    ].join(" "),
              ].join(" ")}
            >
              <div className="relative h-9 w-9">
                <img
                  src={tab.icon}
                  alt={tab.iconAlt}
                  draggable={false}
                  className={[
                    "h-full w-full object-contain",
                    "transition-opacity duration-200",
                    isActive ? "opacity-100" : "opacity-75",
                  ].join(" ")}
                />
              </div>
              <span
                className={[
                  "absolute -bottom-[7px] left-1/2 h-1 w-1",
                  "-translate-x-1/2 rounded-full bg-stone-200",
                  "transition-opacity duration-200",
                  isActive ? "opacity-100" : "opacity-0",
                ].join(" ")}
              />
            </button>
          );
        })}
      </div>

      {/* Master toggle */}
      <div className="flex items-center justify-between px-1">
        <div>
          <p className="text-sm font-medium text-stone-100">
            {t("specifications.useFor", { surface: active.label })}
          </p>
          <p className="text-xs text-stone-500">{t("specifications.useHint")}</p>
        </div>
        <ToggleSwitch
          checked={spec.enabled}
          onChange={(value) => patch("enabled", value)}
          label={t("specifications.toggleLabel")}
          description={t("specifications.toggleDescription")}
          descriptionMode="tooltip"
          bare
        />
      </div>

      {spec.enabled && (
        <div className="space-y-6 rounded-[12px] bg-stone-900/40 p-4 ring-1 ring-white/[0.05]">
          {/* Identity — first-class, shared across surfaces */}
          <Section title={t("specifications.identity.title")}>
            <Field
              horizontal
              label={t("specifications.identity.preferredName")}
              value={spec.identity.preferred_name}
              onChange={(v) => patchIdentity({ preferred_name: v })}
              placeholder={t("specifications.identity.preferredName")}
            />
            <Field
              horizontal
              label={t("specifications.identity.jobTitle")}
              value={spec.identity.job_title}
              onChange={(v) => patchIdentity({ job_title: v })}
              placeholder={t("specifications.identity.jobTitle")}
            />
            <Field
              horizontal
              label={t("specifications.identity.company")}
              value={spec.identity.company}
              onChange={(v) => patchIdentity({ company: v })}
              placeholder={t("specifications.identity.company")}
            />
            <div className="flex items-center justify-between gap-6">
              <span className="w-32 shrink-0 text-sm text-stone-300">
                {t("specifications.identity.timezone")}
              </span>
              <Dropdown
                className="w-full"
                options={northAmericanTimeZones.map((z) => ({
                  value: z.id,
                  label: `${z.label}  ·  ${zoneTime(z.id, now)}`,
                }))}
                selectedValue={spec.identity.timezone || undefined}
                onSelect={(value) => patchIdentity({ timezone: value })}
                placeholder={t("specifications.identity.timezone")}
              />
            </div>
          </Section>

          {/* Email-specific typed keys */}
          {isEmailSurface && (
            <Section title={t("specifications.email.title")}>
              <Field
                horizontal
                label={t("specifications.email.signatureName")}
                value={spec.email.signature_name}
                onChange={(v) => patchEmail({ signature_name: v })}
                placeholder={t("specifications.email.signatureName")}
              />
              <Field
                horizontal
                label={t("specifications.email.signoff")}
                value={spec.email.signoff}
                onChange={(v) => patchEmail({ signoff: v })}
                placeholder={t("specifications.email.signoff")}
              />
              <BoolField
                label={t("specifications.email.includeJobTitle")}
                checked={spec.email.include_job_title}
                onChange={(v) => patchEmail({ include_job_title: v })}
              />
              <BoolField
                label={t("specifications.email.includeCompany")}
                checked={spec.email.include_company}
                onChange={(v) => patchEmail({ include_company: v })}
              />
              <Field
                horizontal
                label={t("specifications.email.phone")}
                value={spec.email.phone}
                onChange={(v) => patchEmail({ phone: v })}
                placeholder={t("specifications.email.phone")}
              />
              <Field
                horizontal
                label={t("specifications.email.website")}
                value={spec.email.website}
                onChange={(v) => patchEmail({ website: v })}
                placeholder={t("specifications.email.website")}
              />
              <Field
                horizontal
                label={t("specifications.email.address")}
                value={spec.email.address}
                onChange={(v) => patchEmail({ address: v })}
                placeholder={t("specifications.email.address")}
              />
            </Section>
          )}

          {/* Slack-specific typed keys */}
          {activeTab === "slack" && (
            <Section title={t("specifications.slack.title")}>
              <Field
                horizontal
                label={t("specifications.slack.displayName")}
                value={spec.slack.display_name}
                onChange={(v) => patchSlack({ display_name: v })}
                placeholder={t("specifications.slack.displayName")}
              />
            </Section>
          )}

          {/* Custom — arbitrary user-defined key/value pairs */}
          <Section title={t("specifications.custom.title")}>
            <div className="space-y-2">
              {customRows.length === 0 && (
                <p className="text-xs text-stone-600">
                  {t("specifications.custom.empty")}
                </p>
              )}
              {customRows.map(([key, value], index) => (
                <div key={index} className="flex items-center gap-2">
                  <Input
                    variant="compact"
                    className="w-2/5"
                    value={key}
                    placeholder={t("specifications.custom.keyPlaceholder")}
                    onChange={(e) => {
                      const next = customRows.map((row, i) =>
                        i === index
                          ? ([e.target.value, row[1]] as [string, string])
                          : row,
                      );
                      updateCustomRows(next);
                    }}
                  />
                  <Input
                    variant="compact"
                    className="flex-1"
                    value={value}
                    placeholder={t("specifications.custom.valuePlaceholder")}
                    onChange={(e) => {
                      const next = customRows.map((row, i) =>
                        i === index
                          ? ([row[0], e.target.value] as [string, string])
                          : row,
                      );
                      updateCustomRows(next);
                    }}
                  />
                  <button
                    type="button"
                    aria-label={t("specifications.custom.remove")}
                    onClick={() => {
                      updateCustomRows(
                        customRows.filter((_, i) => i !== index),
                      );
                    }}
                    className="shrink-0 rounded-md px-2 py-1 text-stone-500 transition-colors hover:bg-white/[0.05] hover:text-stone-200"
                  >
                    <svg
                      viewBox="0 0 24 24"
                      className="h-3.5 w-3.5"
                      fill="none"
                      stroke="currentColor"
                      strokeWidth={2}
                      strokeLinecap="round"
                      aria-hidden="true"
                    >
                      <path d="M6 6l12 12M18 6L6 18" />
                    </svg>
                  </button>
                </div>
              ))}
              <button
                type="button"
                onClick={() => updateCustomRows([...customRows, ["", ""]])}
                className="rounded-md px-2 py-1 text-xs font-medium text-blue-400 transition-colors hover:text-blue-300"
              >
                {t("specifications.custom.add")}
              </button>
            </div>
          </Section>
        </div>
      )}
    </div>
  );
}

/* ----------------------------- primitives ----------------------------- */

function Section({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-xs font-medium uppercase tracking-wide text-stone-400">
          {title}
        </h3>
        {hint && <p className="mt-0.5 text-xs text-stone-600">{hint}</p>}
      </div>
      <div className="space-y-3">{children}</div>
    </div>
  );
}

function Field({
  label,
  value,
  onChange,
  placeholder,
  horizontal = false,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  horizontal?: boolean;
}) {
  if (horizontal) {
    return (
      <label className="flex items-center justify-between gap-6">
        <span className="w-32 shrink-0 text-sm text-stone-300">{label}</span>
        <Input
          variant="compact"
          className="flex-1"
          value={value}
          placeholder={placeholder}
          onChange={(e) => onChange(e.target.value)}
        />
      </label>
    );
  }

  return (
    <label className="block space-y-1">
      <span className="text-xs text-stone-400">{label}</span>
      <Input
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
      />
    </label>
  );
}

function BoolField({
  label,
  checked,
  onChange,
}: {
  label: string;
  checked: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-sm text-stone-300">{label}</span>
      <ToggleSwitch
        checked={checked}
        onChange={onChange}
        label={label}
        description={label}
        descriptionMode="tooltip"
        bare
      />
    </div>
  );
}
