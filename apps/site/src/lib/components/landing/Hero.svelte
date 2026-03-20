<script lang="ts">
    import { base } from "$app/paths";
</script>

<div class="hero min-h-[70vh] bg-base-100">
    <div class="hero-content text-center max-w-3xl">
        <div>
            <img
                src="{base}/images/main.png"
                alt="MapXr app screenshot"
                class="mx-auto mb-8 rounded-2xl shadow-2xl shadow-gray-700 max-w-full"
            />
            <div class="badge badge-primary badge-outline mb-4">
                Open Source · MIT License
            </div>
            <h1 class="text-5xl md:text-6xl font-bold mb-6 leading-tight">
                Your TAP.<br />
                <span class="text-primary">Your rules.</span>
            </h1>
            <p
                class="text-xl text-base-content/70 mb-8 max-w-2xl mx-auto leading-relaxed"
            >
                MapXr lets you remap your TAP keyboard quickly and easily, with
                support for two devices worn at once and more bindable actions
                than you will know what to do with.
            </p>
            <div class="flex flex-wrap gap-3 justify-center">
                <a
                    href="https://github.com/halfmonty/mapxr/releases"
                    target="_blank"
                    rel="noreferrer"
                    class="btn btn-primary btn-lg"
                >
                    Download for Free
                </a>
                <a href="{base}/docs" class="btn btn-outline btn-lg">
                    Read the Docs
                </a>
            </div>
        </div>
    </div>
</div>
