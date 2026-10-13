export default {
	content: ["./src/**/*.{html,js,svelte,ts}"],
	safelist: [{ pattern: /^grid-cols-\d+/ }],
	theme: {
		extend: {}
	},
	plugins: []
};
