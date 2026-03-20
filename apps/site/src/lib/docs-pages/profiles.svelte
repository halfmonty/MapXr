<script lang="ts">
    import { base } from "$app/paths";
</script>

<h1>Profiles</h1>

<p>
    A <strong>profile</strong> is a map of TAP Strap finger combinations to actions.
</p>
<img class="m-2" src="{base}/images/profile.png" alt="Device Connected" />

<h2>Creating a Profile</h2>

<ul>
    Click the <code>+New</code> button in the top right corner of the Profiles
    tab
    <img class="m-2" src="{base}/images/new.png" alt="New Profile" />
</ul>
<ul>
    You will be prompted to enter a profile name and select if it is a single
    handed profile or a dual profile for two Tap devices.<br />
    If single you can select which hand the profile is for.

    <img class="m-2" src="{base}/images/prompt.png" alt="New Profile Prompt" />
</ul>
<br />
<ul>
    Once the profile is created you can configure the profile mappings,
    settings, aliases, variables and lifecycle events.
    <img class="m-2" src="{base}/images/tabs.png" alt="Profile Tabs" />
</ul>

<div class="alert alert-info mt-6">
    <span>
        See <a href="{base}/docs/actions" class="link">Actions</a> for the full list
        of mapping options.
    </span>
</div>
