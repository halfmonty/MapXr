import { listContextRules, saveContextRules } from "../commands";
import type { ContextRule } from "../types";

class ContextRulesStore {
  rules = $state<ContextRule[]>([]);
  saving = $state(false);
  error = $state<string | null>(null);

  async init(): Promise<void> {
    await this.reload();
  }

  async reload(): Promise<void> {
    const data = await listContextRules();
    this.rules = data.rules;
  }

  /** Replace the full rule list, persist to disk, and update local state on success. */
  async save(rules: ContextRule[]): Promise<void> {
    this.saving = true;
    this.error = null;
    try {
      await saveContextRules({ version: 1, rules });
      this.rules = rules;
    } catch (e) {
      this.error = e instanceof Error ? e.message : String(e);
      throw e;
    } finally {
      this.saving = false;
    }
  }
}

export const contextRulesStore = new ContextRulesStore();
