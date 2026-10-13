import { PUBLIC_KRATOS_PUBLIC_API } from "$env/static/public";
import { Configuration, FrontendApi } from "@ory/client";
import type { FullAutoFill } from "svelte/elements";

export interface UINode {
	attributes: {
		disabled?: boolean;
		name: string;
		node_type: string;
		required?: boolean;
		type?: string;
		value: string;
		autocomplete?: FullAutoFill;
	};
	group: string;
	messages?: {
		id: number;
		text: string;
	}[];
	meta: {
		label: {
			id: number;
			text: string;
			type: string;
		};
	};
	type: string;
}

export const ErrorSessionAlreadyAvailable = "session_already_available";

export default new FrontendApi(
	new Configuration({
		basePath: PUBLIC_KRATOS_PUBLIC_API,
		baseOptions: { withCredentials: true }
	})
);
