export type DevlogEntry = {
  slug: string;
  date: string;
  title: string;
  body: string;
  epicsDone: number;
  totalEpics: number;
  tags?: string[];
};

export const DEVLOG: DevlogEntry[] = [
  {
    slug: '2026-03-18-profile-persistence',
    date: '2026-03-18',
    title: 'Profile persistence and startup profile selection',
    body: 'Replaced alphabetical startup profile selection with a preferences.json that remembers the last activated profile. Fixed a bug where explicit deactivation was indistinguishable from first-launch state, causing a profile to always be selected on restart. Added device-aware suggestion banners.',
    epicsDone: 7,
    totalEpics: 9,
    tags: ['ux', 'feature', 'bug-fix'],
  },
  {
    slug: '2026-03-18-ble-ux-and-role-reassignment',
    date: '2026-03-18',
    title: 'BLE UX polish, starter profiles, and live role reassignment',
    body: 'Improved scan UX with seen/paired/cached device states and stale-RSSI fixes. Added connected device name persistence. Seeded a starter profile on first launch. Renamed reference docs. Implemented live device role reassignment without disconnecting.',
    epicsDone: 7,
    totalEpics: 9,
    tags: ['ble', 'ui', 'ux', 'feature'],
  },
  {
    slug: '2026-03-18-project-website',
    date: '2026-03-18',
    title: 'Project website launched',
    body: 'Scaffolded the mapxr project site with Svelte 5, Tailwind CSS v4, and DaisyUI v5. Implemented landing page, docs scaffold, and devlog view with full hash-based routing.',
    epicsDone: 7,
    totalEpics: 9,
    tags: ['website', 'docs'],
  },
  {
    slug: '2026-03-16-hardware-debounce-windows-ci',
    date: '2026-03-16',
    title: 'Hardware debounce fix + Windows CI',
    body: 'Fixed double-tap inversion bug caused by TAP Strap hardware bounce. Added 50ms debounce window to ComboEngine. Also added GitHub Actions workflow for Windows builds and a user-facing README.',
    epicsDone: 6,
    totalEpics: 9,
    tags: ['bug-fix', 'ci', 'ble'],
  },
  {
    slug: '2026-03-15-hold-modifier',
    date: '2026-03-15',
    title: 'Hold modifier action (sticky keys)',
    body: 'Implemented the hold_modifier action with Toggle, Count, and Timeout modes. Added HoldModifierMode enum, full validation, and 15+ tests. Spec-approved before implementation.',
    epicsDone: 6,
    totalEpics: 9,
    tags: ['feature', 'mapping-core'],
  },
  {
    slug: '2026-03-14-ble-scanner-filtering',
    date: '2026-03-14',
    title: 'BLE scanner filtering for TAP devices',
    body: 'Updated the BLE scanner to filter advertisements by TAP service UUID so only genuine TAP Strap devices appear in the device list.',
    epicsDone: 5,
    totalEpics: 9,
    tags: ['ble', 'scanner'],
  },
  {
    slug: '2026-03-10-combo-engine',
    date: '2026-03-10',
    title: 'ComboEngine and cross-device combo resolution',
    body: 'Implemented the full ComboEngine with single-tap, double-tap, and cross-device combo detection. Timing is controlled via tokio::time for deterministic tests.',
    epicsDone: 4,
    totalEpics: 9,
    tags: ['engine', 'mapping-core'],
  },
  {
    slug: '2026-03-05-profile-validation-layer-stack',
    date: '2026-03-05',
    title: 'Profile validation and layer stack',
    body: 'Added profile validation rules (duplicate triggers, missing actions, hold_modifier constraints). Implemented LayerStack with push/pop/activate semantics.',
    epicsDone: 3,
    totalEpics: 9,
    tags: ['engine', 'mapping-core'],
  },
  {
    slug: '2026-02-28-core-types',
    date: '2026-02-28',
    title: 'Core types and JSON schema',
    body: 'Defined TapCode, Trigger, Action, Profile, and Layer types. Full serde round-trip tests. Profile deserialization from .json fixture files.',
    epicsDone: 2,
    totalEpics: 9,
    tags: ['types', 'mapping-core'],
  },
];
