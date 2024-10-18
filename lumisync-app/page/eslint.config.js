import { defineConfig } from 'eslint/config';
import js from '@eslint/js';
import globals from 'globals';
import solid from 'eslint-plugin-solid/configs/recommended';

export default defineConfig([
  js.configs.recommended,
  solid,
  {
    languageOptions: {
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      'no-unused-vars': [
        'error',
        {
          varsIgnorePattern: '^_',
          argsIgnorePattern: '^_',
        },
      ],
    },
  },
]);
