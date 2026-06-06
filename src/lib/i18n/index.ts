import en from './en';
import de from './de';

export type Locale = 'en' | 'de';
export type TranslationKey = keyof typeof en;

const translations: Record<Locale, Record<TranslationKey, string>> = { en, de };

const STORAGE_KEY = 'coldbrew.locale';

let currentLocale: Locale = 'en';

export function initLocale(): Locale {
	if (typeof localStorage !== 'undefined') {
		const saved = localStorage.getItem(STORAGE_KEY);
		if (saved === 'en' || saved === 'de') {
			currentLocale = saved;
		}
	}
	return currentLocale;
}

export function setLocale(locale: Locale): void {
	currentLocale = locale;
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem(STORAGE_KEY, locale);
	}
}

export function getLocale(): Locale {
	return currentLocale;
}

export function t(key: TranslationKey, vars?: Record<string, string>): string {
	let text = translations[currentLocale][key] ?? translations.en[key] ?? key;
	if (vars) {
		for (const [k, v] of Object.entries(vars)) {
			text = text.replace(`{${k}}`, v);
		}
	}
	return text;
}

export function localeName(locale: Locale): string {
	return locale === 'en' ? 'English' : 'Deutsch';
}
