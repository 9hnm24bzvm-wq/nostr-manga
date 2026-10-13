import { getContext, setContext } from "svelte";

class LightboxState {
	urls = $state<string[]>([]);
	visible = $state<boolean>(false);
	index = $state<number>(0);

	set(...values: string[]) {
		this.urls = values;
		this.index = 0;
	}

	open(index?: number) {
		if (index) {
			this.index = index;
		}
		this.visible = true;
	}

	close = $derived(() => {
		this.visible = false;
	});

	prev = $derived((e?: MouseEvent) => {
		e?.preventDefault();
		e?.stopPropagation();
		e?.stopImmediatePropagation();
		this.index = Math.max(0, this.index - 1);
	});

	next = $derived((e?: MouseEvent) => {
		e?.preventDefault();
		e?.stopPropagation();
		e?.stopImmediatePropagation();
		this.index = Math.min(this.urls.length - 1, this.index + 1);
	});
}

const SYMBOL = "lightbox";

export function setLightbox() {
	return setContext<LightboxState>(Symbol.for(SYMBOL), new LightboxState());
}

export function useLightbox() {
	return getContext<LightboxState>(Symbol.for(SYMBOL));
}
