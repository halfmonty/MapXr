<script lang="ts">
  import { base } from '$app/paths';
</script>

<h1>Profiles</h1>

<p>
  A <strong>profile</strong> is a JSON file that tells mapxr how to translate TAP Strap finger
  combinations into actions. You can have multiple profiles — one per application, workflow,
  or context — and switch between them at any time.
</p>

<h2>Profile structure</h2>

<pre><code>{`{
  "name": "My Profile",
  "description": "Optional description",
  "default_layer": "base",
  "layers": {
    "base": {
      "mappings": [
        {
          "trigger": { "type": "single", "device": "left", "fingers": [1, 0, 0, 0, 0] },
          "action":  { "type": "key", "key": "ctrl+c" }
        }
      ]
    }
  }
}`}</code></pre>

<h2>Top-level fields</h2>

<div class="overflow-x-auto">
  <table class="table table-zebra">
    <thead>
      <tr>
        <th>Field</th>
        <th>Type</th>
        <th>Required</th>
        <th>Description</th>
      </tr>
    </thead>
    <tbody>
      <tr>
        <td><code>name</code></td>
        <td>string</td>
        <td>Yes</td>
        <td>Human-readable profile name</td>
      </tr>
      <tr>
        <td><code>description</code></td>
        <td>string</td>
        <td>No</td>
        <td>Optional description</td>
      </tr>
      <tr>
        <td><code>default_layer</code></td>
        <td>string</td>
        <td>Yes</td>
        <td>Name of the layer that is active on startup</td>
      </tr>
      <tr>
        <td><code>layers</code></td>
        <td>object</td>
        <td>Yes</td>
        <td>Map of layer name → layer definition</td>
      </tr>
    </tbody>
  </table>
</div>

<h2>App-specific profiles</h2>

<p>
  To activate a profile automatically when a specific application is in focus, add an
  <code>app_match</code> field:
</p>

<pre><code>{`{
  "name": "VS Code",
  "app_match": { "process_name": "code" },
  ...
}`}</code></pre>

<p>
  mapxr checks the active window every 500 ms and switches profiles automatically.
  If no app-specific profile matches, the global default profile is used.
</p>

<div class="alert alert-info mt-6">
  <span>
    See <a href="{base}/docs/triggers" class="link">Triggers</a> and
    <a href="{base}/docs/actions" class="link">Actions</a> for the full list of mapping options.
  </span>
</div>
