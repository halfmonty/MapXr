<h1>BLE scanner filtering for TAP devices</h1>

<p>
  Before this change, MapXr's Bluetooth scanner would surface every BLE device in range: phones,
  headphones, fitness trackers, smart lightbulbs, all of it. Useful for debugging, but not
  something you want to ship to users.
</p>

<h2>How TAP Strap devices advertise</h2>

<p>
  TAP Strap devices include a known service UUID in their BLE advertisement packets. By filtering
  scan results to only those that include this UUID, we can limit the device list to genuine TAP
  Straps without requiring any connection attempt first.
</p>

<p>
  The filter is applied in the <code>btleplug</code> scan filter configuration, so unrelated
  devices never even reach MapXr's application layer. The OS-level BLE stack discards them before
  they're passed up.
</p>

<h2>Why this matters</h2>

<p>
  Beyond the obvious UX improvement, filtering also reduces the churn of advertisement events that
  the application needs to process. In a busy BLE environment (an office, a coffee shop) an
  unfiltered scan can receive dozens of advertisement packets per second. Filtering to TAP devices
  only reduces that to essentially zero noise between actual device events.
</p>
