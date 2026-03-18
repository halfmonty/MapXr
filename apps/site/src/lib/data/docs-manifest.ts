export type DocEntry = { slug: string; title: string; group: string };

export const DOCS: DocEntry[] = [
  { slug: 'getting-started', title: 'Getting Started', group: 'Basics'   },
  { slug: 'profiles',        title: 'Profiles',        group: 'Basics'   },
  { slug: 'triggers',        title: 'Triggers',        group: 'Mapping'  },
  { slug: 'actions',         title: 'Actions',         group: 'Mapping'  },
  { slug: 'layers',          title: 'Layers',          group: 'Advanced' },
];
