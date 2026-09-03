import React from "react";
import SelectComponent from "react-select";
import CreatableSelect from "react-select/creatable";
import type {
  ActionMeta,
  Props as ReactSelectProps,
  SingleValue,
  StylesConfig,
} from "react-select";
import { useIsLight } from "../../lib/utils/theme";

export type SelectOption = {
  value: string;
  label: string;
  isDisabled?: boolean;
};

type BaseProps = {
  value: string | null;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  isLoading?: boolean;
  isClearable?: boolean;
  onChange: (value: string | null, action: ActionMeta<SelectOption>) => void;
  onBlur?: () => void;
  className?: string;
  formatCreateLabel?: (input: string) => string;
};

type CreatableProps = {
  isCreatable: true;
  onCreateOption: (value: string) => void;
};

type NonCreatableProps = {
  isCreatable?: false;
  onCreateOption?: never;
};

export type SelectProps = BaseProps & (CreatableProps | NonCreatableProps);

const baseBackground =
  "color-mix(in srgb, var(--color-mid-gray) 10%, transparent)";
const hoverBackground =
  "color-mix(in srgb, var(--color-accent) 12%, transparent)";
const focusBackground =
  "color-mix(in srgb, var(--color-accent) 20%, transparent)";
const neutralBorder =
  "color-mix(in srgb, var(--color-mid-gray) 80%, transparent)";

const getSelectStyles = (isLight: boolean): StylesConfig<SelectOption, false> => ({
  control: (base, state) => ({
    ...base,
    minHeight: 36,
    borderRadius: 6,
    borderColor: state.isFocused ? "var(--color-accent)" : neutralBorder,
    boxShadow: state.isFocused ? "0 0 0 1px var(--color-accent)" : "none",
    backgroundColor: state.isFocused ? focusBackground : baseBackground,
    fontSize: "0.875rem",
    color: "var(--color-text)",
    transition: "all 150ms ease",
    ":hover": {
      borderColor: "var(--color-accent)",
      backgroundColor: hoverBackground,
    },
  }),
  valueContainer: (base) => ({
    ...base,
    paddingInline: 10,
    paddingBlock: 4,
  }),
  input: (base) => ({
    ...base,
    color: "var(--color-text)",
  }),
  singleValue: (base) => ({
    ...base,
    color: "var(--color-text)",
  }),
  dropdownIndicator: (base, state) => ({
    ...base,
    color: state.isFocused
      ? "var(--color-accent)"
      : "color-mix(in srgb, var(--color-mid-gray) 80%, transparent)",
    ":hover": {
      color: "var(--color-accent)",
    },
  }),
  clearIndicator: (base) => ({
    ...base,
    color: "color-mix(in srgb, var(--color-mid-gray) 80%, transparent)",
    ":hover": {
      color: "var(--color-accent)",
    },
  }),
  menu: (provided) => ({
    ...provided,
    zIndex: 30,
    backgroundColor: isLight ? "#ffffff" : "#363230",
    color: "var(--color-text)",
    border: isLight
      ? "1px solid color-mix(in srgb, var(--color-mid-gray) 30%, transparent)"
      : "1px solid rgba(255,255,255,0.06)",
    boxShadow: "0 10px 30px rgba(15, 15, 15, 0.2)",
    paddingTop: 2,
    paddingBottom: 2,
  }),
  menuList: (base) => ({
    ...base,
    paddingTop: 2,
    paddingBottom: 2,
  }),
  option: (base, state) => ({
    ...base,
    backgroundColor: state.isSelected
      ? isLight
        ? focusBackground
        : "rgba(255,255,255,0.08)"
      : state.isFocused
        ? isLight
          ? hoverBackground
          : "rgba(255,255,255,0.08)"
        : "transparent",
    color: "var(--color-text)",
    cursor: state.isDisabled ? "not-allowed" : base.cursor,
    opacity: state.isDisabled ? 0.5 : 1,
    paddingTop: 4,
    paddingBottom: 4,
    paddingLeft: 10,
    paddingRight: 10,
    fontSize: "0.875rem",
  }),
  placeholder: (base) => ({
    ...base,
    color: "color-mix(in srgb, var(--color-mid-gray) 65%, transparent)",
  }),
});

export const Select: React.FC<SelectProps> = React.memo(
  ({
    value,
    options,
    placeholder,
    disabled,
    isLoading,
    isClearable = true,
    onChange,
    onBlur,
    className = "",
    isCreatable,
    formatCreateLabel,
    onCreateOption,
  }) => {
    const isLight = useIsLight();
    const selectValue = React.useMemo(() => {
      if (!value) return null;
      const existing = options.find((option) => option.value === value);
      if (existing) return existing;
      return { value, label: value, isDisabled: false };
    }, [value, options]);

    const handleChange = (
      option: SingleValue<SelectOption>,
      action: ActionMeta<SelectOption>,
    ) => {
      onChange(option?.value ?? null, action);
    };

    const sharedProps: Partial<ReactSelectProps<SelectOption, false>> = {
      className,
      classNamePrefix: "app-select",
      value: selectValue,
      options,
      onChange: handleChange,
      placeholder,
      isDisabled: disabled,
      isLoading,
      onBlur,
      isClearable,
      styles: getSelectStyles(isLight),
    };

    if (isCreatable) {
      return (
        <CreatableSelect<SelectOption, false>
          {...sharedProps}
          onCreateOption={onCreateOption}
          formatCreateLabel={formatCreateLabel}
        />
      );
    }

    return <SelectComponent<SelectOption, false> {...sharedProps} />;
  },
);

Select.displayName = "Select";
