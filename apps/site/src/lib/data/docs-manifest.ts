export type DocEntry = { slug: string; title: string; group: string };

export const DOCS: DocEntry[] = [
  { slug: "getting-started", title: "Getting Started", group: "Basics" },
  { slug: "profiles", title: "Profiles", group: "Basics" },
  { slug: "actions", title: "Actions", group: "Profile Features" },
  { slug: "layers", title: "Layers", group: "Profile Features" },
  {
    slug: "profilesettings",
    title: "Profile Settings",
    group: "Profile Features",
  },
  { slug: "aliases", title: "Aliases", group: "Profile Features" },
  { slug: "variables", title: "Variables", group: "Profile Features" },
  { slug: "lifecycle", title: "Lifecycle", group: "Profile Features" },
  { slug: "autoswitch", title: "Autoswitch", group: "Advanced" },
  { slug: "debug", title: "Debug", group: "Advanced" },
  { slug: "settings", title: "Settings", group: "Advanced" },
];
