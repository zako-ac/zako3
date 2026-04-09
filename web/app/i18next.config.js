import { defineConfig } from 'i18next-cli';

/** @type {import('i18next-cli').I18nextToolkitConfig} */
export default defineConfig({
    locales: [
        "en",
        "ko"
    ],
    extract: {
        input: "src/**/*.{js,jsx,ts,tsx}",
        output: "src/i18n/locales/{{language}}.json"
    }
});
