# mapxr — project vision

## What is this

mapxr is a desktop application for configuring and running custom input mappings on
[Tap](https://www.tapwithus.com) wearable keyboards. A Tap device is a five-finger
chord keyboard worn on one hand (or two). Each combination of fingers produces a code
that can be mapped to any keyboard shortcut, action, or sequence of actions.

mapxr runs entirely on the user's machine. It connects to Tap devices over Bluetooth,
listens for tap events in the background, and executes the configured actions in real time.

---

## Who it is for

mapxr is built for the **non-technical hobbyist** who bought a Tap device because they
genuinely want to use it as a productivity tool — not as a novelty or party trick.

This is someone who:
- Has a real workflow they want to improve (writing, coding, media production, navigation)
- Is comfortable with software but is not a developer
- Wants to configure their device once and then rely on it
- Will not tolerate software that is flaky, slow, or requires babysitting

mapxr also has meaningful potential for **users with accessibility needs** — anyone for
whom a conventional keyboard is difficult, painful, or impossible to use. For these users
reliability and long-term stability are not preferences, they are requirements.

---

## The problem it solves

The Tap device ships with reasonable default mappings but its real power is
customisability. The official tooling for creating custom maps requires:

1. Visiting a specific third-party website to design the mapping
2. Using a specific Android / iOS app to write the map to the device firmware

This creates a fragile dependency chain. If the website goes offline, if the mobile app
is abandoned, the Tap's customisability becomes inaccessible. A device that costs
hundreds of dollars should not lose utility because a company's SaaS subscription
lapses or a mobile app falls off an app store.

mapxr solves this by providing a **local-first alternative** that owns the full
configuration and execution stack. No website. No mobile app. No account. No internet
connection required to use your device.
---

## Design principles

### 1. Local first, always

All profile data is created, stored, and executed on the user's machine. No cloud account,
no telemetry, no network calls are required for any core functionality. The app must be
fully functional on a machine that has never been connected to the internet.

Community profile sharing (if implemented) must be strictly opt-in and additive — a user
who never shares or downloads profiles must experience zero difference.

### 2. Free, unconditionally

mapxr is free software. The Tap device is expensive hardware; charging additionally for
software to make it useful would be exploitative. There is no premium tier, no feature
gating, and no subscription. If this project ever requires funding, donations are the
only acceptable mechanism.

### 3. Fail to a usable state

When something goes wrong, the app should degrade gracefully rather than break entirely.
A corrupt profile file should not prevent other profiles from loading. A BLE dropout
should reconnect automatically without user intervention. A missing config file should be
recreated from safe defaults, not cause a crash. Users should never need to manually
intervene to restore normal operation after a transient failure.

### 4. Built to last

Good software can be **feature complete**. mapxr is not a platform that requires constant
maintenance and iteration to remain useful. The goal is to build it correctly, ship it,
and have it continue working reliably for years without further intervention.

Concretely: dependencies should be chosen for stability, not novelty. The profile JSON
schema should be versioned and forwards-compatible so that old profiles never break.
Platform APIs used should be stable, well-documented, and unlikely to be deprecated. Code
should be written for the maintainer who returns after two years away, not for today's
convenience.

### 5. The user's device, the user's data

Profile files are plain JSON stored in a well-known location on the user's filesystem.
They can be backed up, version-controlled, shared manually, or edited in a text editor.
mapxr must never obscure or lock away the user's configuration data.

---

## Scope

### In scope

- Connecting to and streaming from one or two Tap devices over Bluetooth
- Creating, editing, and managing input profiles with a visual UI
- Executing keyboard actions, mouse actions, macros, and layer operations in response to taps
- Running in the background (system tray) so mappings are always active
- Context-aware profile switching based on the active application
- A CLI tool for working with profile files outside the GUI
- A packaging and distribution path for Windows, macOS, and Linux
- An Android port sharing the same profile format and frontend

### Out of scope

- Any cloud service, account system, or remote configuration
- Writing mappings directly to Tap device firmware (mapxr works at the software layer;
  it does not alter what the device hardware stores)
- Supporting hardware other than Tap devices
- A scripting language or plugin system beyond what is in the approved action vocabulary
- Monetisation of any kind

---

## Success condition

mapxr is successful when a non-technical user can:

1. Pair their Tap device
2. Create a profile that makes their daily workflow meaningfully faster or easier
3. Close the configuration UI and forget about it

The software should feel like a finished tool, not a project in progress.
