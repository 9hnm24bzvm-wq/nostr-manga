const Map: Record<string, string> = {
	en: "gb",
	ja: "jp",
	ko: "kr",
	zh: "cn",
	"zh-hk": "hk",

	id: "id",
	jv: "id",
	vi: "vn",

	af: "za",
	sq: "al",
	ar: "sa",
	az: "az",
	eu: "eu",
	be: "by",
	bn: "bd",
	bg: "bg",
	my: "mm",
	ca: "ad",

	cv: "ru-cu",
	hr: "hr",
	cs: "cz",
	da: "dk",
	nl: "nl",
	eo: "eo",
	et: "et",
	tl: "ph",
	fi: "fi",
	fr: "fr",
	ka: "ka",
	de: "de",
	el: "gr",
	he: "il",
	hi: "in",
	hu: "hu",

	ga: "ie",
	it: "it",
	kk: "kz",
	la: "ri",
	lt: "lt",
	ms: "my",
	mn: "mn",
	mx: "mx",
	ne: "np",
	no: "no",
	fa: "ir",
	pl: "pl",
	pt: "pt",
	"pt-br": "br",
	ro: "ro",
	ru: "ru",
	sr: "rs",
	sk: "sk",
	sl: "si",
	es: "es",
	"es-la": "mx",
	sv: "se",
	tam: "tam",
	te: "tel",
	th: "th",
	tr: "tr",
	uk: "ua",
	ur: "pk",
	uz: "uz"
};

export const Languages: Record<string, string> = {
	en: "English",
	ja: "Japanese",
	ko: "Korean",
	zh: "Chinese (Simplified)",
	"zh-hk": "Chinese (Traditional)",

	id: "Indonesian",
	jv: "Javanese",
	vi: "Vietnamese",

	af: "Afrikaans",
	sq: "Albanian",
	ar: "Arabic",
	az: "Azerbaijani",
	eu: "Basque",
	be: "Belarusian",
	bn: "Bengali",
	bg: "Bulgarian",
	my: "Burmese",
	ca: "Catalan",

	cv: "Chuvash",
	hr: "Croatian",
	cs: "Czech",
	da: "Danish",
	nl: "Dutch",
	eo: "Esperanto",
	et: "Estonian",
	tl: "Filipino",
	fi: "Finish",
	fr: "French",
	ka: "Georgian",
	de: "German",
	el: "Greek",
	he: "Hebrew",
	hi: "Hindi",
	hu: "Hungarian",
	ga: "Irish",
	it: "Italian",
	kk: "Kazakh",
	la: "Latin",
	lt: "Lithuanian",
	ms: "Malay",
	mn: "Mongolian",
	ne: "Nepali",
	no: "Norwegian",
	fa: "Persian",
	pl: "Polish",
	pt: "Portuguese",
	"pt-br": "Portuguese (Brazil)",
	ro: "Romanian",
	ru: "Russian",
	sr: "Serbian",
	sk: "Slovak",
	sl: "Slovenian",
	es: "Spanish",
	"es-la": "Spanish (LATAM)",
	sv: "Swedish",
	tam: "Tamil",
	te: "Telugu",
	th: "Thai",
	tr: "Turkish",
	uk: "Ukranian",
	ur: "Urdu",
	uz: "Uzbek"
};

export const to3166 = (code: string): string => {
	if (code && code in Map) {
		return Map[code];
	} else if (code.includes("-")) {
		if (code.endsWith("-ro")) {
			return to3166(code.split("-")[0]);
		}
		return to3166(code.split("-")[1]);
	}
	return "unknown";
};

export const toLanguage = (code: string): string => {
	if (code && code in Languages) {
		return Languages[code];
	}
	return "";
};
