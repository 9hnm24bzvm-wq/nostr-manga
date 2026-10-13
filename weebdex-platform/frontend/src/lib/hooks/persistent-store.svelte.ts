import { browser } from "$app/environment";
import { writable, type Subscriber, type Writable } from "svelte/store";

export class PersistentStore<T> {
	private readonly key: string;
	private readonly w: Writable<T>;
	private readonly origSet: (value: T) => void;

	constructor(key: string, initialValue: T) {
		if (browser) {
			const localRaw = localStorage.getItem(key);
			const localState: T = localRaw ? JSON.parse(localRaw) : initialValue;
			this.key = key;
			this.w = writable(localState);
			this.origSet = this.w.set;

			if (!localRaw) {
				this.set(initialValue);
			}
		}
	}

	subscribe(run: Subscriber<T>) {
		return this.w.subscribe(run);
	}

	set(value: T) {
		localStorage.setItem(this.key, JSON.stringify(value));
		this.origSet(value);
	}

	update(updater: (value: T) => T) {
		this.w.update(updater);
	}
}
