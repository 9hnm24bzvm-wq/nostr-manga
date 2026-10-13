// Corrupt/denied browser storage must not crash app initialization or lose updates.
import { browser } from "$app/environment";
import { get, writable, type Subscriber, type Writable } from "svelte/store";
import { toast } from "svelte-sonner";
function normalizeStored(value: unknown, defaults: unknown, key: string, field = ""): unknown {
	if (Array.isArray(defaults)) {
		if (!Array.isArray(value) || value.length > 1000) throw Error("Invalid stored preference array");
		if (["uploaders", "groups"].includes(field)) {
			if (value.some(row => !row || typeof row !== "object" || Array.isArray(row) || typeof row.id !== "string" || !row.id.length || row.id.length > 128 || row.name !== undefined && typeof row.name !== "string")) throw Error("Invalid stored preference identity");
		} else if (value.some(row => typeof row !== "string" || row.length > 128)) throw Error("Invalid stored preference array member");
		return value;
	}
	if (defaults !== null && typeof defaults === "object") {
		if (!value || typeof value !== "object" || Array.isArray(value)) throw Error("Invalid stored preference object");
		const source = value as Record<string, unknown>;
		return Object.fromEntries(Object.entries(defaults).map(([name, fallback]) => [name, name in source ? normalizeStored(source[name], fallback, key, name) : fallback]));
	}
	if (typeof value !== typeof defaults || value === null) throw Error("Invalid stored preference type");
	if (typeof value === "number") {
		if (!Number.isFinite(value)) throw Error("Invalid stored preference number");
		const ranges: Record<string, [number, number]> = { scale: [0, 6], layout: [0, 1], direction: [0, 1], offset: [0, 10], width: [10, 100], gap: [0, 100], scroll: [5, 100], preload: [1, 100], parallel: [1, 10], version: [0, 2147483647] };
		const range = ranges[field]; if (range && (!Number.isInteger(value) || value < range[0] || value > range[1])) throw Error("Invalid stored preference range");
		if (key === "whyxk2oq" && ![0, 1, 2].includes(value)) throw Error("Invalid comments visibility");
	}
	if (typeof value === "string") {
		if (value.length > 128 || field === "items_per_page" && (!/^[0-9]{1,3}$/.test(value) || Number(value) < 1 || Number(value) > 100) || field === "panel_position" && !["left", "right"].includes(value) || ["nrs2qcav", "a0yb25h8"].includes(key) && !["grid", "list"].includes(value) || key === "ho5r3tmc" && !["settings", "chapters", "groups", "comments"].includes(value)) throw Error("Invalid stored preference choice");
	}
	return value;
}
export class PersistentStore<T> {
	private readonly key: string;
	private readonly w: Writable<T>;
	constructor(key: string, initialValue: T) {
		this.key = key;
		let value = initialValue;
		let absent = false;
		if (browser) {
			try {
				const raw = localStorage.getItem(key);
				absent = raw === null;
				if (raw !== null) value = normalizeStored(JSON.parse(raw), initialValue, key) as T;
			} catch {
				// Keep malformed old bytes for recovery until an explicit subsequent edit.
				console.warn("Stored device preferences could not be read; defaults are active.");
			}
		}
		this.w = writable(value);
		if (browser && absent) this.set(initialValue);
	}
	subscribe(run: Subscriber<T>) { return this.w.subscribe(run); }
	set(value: T) {
		if (browser) {
			try { localStorage.setItem(this.key, JSON.stringify(value)); }
			catch { toast.error("Device preferences could not be saved", { description: "The current value is active only in this page. Browser storage was denied or full." }); }
		}
		this.w.set(value);
	}
	update(updater: (value: T) => T) { this.set(updater(get(this.w))); }
}
