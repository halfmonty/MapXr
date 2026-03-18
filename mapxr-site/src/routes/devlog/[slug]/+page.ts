import { DEVLOG } from '$lib/data/devlog.js';
import { error } from '@sveltejs/kit';

export function entries() {
  return DEVLOG.map(e => ({ slug: e.slug }));
}

export function load({ params }) {
  const entry = DEVLOG.find(e => e.slug === params.slug);
  if (!entry) error(404, 'Post not found');
  return { entry };
}
