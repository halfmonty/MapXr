import { listProfiles } from "../commands";
import type { ProfileSummary, ProfileErrorPayload } from "../types";

class ProfileStore {
  profiles = $state<ProfileSummary[]>([]);
  /** Errors accumulated since the last reload. */
  loadErrors = $state<ProfileErrorPayload[]>([]);

  /** Fetch profile list from the backend. Call once on app mount. */
  async init(): Promise<void> {
    await this.reload();
  }

  /** Re-fetch the profile list, clearing any previous load errors. */
  async reload(): Promise<void> {
    this.loadErrors = [];
    this.profiles = await listProfiles();
  }

  /** Called when a `profile-error` event is received during a registry reload. */
  appendError(err: ProfileErrorPayload): void {
    this.loadErrors = [...this.loadErrors, err];
  }
}

export const profileStore = new ProfileStore();
