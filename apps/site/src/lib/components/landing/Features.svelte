<script lang="ts">
    import FeatureCard from "./FeatureCard.svelte";

    const features = [
        {
            icon: "⌨️",
            title: "Keyboard Shortcuts",
            description:
                "Map any tap combo to any key sequence — from simple Ctrl+C to complex multi-modifier shortcuts for your favourite apps.",
        },
        {
            icon: "⚡",
            title: "Multi Device Support",
            description:
                "Chain taps across hands for cross-device combos. mapxr resolves single, double, and multi-device tap sequences with precise timing.",
        },
        {
            icon: "📋",
            title: "App-Specific Profiles",
            description:
                "Define different mappings per application. Your TAP Strap automatically switches profiles when you switch windows.",
        },
        {
            icon: "🔀",
            title: "Layers",
            description:
                "Stack multiple mapping layers. Hold a modifier tap to temporarily switch layers, then release to pop back — like a keyboard with Fn keys.",
        },
        {
            icon: "🖱️",
            title: "Mouse & Media",
            description:
                "Control mouse clicks, scroll, media playback, and volume. Everything a keyboard can do, your fingers can do wirelessly.",
        },
        {
            icon: "📝",
            title: "JSON Profiles",
            description:
                "Profiles are plain JSON files you can hand-edit, version-control, and share. No proprietary formats, no cloud lock-in.",
        },
    ];
</script>

<section class="py-16 px-4 bg-base-100">
    <div class="max-w-5xl mx-auto">
        <h2 class="text-3xl font-bold text-center mb-3">Everything you need</h2>
        <p class="text-center text-base-content/60 mb-10">
            Powerful features, zero friction.
        </p>
        <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {#each features as feature}
                <FeatureCard {...feature} />
            {/each}
        </div>
    </div>
</section>
