export interface LanguageMeta {
  code: string;
  nativeName: string;
  dir: "ltr" | "rtl";
}

export const LANGUAGES: LanguageMeta[] = [
  { code: "en", nativeName: "English", dir: "ltr" },
  { code: "tr", nativeName: "Türkçe", dir: "ltr" },
  { code: "fr", nativeName: "Français", dir: "ltr" },
  { code: "de", nativeName: "Deutsch", dir: "ltr" },
  { code: "ru", nativeName: "Русский", dir: "ltr" },
  { code: "es", nativeName: "Español", dir: "ltr" },
  { code: "ar", nativeName: "العربية", dir: "rtl" },
  { code: "zh", nativeName: "中文", dir: "ltr" },
  { code: "ja", nativeName: "日本語", dir: "ltr" },
  { code: "ko", nativeName: "한국어", dir: "ltr" },
];

export const DEFAULT_LANGUAGE = "en";

export function isSupportedLanguage(code: string): boolean {
  return LANGUAGES.some((language) => language.code === code);
}

export function languageDir(code: string): "ltr" | "rtl" {
  return LANGUAGES.find((language) => language.code === code)?.dir ?? "ltr";
}
