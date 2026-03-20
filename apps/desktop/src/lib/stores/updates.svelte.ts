import type { UpdateInfo, UpdateProgressPayload } from "../types";

const DISMISSED_KEY = "mapxr.dismissedUpdate";

class UpdateStore {
  /** The available update, if one has been detected. */
  available = $state<UpdateInfo | null>(null);

  /** Version string of the update the user has dismissed, persisted in localStorage. */
  dismissed = $state<string | null>(
    typeof localStorage !== "undefined" ? localStorage.getItem(DISMISSED_KEY) : null,
  );

  /** True while a download is in progress. */
  downloading = $state(false);

  /** Download progress, updated via `update-download-progress` events. */
  progress = $state<UpdateProgressPayload | null>(null);

  /** Error from the most recent download attempt. */
  downloadError = $state<string | null>(null);

  /** True when an update is available and has not been dismissed. */
  get shouldShow(): boolean {
    return this.available !== null && this.available.version !== this.dismissed;
  }

  /** Record a detected update. Called when an `update-available` event arrives. */
  setAvailable(info: UpdateInfo): void {
    this.available = info;
  }

  /** Dismiss the current update; the banner will not show again for this version. */
  dismiss(): void {
    if (this.available) {
      this.dismissed = this.available.version;
      localStorage.setItem(DISMISSED_KEY, this.available.version);
    }
  }

  /** Update download progress from a `update-download-progress` event payload. */
  applyProgress(payload: UpdateProgressPayload): void {
    this.progress = payload;
  }

  /** Reset download state (e.g. after an error). */
  resetDownload(): void {
    this.downloading = false;
    this.progress = null;
    this.downloadError = null;
  }
}

export const updateStore = new UpdateStore();
