/// <reference types="vite-plugin-pwa/client" />

declare global {
	export interface Number {
		date(short?: boolean): string;
		namespace(): string;
		short(): string;
	}
	export interface String {
		short(): string;
		ago(): string;
	}
}
export {};
