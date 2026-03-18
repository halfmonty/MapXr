<script lang="ts">
  import { base } from '$app/paths';
</script>

<h1>Getting Started</h1>

<p>
  mapxr is a desktop application that connects to your TAP Strap wearable keyboard via Bluetooth
  and remaps finger-tap combinations to keyboard shortcuts, mouse actions, and custom macros.
</p>

<h2>Prerequisites</h2>

<ul>
  <li>A <strong>TAP Strap 2</strong> wearable keyboard</li>
  <li>Windows 10+ or Linux (with BlueZ)</li>
  <li>Bluetooth 4.0+ adapter</li>
</ul>

<h2>Installation</h2>

<ol>
  <li>
    Download the latest release from
    <a href="https://github.com/mapxr/mapxr/releases" target="_blank" rel="noreferrer">GitHub Releases</a>.
  </li>
  <li>On Windows: run the <code>.msi</code> installer.</li>
  <li>On Linux: make the <code>.AppImage</code> executable and run it.</li>
  <li>Launch mapxr. It will appear in your system tray.</li>
</ol>

<h2>Pairing your TAP Strap</h2>

<ol>
  <li>Put your TAP Strap into pairing mode (hold the button until the LED flashes).</li>
  <li>Open mapxr and click <strong>Scan for devices</strong>.</li>
  <li>Select your TAP Strap from the list and click <strong>Connect</strong>.</li>
  <li>The status indicator will turn green when connected.</li>
</ol>

<h2>Loading your first profile</h2>

<p>
  Profiles are JSON files that describe your tap mappings.
  mapxr ships with a <code>default.json</code> profile you can use as a starting point.
</p>

<ol>
  <li>Click <strong>Load Profile</strong> in the mapxr window.</li>
  <li>Browse to a <code>.json</code> profile file and open it.</li>
  <li>mapxr will validate the profile and activate it immediately.</li>
</ol>

<div class="alert alert-info mt-6">
  <span>
    See the <a href="{base}/docs/profiles" class="link">Profiles</a> page for the full profile JSON schema.
  </span>
</div>
