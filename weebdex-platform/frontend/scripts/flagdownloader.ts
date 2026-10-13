import fs from "fs";
import fetch from "node-fetch";

const Codes: Record<string, string> = {
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

const flags = Object.values(Codes);

flags.forEach(code => {
	console.info(`https://mangadex.org/img/flags/${code}.svg`);
	fetch(`https://mangadex.org/img/flags/${code}.svg`).then(res => {
		if (res.ok && res.body) {
			const stream = fs.createWriteStream(`./static/images/flags/${code}.svg`);
			res.body.pipe(stream);
		}
	});
});
