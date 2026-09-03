"use client";

import type React from "react";
import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

import { HugeiconsIcon } from "@hugeicons/react";
import {
	AlertCircleIcon,
	CircleCheckIcon,
	TriangleAlertIcon,
} from "@hugeicons/core-free-icons";


import { useIsLight } from "@/lib/utils/theme";
import { cn } from "./lib/utils";
import { IOSSpinner } from "./shared/global-spinner";


export type SonnerKind =
	| "loading"
	| "success"
	| "error"
	| "warning";

export interface SonnerState {
	kind: SonnerKind;
	message: string;
	id?: string | number;
}

const AUTO_DISMISS_MS = 5000;

const smoothEase = [0.16, 1, 0.3, 1] as const;

const KIND_ICONS = {
	success: CircleCheckIcon,
	error: AlertCircleIcon,
	warning: TriangleAlertIcon,
} as const;

export function Sonner({
	sonner,
	className,
}: {
	sonner: SonnerState | null;
	className?: string;
}): React.ReactElement {
	const [dismissedKey, setDismissedKey] =
		useState<string | null>(null);

	const toastKey = useMemo(() => {
		if (!sonner) return null;

		return sonner.id !== undefined
			? `${sonner.kind}:${sonner.message}:${sonner.id}`
			: `${sonner.kind}:${sonner.message}`;
	}, [sonner]);

	useEffect(() => {
		if (!sonner) {
			setDismissedKey(null);
		}
	}, [sonner]);

	const visible =
		sonner !== null &&
		toastKey !== null &&
		dismissedKey !== toastKey;

	return (
		<AnimatePresence mode="wait">
			{visible && sonner && toastKey && (
				<SonnerToast
					key={toastKey}
					sonner={sonner}
					className={className}
					onDismiss={() => setDismissedKey(toastKey)}
				/>
			)}
		</AnimatePresence>
	);
}

function SonnerToast({
	sonner,
	className,
	onDismiss,
}: {
	sonner: SonnerState;
	className?: string;
	onDismiss: () => void;
}): React.ReactElement {
	const isLight = useIsLight();

	useEffect(() => {
		if (sonner.kind === "loading") return;

		const timer = window.setTimeout(
			onDismiss,
			AUTO_DISMISS_MS,
		);

		return () => window.clearTimeout(timer);
	}, [sonner.kind, onDismiss]);

	const KindIcon =
		sonner.kind === "loading"
			? null
			: KIND_ICONS[sonner.kind];

	const iconColor = {
		loading: isLight
			? "text-stone-400"
			: "text-stone-500",

		success: isLight
			? "text-green-600"
			: "text-green-700",

		error: isLight
			? "text-rose-600"
			: "text-rose-700",

		warning: isLight
			? "text-orange-600"
			: "text-orange-700",
	}[sonner.kind];

	const isAssertive =
		sonner.kind === "error" ||
		sonner.kind === "warning";

	return (
		<div
			className={cn(
				"pointer-events-none fixed bottom-6 left-1/2 z-[60]",
				"w-[calc(100%-2rem)] max-w-[360px] -translate-x-1/2",
				className,
			)}
		>
			<motion.div
				role={isAssertive ? "alert" : "status"}
				aria-live={
					isAssertive ? "assertive" : "polite"
				}
				initial={{
					opacity: 0,
					y: 8,
					scale: 0.985,
					filter: "blur(4px)",
				}}
				animate={{
					opacity: 1,
					y: 0,
					scale: 1,
					filter: "blur(0px)",
				}}
				exit={{
					opacity: 0,
					y: 5,
					scale: 0.99,
					filter: "blur(3px)",
				}}
				transition={{
					duration: 0.28,
					ease: smoothEase,
				}}
				className={cn(
					"flex min-h-11 w-full items-center gap-2.5",
					"rounded-[8px] border px-3.5 py-2.5",
					"text-[13px] font-normal leading-5 tracking-tight antialiased",

					isLight
						? [
								"border-stone-200/70",
								"bg-white",
								"text-stone-900",
							]
						: [
								"border-transparent",
								"bg-stone-800",
								"text-stone-100",
							],
				)}
			>
				<div className="flex size-[18px] shrink-0 items-center justify-center">
					{sonner.kind === "loading" ? (
						<IOSSpinner
							size={16}
							
						/>
					) : KindIcon ? (
						<HugeiconsIcon
							icon={KindIcon}
							size={17}
							strokeWidth={1.7}
							className={iconColor}
						/>
					) : null}
				</div>

				<span className="min-w-0 flex-1">
					{sonner.message}
				</span>
			</motion.div>
		</div>
	);
}