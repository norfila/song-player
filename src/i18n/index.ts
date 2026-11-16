import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import de from './locales/de.json';
import en from './locales/en.json';
import appearanceDe from './locales/appearance.de.json';
import appearanceEn from './locales/appearance.en.json';
import settingsNavDe from './locales/settings-nav.de.json';
import settingsNavEn from './locales/settings-nav.en.json';
import configurationDe from './locales/configuration.de.json';
import configurationEn from './locales/configuration.en.json';

import featuresDe from './locales/features.de.json';
import featuresEn from './locales/features.en.json';
import workspaceDe from './locales/workspace.de.json';
import workspaceEn from './locales/workspace.en.json';

void i18n.use(initReactI18next).init({
  resources: {
    de: { translation: { ...de, ...appearanceDe, ...settingsNavDe, ...configurationDe, ...featuresDe, ...workspaceDe } },
    en: { translation: { ...en, ...appearanceEn, ...settingsNavEn, ...configurationEn, ...featuresEn, ...workspaceEn } },
  },
  lng: 'de',
  fallbackLng: 'de',
  interpolation: { escapeValue: false },
  returnNull: false,
});

export default i18n;
