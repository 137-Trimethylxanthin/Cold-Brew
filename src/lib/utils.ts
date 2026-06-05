import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
	return twMerge(clsx(inputs));
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type WithElementRef<T> = T & { ref?: any };

export type WithoutChildrenOrChild<T> = Omit<T, "children" | "child">;

export type WithoutChild<T> = Omit<T, "child">;
