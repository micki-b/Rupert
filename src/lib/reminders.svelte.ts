// The reminder settings (#109) for screens that only read them (Details):
// loaded once, then kept in step with the Settings window.
import { api, onSettingsChanged, type ReminderSettings } from "./api";

let current = $state<ReminderSettings | null>(null);
let started = false;

export const reminderSettings = {
  /** `null` until loaded. */
  get value(): ReminderSettings | null {
    if (!started) {
      started = true;
      api.getSettings().then((s) => (current = s.reminders)).catch(() => {});
      onSettingsChanged((s) => (current = s.reminders));
    }
    return current;
  },
};

/** Today on this computer, YYYY-MM-DD. */
export const todayIso = () => new Date().toLocaleDateString("sv");
