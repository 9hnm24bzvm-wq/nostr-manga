import { getContext, setContext } from "svelte";

class KeyState {
	value = $state(0);

	refresh() {
		this.value++;
	}
}

const SYMBOL_KEY = "key";

export function setKey() {
	return setContext<KeyState>(Symbol.for(SYMBOL_KEY), new KeyState());
}

export function useKey() {
	return getContext<KeyState>(Symbol.for(SYMBOL_KEY));
}
