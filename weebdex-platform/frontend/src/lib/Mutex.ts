import { get, writable } from "svelte/store";

class Mutex {
	private readonly state = writable<boolean>(false);
	readonly subscribe = this.state.subscribe;

	isLocked = () => get(this.state);
	lock = () => this.state.set(true);
	unlock = () => this.state.set(false);

	tryLock = async (onBusy?: () => void) => {
		if (get(this.state)) {
			if (onBusy) {
				onBusy();
			}
			await new Promise<void>(resolve => {
				const wait = () => {
					if (get(this.state)) {
						window.requestAnimationFrame(() => wait());
					} else {
						resolve();
					}
				};
				window.requestAnimationFrame(() => wait());
			});
		}
		this.lock();
	};

	wait = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {
		await this.tryLock(onBusy);
		const r = await callback();
		this.unlock();
		return r;
	};

	exec = async <T>(callback: () => Promise<T> | T, onBusy?: () => void) => {
		if (get(this.state)) {
			if (onBusy) {
				onBusy();
			}
			return;
		}

		this.lock();
		const r = await callback();
		this.unlock();
		return r;
	};

	preventDefault = (node: HTMLElement) => {
		if (node instanceof HTMLFormElement) {
			const onSubmit = (ev: SubmitEvent) => {
				if (this.isLocked()) {
					ev.preventDefault();
					ev.stopPropagation();
					ev.stopImmediatePropagation();
				}
			};
			node.addEventListener("submit", onSubmit);
		} else {
			const onClick = (ev: MouseEvent) => {
				if (this.isLocked()) {
					ev.preventDefault();
					ev.stopPropagation();
					ev.stopImmediatePropagation();
				}
			};
			node.addEventListener("click", onClick);
		}
		return {
			destroy() {}
		};
	};
}
export default Mutex;
