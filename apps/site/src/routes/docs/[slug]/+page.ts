import { DOCS } from '$lib/data/docs-manifest.js';
import { error } from '@sveltejs/kit';

export function entries() {
  return DOCS.map(d => ({ slug: d.slug }));
}

export function load({ params }) {
  const entry = DOCS.find(d => d.slug === params.slug);
  if (!entry) error(404, 'Page not found');
  return { slug: params.slug, title: entry.title };
}
