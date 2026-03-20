import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({
      fallback: '404.html',
    }),
    files: {
      assets: 'public',
    },
    paths: {
      base: process.env.NODE_ENV === 'production' ? '/mapxr' : '',
    },
    prerender: {
      handleHttpError: 'warn',
    },
  },
};

export default config;
