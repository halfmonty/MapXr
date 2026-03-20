<script lang="ts">
    import { base } from "$app/paths";
</script>

<h1>Getting Started</h1>

<p>
    mapxr is a desktop application that connects to your TAP Strap wearable
    keyboard via Bluetooth and remaps finger-tap combinations to keyboard
    shortcuts, mouse actions, and custom macros.
</p>

<h2>Prerequisites</h2>

<ul>
    <li>
        A <strong>TAP Strap</strong> or <strong>TapXr</strong> device
    </li>
    <li>Windows 10+ or Linux (with BlueZ)</li>
    <li>Bluetooth 4.0+ adapter</li>
</ul>

<h2>Installation</h2>

<ol>
    <li>
        Download the latest release from
        <a
            class="link link-primary"
            href="https://github.com/halfmonty/mapxr/releases"
            target="_blank"
            rel="noreferrer">GitHub Releases</a
        >.
    </li>
    <li>
        On Windows: run the <code>.msi</code> or <code>.exe</code> installer.
    </li>
    <li>
        On Linux: a <code>.rpm</code> and <code>.deb</code> package are
        available. <br />A <code>.AppImage</code> is also available but may not work
        on all Linux distributions due to wayland gtk issues.
    </li>
</ol>

<h2>Pairing your TAP Strap</h2>

<ol>
    <li>
        Put your TAP Strap into pairing mode (ensure it is not connected to any
        other devices).
    </li>
    <li>Open MapXr in Devices and click <strong>Scan</strong>.</li>
    <img class="m-2" src="{base}/images/devicescan.png" alt="Devices Scan" />
    <li>
        Select your TAP Strap from the list, choose the role (solo if you have
        one tap, left or right if you have two) and click <strong
            >Connect</strong
        >.
    </li>
    <li>The device will appear in the connected devices list.</li>

    <img
        class="m-2"
        src="{base}/images/deviceconnected.png"
        alt="Device Connected"
    />
</ol>

<h2>Next Step</h2>

<p>
    Your taps should now be registered with MapXr and visible on the left side
    of the window.
</p>

<p>All that is left is to create a profile to define your tap mappings.</p>

<div class="alert alert-info mt-6">
    <span>
        See the <a href="{base}/docs/profiles" class="link">Profiles</a> page for
        the full profile JSON schema.
    </span>
</div>
