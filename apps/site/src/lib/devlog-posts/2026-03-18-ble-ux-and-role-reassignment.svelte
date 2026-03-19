<h1>BLE UX polish, starter profiles, and live role reassignment</h1>

<p>
  Today was a housekeeping-and-polish session rather than an epic-level feature push. Several small
  but noticeable rough edges in the device-management flow got addressed, a quality-of-life
  improvement was shipped for connected device handling, and the reference docs got a long-overdue
  rename pass.
</p>

<h2>BLE scan UX: cached devices, stale RSSI, and the "Paired" state</h2>

<p>
  The scan list was showing stale data in a few scenarios that were easy to reproduce but annoying
  to diagnose. Three distinct device states needed to be tracked and displayed differently:
</p>

<ul>
  <li>
    <strong>Seen in scan</strong> — the device was actively advertising during the scan window.
    Shows a signal-strength badge (Strong / Good / Fair / Weak) and the Connect button is enabled.
  </li>
  <li>
    <strong>Paired</strong> — the device has an active BLE connection to the OS (BlueZ, Windows
    Bluetooth stack, etc.). The OS clears RSSI for connected peripherals and they stop advertising,
    so no signal reading is available. These show a "Paired" badge and Connect remains enabled so
    the user can take over the connection from the OS.
  </li>
  <li>
    <strong>Cached</strong> — the device is in the OS Bluetooth cache from a previous scan but was
    not seen this time. It is likely off or out of range. Shows a greyed-out "Cached" badge and
    Connect is disabled.
  </li>
</ul>

<p>
  The Linux path had an extra wrinkle. After a device is connected and then disconnected, BlueZ
  clears its RSSI entry. On the next scan the device's first <code>PropertiesChanged</code> signal
  can arrive after the scan event-loop deadline, leaving the RSSI map empty even though the device
  is genuinely present. The fix is a pre-scan RSSI snapshot taken via <code>get_devices()</code>
  before <code>start_discovery</code>. If RSSI was absent pre-scan (cleared by the prior
  connection) and is present post-scan, the device counts as seen.
</p>

<p>
  Two new fields were added to <code>TapDeviceInfo</code> — <code>seen_in_scan</code> and
  <code>is_connected_to_os</code> — and propagated through the DTO, TypeScript types, and
  Svelte UI.
</p>

<h2>Connected device name display</h2>

<p>
  Previously the connected-devices panel showed only the role and address — the human-readable
  device name (e.g. "TapXR_A0363") was lost the moment you clicked Connect.
</p>

<p>
  The fix has two parts. First, the name is captured from the scan result <em>before</em> the
  <code>await connectDevice(...)</code> call, not after. After the await, the Tauri
  <code>device-connected</code> event has already fired, which reactively removes the device from
  <code>availableDevices</code> (the filtered scan list). Looking up the name after the await
  therefore finds nothing.
</p>

<p>
  Second, names are persisted to <code>localStorage</code> under <code>mapxr:device-names</code> as
  an address → name map. On app restart, the auto-reconnect loop fires <code>device-connected</code>
  events with no UI interaction, so <code>setName</code> is never called. Loading from storage on
  <code>DeviceStore</code> construction means reconnect events immediately resolve the correct name.
</p>

<h2>Starter profile seeding on first launch</h2>

<p>
  New installations had no profiles and no obvious way to get started. A
  <code>starter-right.json</code> profile with 15 right-hand single-finger mappings (copy, paste,
  undo, save, arrow navigation, etc.) is now embedded in the binary via <code>include_str!</code>
  and seeded into the OS config directory on first launch.
</p>

<p>
  Seeding is a no-op if any <code>.json</code> file already exists in the config dir, so existing
  users and developers with their own profiles are unaffected.
</p>

<h2>Reference docs reorganisation</h2>

<p>
  The <code>docs/reference/</code> directory had accumulated six files with names that didn't
  reflect their contents. Most notably, <code>other-uuids.txt</code> — which is actually the best
  annotated GATT characteristic map in the project — got renamed to
  <code>gatt-characteristics.txt</code>. The full rename list:
</p>

<ul>
  <li><code>api-doc.txt</code> → <code>tap-ble-api.txt</code></li>
  <li><code>other-uuids.txt</code> → <code>gatt-characteristics.txt</code></li>
  <li><code>gatt-output.txt</code> → <code>gatt-probe-output.txt</code></li>
  <li><code>windows-sdk-guid-reference.txt</code> → <code>windows-sdk-guids.cs</code> (it's C# source; now syntax-highlighted in editors)</li>
  <li><code>gettingfirmware.txt</code> → <code>firmware-update-aws.txt</code></li>
</ul>

<h2>Live role reassignment without disconnecting</h2>

<p>
  The most substantial change today: you can now change a connected device's role (solo → left,
  left → right, etc.) without disconnecting and reconnecting. Previously this was the only way to
  reassign, which meant exiting controller mode, dropping the BLE connection, waiting for the OS to
  release the connection slot, scanning again, and reconnecting.
</p>

<p>
  The key insight is that <code>DeviceId</code> (the role) is just a label — it gets stamped onto
  <code>RawTapEvent</code>s and onto <code>BleStatusEvent</code> notifications, but the underlying
  BLE connection, GATT characteristic handles, controller mode, and notification subscription are
  all properties of the <code>btleplug</code> <code>Peripheral</code> object and don't care about
  the role at all.
</p>

<p>
  The complication is that the role is captured by value in three background tasks spawned at
  connect time: <code>keepalive_task</code>, <code>notification_task</code>, and
  <code>connection_monitor_task</code>. A new <code>TapDevice::reassign()</code> method handles
  this by cancelling the existing tasks via the <code>watch</code> channel, writing
  <code>ENTER_CONTROLLER_MODE</code> immediately to reset the device's 10-second keepalive timer,
  then respawning all three tasks with the new <code>DeviceId</code>.
</p>

<p>
  <code>BleManager::reassign_role()</code> moves the entry in the connected map, calls
  <code>reassign()</code>, then emits <code>BleStatusEvent::Disconnected</code> (old role) followed
  by <code>BleStatusEvent::Connected</code> (new role). The existing Tauri event pump picks these
  up and pushes <code>device-disconnected</code> / <code>device-connected</code> events to the
  frontend — no new plumbing required.
</p>

<p>
  On the UI side, the connected-devices table now shows solo / left / right role buttons inline on
  each row. A role button is disabled if it is the device's current role, or if that role is
  already occupied by another connected device. This prevents conflicts entirely in the UI rather
  than returning an error from the backend.
</p>
