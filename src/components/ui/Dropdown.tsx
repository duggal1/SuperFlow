"use client";

import * as React from "react";
import { Menu as MenuPrimitive } from "@base-ui/react/menu";
import { CaretDown, CaretRight } from "@phosphor-icons/react";
import { cn } from "../lib/utils";
import { useIsLight } from "../../lib/utils/theme";

export const MenuCreateHandle: typeof MenuPrimitive.createHandle =
  MenuPrimitive.createHandle;

export const Menu: typeof MenuPrimitive.Root = MenuPrimitive.Root;

export const MenuPortal: typeof MenuPrimitive.Portal = MenuPrimitive.Portal;

export function MenuTrigger({
  className,
  children,
  ...props
}: MenuPrimitive.Trigger.Props): React.ReactElement {
  return (
    <MenuPrimitive.Trigger
      className={className}
      data-slot="menu-trigger"
      {...props}
    >
      {children}
    </MenuPrimitive.Trigger>
  );
}

export function MenuPopup({
  children,
  className,
  sideOffset = 4,
  align = "center",
  alignOffset,
  side = "bottom",
  anchor,
  portalProps,
  ...props
}: MenuPrimitive.Popup.Props & {
  align?: MenuPrimitive.Positioner.Props["align"];
  sideOffset?: MenuPrimitive.Positioner.Props["sideOffset"];
  alignOffset?: MenuPrimitive.Positioner.Props["alignOffset"];
  side?: MenuPrimitive.Positioner.Props["side"];
  anchor?: MenuPrimitive.Positioner.Props["anchor"];
  portalProps?: MenuPrimitive.Portal.Props;
}): React.ReactElement {
  const isLight = useIsLight();
  return (
    <MenuPortal {...portalProps}>
      <MenuPrimitive.Positioner
        align={align}
        alignOffset={alignOffset}
        anchor={anchor}
        className="z-50"
        data-slot="menu-positioner"
        side={side}
        sideOffset={sideOffset}
      >
        <MenuPrimitive.Popup
          className={cn(
            isLight
              ? "relative flex not-[class*='w-']:min-w-32 origin-(--transform-origin) rounded-[6px] border border-stone-200 bg-white text-stone-900 shadow-none outline-none"
              : "relative flex not-[class*='w-']:min-w-32 origin-(--transform-origin) rounded-[6px] border border-stone-700 bg-stone-800 text-stone-50 shadow-none outline-none",
            className,
          )}
          data-slot="menu-popup"
          {...props}
        >
          <div className="max-h-(--available-height) w-full overflow-y-auto p-1">
            {children}
          </div>
        </MenuPrimitive.Popup>
      </MenuPrimitive.Positioner>
    </MenuPortal>
  );
}

export function MenuGroup(
  props: MenuPrimitive.Group.Props,
): React.ReactElement {
  return <MenuPrimitive.Group data-slot="menu-group" {...props} />;
}

export function MenuItem({
  className,
  inset,
  variant = "default",
  ...props
}: MenuPrimitive.Item.Props & {
  inset?: boolean;
  variant?: "default" | "destructive";
}): React.ReactElement {
  const isLight = useIsLight();
  return (
    <MenuPrimitive.Item
      className={cn(
        isLight
          ? "flex min-h-8 cursor-pointer select-none items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-700 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-100/80 data-highlighted:text-stone-900 data-inset:ps-8 data-[variant=destructive]:text-red-400 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&>svg:not([class*='opacity-'])]:opacity-80 [&>svg:not([class*='size-'])]:size-4 [&>svg]:pointer-events-none [&>svg]:-mx-0.5 [&>svg]:shrink-0"
          : "flex min-h-8 cursor-pointer select-none items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-200 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-700 data-highlighted:text-stone-50 data-inset:ps-8 data-[variant=destructive]:text-red-400 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&>svg:not([class*='opacity-'])]:opacity-80 [&>svg:not([class*='size-'])]:size-4 [&>svg]:pointer-events-none [&>svg]:-mx-0.5 [&>svg]:shrink-0",
        className,
      )}
      data-inset={inset}
      data-slot="menu-item"
      data-variant={variant}
      {...props}
    />
  );
}

export function MenuLinkItem({
  className,
  inset,
  variant = "default",
  closeOnClick = true,
  ...props
}: MenuPrimitive.LinkItem.Props & {
  inset?: boolean;
  variant?: "default" | "destructive";
}): React.ReactElement {
  const isLight = useIsLight();
  return (
    <MenuPrimitive.LinkItem
      className={cn(
        isLight
          ? "flex min-h-8 cursor-pointer select-none items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-700 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-100/80 data-highlighted:text-stone-900 data-inset:ps-8 data-[variant=destructive]:text-red-400 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&>svg:not([class*='opacity-'])]:opacity-80 [&>svg:not([class*='size-'])]:size-4 [&>svg]:pointer-events-none [&>svg]:-mx-0.5 [&>svg]:shrink-0"
          : "flex min-h-8 cursor-pointer select-none items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-200 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-700 data-highlighted:text-stone-50 data-inset:ps-8 data-[variant=destructive]:text-red-400 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&>svg:not([class*='opacity-'])]:opacity-80 [&>svg:not([class*='size-'])]:size-4 [&>svg]:pointer-events-none [&>svg]:-mx-0.5 [&>svg]:shrink-0",
        className,
      )}
      closeOnClick={closeOnClick}
      data-inset={inset}
      data-slot="menu-link-item"
      data-variant={variant}
      {...props}
    />
  );
}

export function MenuCheckboxItem({
  className,
  children,
  checked,
  variant = "default",
  ...props
}: MenuPrimitive.CheckboxItem.Props & {
  variant?: "default" | "switch";
}): React.ReactElement {
  return (
    <MenuPrimitive.CheckboxItem
      checked={checked}
      className={cn(
        "grid min-h-8 cursor-pointer items-center gap-2 rounded-[4px] py-1 ps-2 text-base text-stone-200 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-700 data-highlighted:text-stone-50 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0",
        variant === "switch"
          ? "grid-cols-[1fr_auto] gap-4 pe-1.5"
          : "grid-cols-[.75rem_1fr] pe-4",
        className,
      )}
      data-slot="menu-checkbox-item"
      {...props}
    >
      {variant === "switch" ? (
        <>
          <span className="col-start-1">{children}</span>

          <MenuPrimitive.CheckboxItemIndicator
            className="inset-shadow-[0_1px_--theme(--color-black/20%)] inline-flex h-[calc(var(--thumb-size)+2px)] w-[calc(var(--thumb-size)*2-2px)] shrink-0 items-center rounded-full bg-stone-800 p-px outline-none transition-[background-color,box-shadow] duration-200 data-checked:bg-stone-100 data-unchecked:bg-stone-800 sm:[--thumb-size:--spacing(3)] [--thumb-size:--spacing(4)]"
            keepMounted
          >
            <span className="pointer-events-none block aspect-square h-full rounded-(--thumb-size) bg-stone-950 shadow-sm/5 transition-transform duration-150 data-[checked]:translate-x-[calc(var(--thumb-size)-4px)]" />
          </MenuPrimitive.CheckboxItemIndicator>
        </>
      ) : (
        <>
          <MenuPrimitive.CheckboxItemIndicator className="col-start-1 -ms-0.5">
            <svg
              aria-hidden="true"
              fill="none"
              height="24"
              stroke="currentColor"
              strokeLinecap="round"
              strokeLinejoin="round"
              strokeWidth="2"
              viewBox="0 0 24 24"
              width="24"
              xmlns="http://www.w3.org/2000/svg"
            >
              <path d="M5.252 12.7 10.2 18.63 18.748 5.37" />
            </svg>
          </MenuPrimitive.CheckboxItemIndicator>

          <span className="col-start-2">{children}</span>
        </>
      )}
    </MenuPrimitive.CheckboxItem>
  );
}

export function MenuRadioGroup(
  props: MenuPrimitive.RadioGroup.Props,
): React.ReactElement {
  return <MenuPrimitive.RadioGroup data-slot="menu-radio-group" {...props} />;
}

export function MenuRadioItem({
  className,
  children,
  ...props
}: MenuPrimitive.RadioItem.Props): React.ReactElement {
  const isLight = useIsLight();
  return (
    <MenuPrimitive.RadioItem
      className={cn(
        isLight
          ? "grid min-h-8 cursor-pointer grid-cols-[.75rem_1fr] items-center gap-2 rounded-[4px] py-1 ps-2 pe-4 text-base text-stone-700 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-100/80 data-highlighted:text-stone-900 aria-checked:bg-stone-100 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0"
          : "grid min-h-8 cursor-pointer grid-cols-[.75rem_1fr] items-center gap-2 rounded-[4px] py-1 ps-2 pe-4 text-base text-stone-200 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-700 data-highlighted:text-stone-50 aria-checked:bg-stone-700 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none [&_svg]:shrink-0",
        className,
      )}
      data-slot="menu-radio-item"
      {...props}
    >
      <MenuPrimitive.RadioItemIndicator className="col-start-1 -ms-0.5 flex items-center justify-center">
        <span
          aria-hidden="true"
          className="size-1.5 rounded-[1px] bg-blue-600"
        />
      </MenuPrimitive.RadioItemIndicator>

      <span className="col-start-2 flex items-center gap-2">{children}</span>
    </MenuPrimitive.RadioItem>
  );
}

export function MenuGroupLabel({
  className,
  inset,
  ...props
}: MenuPrimitive.GroupLabel.Props & {
  inset?: boolean;
}): React.ReactElement {
  return (
    <MenuPrimitive.GroupLabel
      className={cn(
        "px-2 py-1.5 font-medium text-stone-400 text-xs",
        className,
      )}
      data-inset={inset}
      data-slot="menu-label"
      {...props}
    />
  );
}

export function MenuSeparator({
  className,
  ...props
}: MenuPrimitive.Separator.Props): React.ReactElement {
  return (
    <MenuPrimitive.Separator
      className={cn("mx-2 my-1 h-px bg-stone-800", className)}
      data-slot="menu-separator"
      {...props}
    />
  );
}

export function MenuShortcut({
  className,
  ...props
}: React.ComponentProps<"kbd">): React.ReactElement {
  return (
    <kbd
      className={cn(
        "ms-auto font-medium font-sans text-stone-500 text-xs tracking-widest",
        className,
      )}
      data-slot="menu-shortcut"
      {...props}
    />
  );
}

export function MenuSub(
  props: MenuPrimitive.SubmenuRoot.Props,
): React.ReactElement {
  return <MenuPrimitive.SubmenuRoot data-slot="menu-sub" {...props} />;
}

export function MenuSubTrigger({
  className,
  inset,
  children,
  ...props
}: MenuPrimitive.SubmenuTrigger.Props & {
  inset?: boolean;
}): React.ReactElement {
  const isLight = useIsLight();
  return (
    <MenuPrimitive.SubmenuTrigger
      className={cn(
        isLight
          ? "flex min-h-8 cursor-pointer items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-700 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-100/80 data-popup-open:bg-stone-100 data-inset:ps-8 data-highlighted:text-stone-900 data-popup-open:text-stone-900 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none"
          : "flex min-h-8 cursor-pointer items-center gap-2 rounded-[4px] px-2 py-1 text-base text-stone-200 outline-none data-disabled:pointer-events-none data-highlighted:bg-stone-700 data-popup-open:bg-stone-700 data-inset:ps-8 data-highlighted:text-stone-50 data-popup-open:text-stone-50 data-disabled:opacity-64 sm:min-h-7 sm:text-sm [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none",
        className,
      )}
      data-inset={inset}
      data-slot="menu-sub-trigger"
      {...props}
    >
      {children}

      <CaretRight
        aria-hidden="true"
        className="ms-auto -me-0.5 size-3.5 opacity-60"
        weight="regular"
      />
    </MenuPrimitive.SubmenuTrigger>
  );
}

export interface DropdownOption {
  value: string;
  label: string;
  disabled?: boolean;
  /** Optional leading node (e.g. a flag emoji) rendered before the label. */
  icon?: React.ReactNode;
}

export interface DropdownProps {
  options: DropdownOption[];
  selectedValue?: string;
  onSelect: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  onRefresh?: () => void;
  className?: string;
  /** Renders a filter input at the top of the popup. */
  searchable?: boolean;
  searchPlaceholder?: string;
  emptyLabel?: string;
}

/**
 * Clean select built on the base-ui Menu primitives. Replaces the old
 * self-contained `Dropdown` that was removed during the UI migration, so the
 * ~12 settings consumers compile again. Stone surfaces, 8-10px rounding,
 * aqua-500 for the checked row.
 */
export function Dropdown({
  options,
  selectedValue,
  onSelect,
  placeholder = "Select…",
  disabled = false,
  onRefresh,
  className,
  searchable = false,
  searchPlaceholder,
  emptyLabel,
}: DropdownProps) {
  const isLight = useIsLight();
  const [query, setQuery] = React.useState("");

  const filtered = React.useMemo(() => {
    if (!searchable || !query.trim()) return options;
    const q = query.trim().toLowerCase();
    return options.filter((opt) => opt.label.toLowerCase().includes(q));
  }, [options, query, searchable]);

  const selected = options.find((o) => o.value === selectedValue);

  const renderIcon = (icon: React.ReactNode) =>
    icon ? (
      <span className="flex w-4 shrink-0 items-center justify-center text-[13px] leading-none">
        {icon}
      </span>
    ) : null;

  return (
    <Menu>
      <MenuTrigger
        disabled={disabled}
        className={cn(
          isLight
            ? "flex h-9 min-w-44 cursor-pointer items-center justify-between gap-2 rounded-[10px] border-0 bg-stone-100 px-3 text-[13px] text-stone-900 outline-none transition-colors hover:bg-stone-200 data-disabled:cursor-not-allowed data-disabled:opacity-50"
            : "flex h-9 min-w-44 cursor-pointer items-center justify-between gap-2 rounded-[10px] border-0 bg-stone-700 px-3 text-[13px] text-stone-50 outline-none transition-colors hover:bg-stone-600 data-disabled:cursor-not-allowed data-disabled:opacity-50",
          className,
        )}
      >
        <span className="flex items-center gap-2 truncate">
          {selected ? (
            <>
              {renderIcon(selected.icon)}
              {selected.label}
            </>
          ) : (
            placeholder
          )}
        </span>
        <CaretDown className="size-4 shrink-0 opacity-60" weight="bold" />
      </MenuTrigger>
      <MenuPopup
        className={cn(
          "min-w-36 rounded-lg p-1 shadow-none",
          isLight
            ? "border border-stone-200 bg-white text-stone-900 shadow-none"
            : "border-none bg-stone-700 text-stone-50 shadow-none",
        )}
      >
        {onRefresh && (
          <div className="mb-1 flex items-center justify-between gap-3 px-2 py-1">
            {/* eslint-disable-next-line i18next/no-literal-string */}
            <span className="text-[11px] text-stone-500">Device</span>
            {/* eslint-disable i18next/no-literal-string */}
            <button
              type="button"
              onClick={onRefresh}
              className={
                isLight
                  ? "cursor-pointer text-[11px] text-stone-400 transition-colors hover:text-stone-900"
                  : "cursor-pointer text-[11px] text-stone-400 transition-colors hover:text-stone-50"
              }
            >
              Refresh
            </button>
            {/* eslint-enable i18next/no-literal-string */}
          </div>
        )}
        {searchable && (
          <div
            className={cn(
              "mb-1 px-1 pb-1",
              isLight ? "border-b border-stone-200" : "border-b border-stone-600",
            )}
          >
            <input
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.stopPropagation()}
              placeholder={searchPlaceholder ?? placeholder}
              className={
                isLight
                  ? "w-full rounded-md border border-transparent bg-stone-100 px-2 py-1 text-[13px] text-stone-900 outline-none placeholder:text-stone-400 focus:border-blue-600"
                  : "w-full rounded-md bg-stone-800 px-2 py-1 text-[13px] text-stone-50 outline-none placeholder:text-stone-500"
              }
            />
          </div>
        )}
        <MenuRadioGroup
          value={selectedValue}
          onValueChange={(value) => {
            onSelect(String(value));
            setQuery("");
          }}
        >
          {filtered.map((opt) => (
            <MenuRadioItem
              key={opt.value}
              value={opt.value}
              disabled={opt.disabled}
              className={cn(
                "flex cursor-pointer select-none items-center gap-1.5 rounded-md px-1.5 py-1 text-[13px] outline-none data-disabled:pointer-events-none data-disabled:opacity-50",
                isLight
                  ? "text-stone-900 data-highlighted:bg-stone-100/80"
                  : "text-stone-50 data-highlighted:bg-stone-600",
              )}
            >
              {renderIcon(opt.icon)}
              {opt.label}
            </MenuRadioItem>
          ))}
        </MenuRadioGroup>
        {searchable && filtered.length === 0 && (
          <div className="px-2 py-1.5 text-center text-sm text-stone-400">
            {emptyLabel ?? "No results"}
          </div>
        )}
      </MenuPopup>
    </Menu>
  );
}

export function MenuSubPopup({
  className,
  sideOffset = 0,
  alignOffset,
  align = "start",
  ...props
}: MenuPrimitive.Popup.Props & {
  align?: MenuPrimitive.Positioner.Props["align"];
  sideOffset?: MenuPrimitive.Positioner.Props["sideOffset"];
  alignOffset?: MenuPrimitive.Positioner.Props["alignOffset"];
}): React.ReactElement {
  const defaultAlignOffset = align === "center" ? undefined : -5;

  return (
    <MenuPopup
      align={align}
      alignOffset={alignOffset ?? defaultAlignOffset}
      className={className}
      data-slot="menu-sub-content"
      side="inline-end"
      sideOffset={sideOffset}
      {...props}
    />
  );
}

export {
  Menu as DropdownMenu,
  MenuCheckboxItem as DropdownMenuCheckboxItem,
  MenuCreateHandle as DropdownMenuCreateHandle,
  MenuGroup as DropdownMenuGroup,
  MenuGroupLabel as DropdownMenuLabel,
  MenuItem as DropdownMenuItem,
  MenuPopup as DropdownMenuContent,
  MenuPortal as DropdownMenuPortal,
  MenuPrimitive,
  MenuRadioGroup as DropdownMenuRadioGroup,
  MenuRadioItem as DropdownMenuRadioItem,
  MenuSeparator as DropdownMenuSeparator,
  MenuShortcut as DropdownMenuShortcut,
  MenuSub as DropdownMenuSub,
  MenuSubPopup as DropdownMenuSubContent,
  MenuSubTrigger as DropdownMenuSubTrigger,
  MenuTrigger as DropdownMenuTrigger,
};
